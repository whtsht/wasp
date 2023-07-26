use crate::binary::Module;

use super::stack::Stack;

const HOST_MODULE: &str = "env";

pub struct Instance {
    funcs: Vec<FuncInst>,
    start: Option<usize>,
}

impl Instance {
    pub fn new(module: Module) -> Self {
        let mut funcs = vec![];
        for import in module.imports.into_iter() {
            match import.module.as_str() {
                HOST_MODULE => funcs.push(FuncInst::HostFunc { name: import.name }),
                _ => {}
            }
        }

        Self {
            funcs,
            start: module.start.map(|idx| idx as usize),
        }
    }
}

pub enum FuncInst {
    InnerFunc {},
    HostFunc { name: String },
}

pub struct Runtime<E: HostEnv> {
    instance: Instance,
    _stack: Stack,
    env: E,
}

impl<E: HostEnv> Runtime<E> {
    pub fn new(module: Module, env: E) -> Self {
        let instance = Instance::new(module);
        Self {
            instance,
            _stack: Stack::new(),
            env,
        }
    }
    pub fn start(&mut self) {
        if let Some(index) = self.instance.start {
            match &self.instance.funcs[index] {
                FuncInst::HostFunc { name } => self.env.call(name),
                _ => todo!(),
            }
        }
    }
}

pub trait HostEnv {
    fn call(&mut self, name: &str);
}

pub struct DefaultHostEnv {}

impl HostEnv for DefaultHostEnv {
    fn call(&mut self, name: &str) {
        match name {
            "start" => {
                println!("hello");
            }
            _ => {
                panic!("unknown function: {}", name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use wabt::wat2wasm;

    use crate::loader::parser::Parser;

    use super::{DefaultHostEnv, Runtime};

    #[test]
    fn test_simple() {
        let wasm = wat2wasm(
            r#"(module
                   (import "env" "start" (func $start))
                   (start $start)
               )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = Runtime::new(module, DefaultHostEnv {});
        runtime.start();
    }
}
