use std::{env, fs::File, io::Read};

use watagasi::{
    exec::{runtime::debug_runtime, stack::Value},
    loader::parser::Parser,
};

fn main() {
    let mut args = env::args();
    if let Some(modname) = args.nth(1) {
        let mut file = File::open(modname).expect("failed to read file");
        let mut buffer = vec![];
        file.read_to_end(&mut buffer).expect("failed to read file");
        let module = Parser::new(&buffer)
            .module()
            .expect("failed to parse module");

        let mut runtime = debug_runtime(module).expect("failed to load module");
        match runtime.invoke("_start", vec![Value::I32(0)]) {
            Ok(_) => {}
            Err(err) => println!("{:?}", err),
        }
    }
}
