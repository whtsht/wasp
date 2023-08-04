use glob::glob;
use log::info;
use serde_json::Value;
use std::io::Write;
use std::{
    fmt::Debug,
    fs::{self, File},
    io::Read,
    path::PathBuf,
    process::Command,
};
use wasp::exec::value::LittleEndian;
use wasp::{
    binary::Module,
    exec::{
        env::{DebugEnv, Env},
        importer::{DefaultImporter, Importer},
        runtime::Runtime,
        value::Value as WValue,
    },
    loader::parser::Parser,
};

const WAST_DIR: &str = "./spec/test/core";
const WAST2JSON: &str = "wast2json";

#[test]
fn main() {
    env_logger::builder()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();

    run_tests();
}

#[derive(Debug, PartialEq)]
enum TestCommand<'a> {
    AssertReturn {
        action: Action<'a>,
        expected: Vec<WValue>,
    },
    Module {
        filename: &'a str,
    },
}

impl<'a> TestCommand<'a> {
    fn from_value(v: &'a Value) -> Option<Self> {
        let ty = v.get("type").unwrap().as_str().unwrap();
        match ty {
            "assert_return" => Some(TestCommand::AssertReturn {
                action: Action::from_value(v.get("action").unwrap())?,
                expected: v
                    .get("expected")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(json_to_value)
                    .collect(),
            }),
            "module" => Some(TestCommand::Module {
                filename: v.get("filename").unwrap().as_str().unwrap(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Action<'a> {
    Invoke { fnname: &'a str, args: Vec<WValue> },
}

fn json_to_value(value: &Value) -> WValue {
    let ty = value.get("type").unwrap().as_str().unwrap();
    let value = value.get("value").unwrap().as_str().unwrap();
    if value == "nan:canonical" {
        match ty {
            "f32" => WValue::F32(f32::NAN),
            "f64" => WValue::F64(f64::NAN),
            _ => panic!(),
        }
    } else {
        let value = value.parse::<u64>().unwrap();
        let mut buf = vec![0u8; 8];
        LittleEndian::write(&mut buf, 0, value);

        match ty {
            "i32" => WValue::I32(LittleEndian::read(&buf, 0)),
            "i64" => WValue::I64(LittleEndian::read(&buf, 0)),
            "f32" => WValue::F32(LittleEndian::read(&buf, 0)),
            "f64" => WValue::F64(LittleEndian::read(&buf, 0)),
            _ => panic!(),
        }
    }
}

impl<'a> Action<'a> {
    fn from_value(v: &'a Value) -> Option<Self> {
        let ty = v.get("type").unwrap().as_str().unwrap();
        if ty == "invoke" {
            Some(Action::Invoke {
                fnname: v.get("field").unwrap().as_str().unwrap(),
                args: v
                    .get("args")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(json_to_value)
                    .collect(),
            })
        } else {
            None
        }
    }
}

fn get_test_case<'a>(v: &'a Value) -> Vec<TestCommand<'a>> {
    let v = v.as_object().unwrap();
    let commands = v.get("commands").unwrap().as_array().unwrap();
    commands
        .iter()
        .filter_map(|v| TestCommand::from_value(v))
        .collect()
}

fn get_module(filename: &str) -> Module {
    let mut file = File::open(&format!("{}/{}", WAST_DIR, filename)).unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf).unwrap();
    let mut parser = Parser::new(&buf);
    parser.module().unwrap()
}

macro_rules! assert_eq_with_nan {
    ($left:expr, $right:expr) => {{
        match (&$left, &$right) {
            (left_val, right_val) => {
                if left_val.is_nan() && right_val.is_nan() {
                    // Both are NaN, consider them equal.
                } else {
                    // If one is NaN and the other is not, raise an assertion failure.
                    assert!(
                        !left_val.is_nan() && !right_val.is_nan(),
                        "Assertion failed: `(left == right)` with NaN values: `NaN == {:?}`",
                        if left_val.is_nan() {
                            right_val
                        } else {
                            left_val
                        }
                    );

                    // Regular equality check for non-NaN values.
                    assert_eq!(left_val, right_val);
                }
            }
        }
    }};
}

fn run_test<E: Env + Debug, I: Importer + Debug>(
    runtime: &mut Runtime<E, I>,
    command: &TestCommand,
) {
    match command {
        TestCommand::AssertReturn { action, expected } => match action {
            Action::Invoke { fnname, args } => {
                let ret = runtime.invoke(fnname, args.clone()).unwrap();
                assert_eq!(&ret, expected);
                info!("assert_return: {}:({:?}) == {:?}", fnname, args, ret);
            }
        },
        TestCommand::Module { filename } => {
            let module = get_module(&filename);
            runtime.resister_module(module).unwrap();
        }
    }
}

pub fn run_tests() {
    let entries = fs::read_dir(WAST_DIR).unwrap();

    for entry in entries {
        if let Ok(entry) = entry {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("wast") {
                info!("{:?}", entry.path());
                wast2json(&entry.path());

                let mut json = entry.path().clone();
                json.set_extension("json");
                let mut file = File::open(json).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();

                let v: Value = serde_json::from_str(&content).unwrap();
                let commands = get_test_case(&v);

                let mut runtime =
                    Runtime::without_module(DefaultImporter::new(), DebugEnv {}, "env");
                for command in commands.iter() {
                    run_test(&mut runtime, command);
                }
            }
        }
    }
    clean_up();
}

fn wast2json(input_file: &PathBuf) {
    let input = input_file.to_str().unwrap();
    let mut output = input_file.clone();
    output.set_extension("json");
    let output = output.to_str().unwrap();
    Command::new(WAST2JSON)
        .args(&[input, "-o", output])
        .output()
        .unwrap();
}

fn clean_up() {
    let get_files = |ext: &str| {
        glob(&format!("{}/*.{}", WAST_DIR, ext))
            .unwrap()
            .filter_map(Result::ok)
    };
    let wasm_files = get_files("wasm");
    let wat_files = get_files("wat");
    let json_files = get_files("json");

    for file in wasm_files.chain(wat_files).chain(json_files) {
        fs::remove_file(file).unwrap();
    }
}
