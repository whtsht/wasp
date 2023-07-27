use super::stack::{Label, Stack, StackValue};
use super::trap::{Result, Trap};
use crate::binary::{Export, Func, FuncType, ImportDesc, Instr, Module, ResultType};
use alloc::rc::Rc;

pub type Addr = usize;

pub const HOST_MODULE: &str = "__env";

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Instance {
    funcaddrs: Vec<Addr>,
    start: Option<usize>,
    exports: Vec<Export>,
}

impl Instance {
    pub fn new(module: Module, store: &mut Store) -> Self {
        let mut funcs = vec![];

        for import in module.imports.into_iter() {
            match import.desc {
                ImportDesc::TypeIdx(idx) => match import.module.as_str() {
                    HOST_MODULE => {
                        let addr = store.allocate(FuncInst::HostFunc {
                            functype: module.types[idx as usize].clone(),
                            name: import.name,
                        });
                        funcs.push(addr);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        for func in module.funcs {
            funcs.push(store.allocate(FuncInst::InnerFunc {
                functype: module.types[func.typeidx as usize].clone(),
                instance: Rc::new(Instance::default()),
                func: Rc::new(func),
            }))
        }

        Self {
            funcaddrs: funcs,
            start: module.start.map(|idx| idx as usize),
            exports: module.exports,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FuncInst {
    InnerFunc {
        functype: FuncType,
        instance: Rc<Instance>,
        func: Rc<Func>,
    },
    HostFunc {
        functype: FuncType,
        name: String,
    },
}

#[derive(Debug)]
pub struct Store {
    funcs: Vec<FuncInst>,
}

pub trait Allocatable {
    fn allocate(store: &mut Store, value: Self) -> Addr;
}

impl Allocatable for FuncInst {
    fn allocate(store: &mut Store, value: Self) -> Addr {
        store.funcs.push(value);
        store.funcs.len() - 1
    }
}

impl Store {
    pub fn new() -> Self {
        Self { funcs: vec![] }
    }

    pub fn update_func_inst(&mut self, instance: &Rc<Instance>) {
        self.funcs = self
            .funcs
            .drain(..)
            .map(|f| {
                if let FuncInst::InnerFunc {
                    functype,
                    instance: _,
                    func,
                } = f
                {
                    FuncInst::InnerFunc {
                        functype,
                        instance: instance.clone(),
                        func,
                    }
                } else {
                    f
                }
            })
            .collect();
    }

    pub fn allocate<T: Allocatable>(&mut self, value: T) -> Addr {
        Allocatable::allocate(self, value)
    }
}

#[derive(Debug)]
pub struct Runtime<E: HostEnv> {
    instance: Rc<Instance>,
    store: Store,
    stack: Stack,
    env: E,
}

#[derive(Debug)]
pub enum ExecState {
    Breaking(u32),
    Continue,
    Return,
}

impl<E: HostEnv> Runtime<E> {
    pub fn new(module: Module, env: E) -> Self {
        let mut store = Store::new();
        let instance = Rc::new(Instance::new(module, &mut store));
        store.update_func_inst(&instance);
        Self {
            instance,
            stack: Stack::new(),
            store,
            env,
        }
    }

    pub fn start(&mut self) {
        if let Some(index) = self.instance.start {
            match self.store.funcs[index].clone() {
                FuncInst::HostFunc { functype, name } => {
                    self.env.call(&name, &functype, &mut self.stack)
                }
                FuncInst::InnerFunc {
                    functype,
                    instance: _,
                    func,
                } => {
                    assert_eq!(functype, FuncType(ResultType(vec![]), ResultType(vec![])));
                    match self.exec(&func.as_ref().body.0) {
                        Ok(_) => {}
                        Err(trap) => println!("RuntimeError: {}", trap),
                    }
                }
            }
        }
    }

    pub fn exec(&mut self, instrs: &Vec<Instr>) -> Result<ExecState> {
        let mut next = 0;
        loop {
            if next >= instrs.len() {
                return Ok(ExecState::Return);
            }
            match self.step(&instrs, next)? {
                ExecState::Continue => {}
                ret => return Ok(ret),
            }
            next += 1;
        }
    }

    pub fn binary_op<F: Fn(T, T) -> T, T: StackValue>(&mut self, func: F) {
        let lhs = self.stack.pop_value::<T>();
        let rhs = self.stack.pop_value::<T>();
        self.stack.push_value(func(lhs, rhs));
    }

    pub fn step(&mut self, instrs: &Vec<Instr>, next: usize) -> Result<ExecState> {
        match &instrs[next] {
            Instr::I32Const(a) => self.stack.push_value(*a),
            Instr::I32Add => self.binary_op(|a: i32, b: i32| a + b),
            Instr::Call(a) => {
                let func = &self.store.funcs[*a as usize];
                match func {
                    FuncInst::HostFunc { functype, name } => {
                        self.env.call(name, functype, &mut self.stack);
                    }
                    _ => return Err(Trap::NotImplemented),
                }
            }
            Instr::Block { bt: _, in1 } => {
                self.stack.push_label(Label { typeidx: 0 });
                self.exec(in1)?;
            }
            Instr::Br(l) => {
                while self.stack.labels_len() > *l as usize {
                    self.stack.pop_label();
                }
                return Ok(ExecState::Breaking(*l));
            }

            _ => return Err(Trap::NotImplemented),
        }

        Ok(ExecState::Continue)
    }
}

pub trait HostEnv {
    fn call(&mut self, name: &str, functype: &FuncType, stack: &mut Stack);
}

pub struct DefaultHostEnv {}

impl HostEnv for DefaultHostEnv {
    fn call(&mut self, name: &str, functype: &FuncType, stack: &mut Stack) {
        match name {
            "start" => {
                println!("hello {:?}", functype);
            }
            "print" => {
                println!("{}", stack.pop_value::<i32>());
            }
            _ => {
                panic!("unknown function: {}", name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DefaultHostEnv, Runtime, HOST_MODULE};
    use crate::loader::parser::Parser;
    use crate::tests::wat2wasm;

    #[test]
    fn simple() {
        let wasm = wat2wasm(format!(
            r#"(module
                   (import "{}" "start" (func $start))
                   (start $start)
               )"#,
            HOST_MODULE
        ))
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = Runtime::new(module, DefaultHostEnv {});
        runtime.start();
    }

    #[test]
    fn instr() {
        let wasm = wat2wasm(format!(
            r#"(module
                   (import "{}" "print" (func $print (param i32)))
                   (func $main
                       i32.const 10
                       i32.const 20
                       i32.add
                       call $print
                   )
                   (start $main)
             )"#,
            HOST_MODULE
        ))
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = Runtime::new(module, DefaultHostEnv {});
        runtime.start();
    }

    #[test]
    fn branch() {
        let wasm = wat2wasm(format!(
            r#"(module
                    (import "{}" "print" (func $print (param i32)))
                    (func $main
                        (block (result i32 i32 i32)
                            i32.const 0
                            (block (result i32 i32)
                                i32.const 1
                                i32.const 10
                                (block (param i32) (result i32)
                                    i32.const 2
                                    i32.add
                                    br 0
                                )
                            )
                         )
                         call $print
                         call $print
                         call $print
                    )
                    (start $main)
                )"#,
            HOST_MODULE
        ))
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = Runtime::new(module, DefaultHostEnv {});
        runtime.start();
    }
}
