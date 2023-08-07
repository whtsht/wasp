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
use wasp::exec::importer::Importer;
use wasp::exec::store::Store;
use wasp::exec::value::LittleEndian;
use wasp::{
    binary::Module,
    exec::{env::Env, runtime::Runtime, value::Value as WValue},
    loader::parser::Parser,
};

const WAST_DIR: &str = "./tests/testsuite";
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
    Action {
        action: Action<'a>,
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
            "action" => Some(TestCommand::Action {
                action: Action::from_value(v.get("action").unwrap())?,
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
    if value.find("nan").is_some() {
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

struct SpecTestImporter {}
impl Importer for SpecTestImporter {
    fn import(&mut self, modname: &str) -> Option<Module> {
        let mut file = File::open(&format!("{}/{}", WAST_DIR, modname)).unwrap();
        let mut buf = vec![];
        file.read_to_end(&mut buf).unwrap();
        let mut parser = Parser::new(&buf);
        Some(parser.module().unwrap())
    }
}

struct SpecTestEnv {}
impl Env for SpecTestEnv {
    fn call(
        &mut self,
        name: &str,
        _params: Vec<WValue>,
        _memory: Option<&mut wasp::exec::store::MemInst>,
    ) -> Result<Vec<WValue>, wasp::exec::env::EnvError> {
        if name == "print" {}
        Ok(vec![])
    }
}

fn run_test(
    runtime: &mut Runtime,
    store: &mut Store,
    env: &mut SpecTestEnv,
    command: &TestCommand,
) {
    match command {
        TestCommand::AssertReturn { action, expected } => match action {
            Action::Invoke { fnname, args } => {
                info!("{}({:?})", fnname, args);
                let ret = runtime.invoke(store, env, fnname, args.clone()).unwrap();
                assert_eq!(
                    &ret, expected,
                    "\nexpected {:?}, found {:?}\n fnname: {:?}",
                    expected, ret, fnname
                );
                info!("    = {:?}", ret);
            }
        },
        TestCommand::Module { filename } => {
            *store = Store::new();
            *runtime = Runtime::new("spectest");
            let mut importer = SpecTestImporter {};
            runtime
                .resister_module(store, &mut importer, &filename)
                .unwrap();
            runtime.start(store, env).ok();
        }
        TestCommand::Action { action } => match action {
            Action::Invoke { fnname, args } => {
                info!("{}: {:?}", fnname, args);
                runtime.invoke(store, env, fnname, args.clone()).unwrap();
            }
        },
    }
}

fn skip(filename: &str) -> bool {
    // TODO
    let skip_list = [
        "./tests/testsuite/imports.wast",
        "./tests/testsuite/exports.wast",
        "./tests/testsuite/binary-leb128.wast",
        "./tests/testsuite/data.wast",
        "./tests/testsuite/elem.wast",
        "./tests/testsuite/linking.wast",
    ];
    for s in skip_list.iter() {
        if filename == *s {
            return true;
        }
    }
    false
}

pub fn run_tests() {
    let entries = fs::read_dir(WAST_DIR).unwrap();

    for entry in entries {
        if let Ok(entry) = entry {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("wast") {
                if skip(entry.path().to_str().unwrap()) {
                    continue;
                }

                info!("{:?}", entry.path());
                wast2json(&entry.path());

                let mut json = entry.path().clone();
                json.set_extension("json");
                let mut file = File::open(json).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();

                let v: Value = serde_json::from_str(&content).unwrap();
                let commands = get_test_case(&v);

                let mut runtime = Runtime::new("spectest");
                let mut store = Store::new();
                let mut env = SpecTestEnv {};
                for command in commands.iter() {
                    run_test(&mut runtime, &mut store, &mut env, command);
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
