#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::host_env::DebugHostEnv;
use super::host_env::HostEnv;
use super::importer::DefaultImporter;
use super::importer::Importer;
use super::stack::{Frame, Label, Stack, Value};
use super::trap::Trap;
use crate::binary::{Block, Export};
use crate::binary::{ExportDesc, Func, FuncType, ImportDesc, Instr, Module};
use alloc::rc::Rc;

pub type Addr = usize;

pub const HOST_MODULE: &str = "__env";

#[derive(Debug)]
pub enum ExecState {
    Breaking(u32),
    Continue,
    Return,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Instance {
    funcaddrs: Vec<Addr>,
    types: Vec<FuncType>,
    start: Option<usize>,
    exports: Vec<Export>,
    stack: Stack,
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
    funcs: Vec<Rc<FuncInst>>,
}

pub trait Allocatable {
    fn allocate(store: &mut Store, value: Self) -> Addr;
}

impl Allocatable for FuncInst {
    fn allocate(store: &mut Store, value: Self) -> Addr {
        store.funcs.push(Rc::new(value));
        store.funcs.len() - 1
    }
}

impl Store {
    pub fn new() -> Self {
        Self { funcs: vec![] }
    }

    pub fn update_func_inst(&mut self, instance: Rc<Instance>) {
        self.funcs
            .iter_mut()
            .for_each(|f| match Rc::get_mut(f).unwrap() {
                FuncInst::InnerFunc { instance: i, .. } => {
                    *i = instance.clone();
                }
                _ => {}
            });
    }

    pub fn allocate<T: Allocatable>(&mut self, value: T) -> Addr {
        Allocatable::allocate(self, value)
    }
}

use core::fmt::Debug;
#[derive(Debug)]
pub struct Runtime<E: HostEnv + Debug, I: Importer + Debug> {
    instance: Instance,
    store: Store,
    importer: I,
    env: E,
}

#[derive(Debug)]
pub enum RuntimeError {
    ModuleNotFound,
    Trap(Trap),
}

#[cfg(feature = "std")]
pub fn debug_runtime(
    module: Module,
) -> Result<Runtime<DebugHostEnv, DefaultImporter>, RuntimeError> {
    let mut dummy = Runtime {
        instance: Instance::default(),
        store: Store::new(),
        importer: DefaultImporter::new(),
        env: DebugHostEnv {},
    };
    let instance = dummy.new_instance(module)?;

    Ok(Runtime {
        instance,
        store: dummy.store,
        importer: dummy.importer,
        env: dummy.env,
    })
}

impl<E: HostEnv + Debug, I: Importer + Debug> Runtime<E, I> {
    pub fn new(importer: I, env: E, module: Module) -> Result<Self, RuntimeError> {
        let mut dummy = Self {
            instance: Instance::default(),
            store: Store::new(),
            importer,
            env,
        };
        let instance = dummy.new_instance(module)?;

        Ok(Self {
            instance,
            store: dummy.store,
            importer: dummy.importer,
            env: dummy.env,
        })
    }

    pub fn new_instance(&mut self, module: Module) -> Result<Instance, RuntimeError> {
        let mut funcs = vec![];

        for import in module.imports.into_iter() {
            match import.desc {
                ImportDesc::TypeIdx(idx) => match import.module.as_str() {
                    HOST_MODULE => {
                        let addr = self.store.allocate(FuncInst::HostFunc {
                            functype: module.types[idx as usize].clone(),
                            name: import.name,
                        });
                        funcs.push(addr);
                    }
                    modname => funcs.push(self.get_func_addr(modname, &import.name)?),
                },
                _ => {}
            }
        }

        for func in module.funcs {
            funcs.push(self.store.allocate(FuncInst::InnerFunc {
                functype: module.types[func.typeidx as usize].clone(),
                instance: Rc::new(Instance::default()),
                func: Rc::new(func),
            }))
        }

        Ok(Instance {
            funcaddrs: funcs,
            types: module.types,
            start: module.start.map(|idx| idx as usize),
            exports: module.exports,
            stack: Stack::new(),
        })
    }

    pub fn get_func_addr(&mut self, modname: &str, funcname: &str) -> Result<usize, RuntimeError> {
        let module = self
            .importer
            .import(modname)
            .ok_or_else(|| RuntimeError::ModuleNotFound)?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|export| export.name == funcname)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Func(index) = desc {
                return Ok(instance.funcaddrs[*index as usize]);
            } else {
                panic!("expected function, found {:?}", desc);
            }
        } else {
            panic!("a function named {}.{} was not found", modname, funcname)
        }
    }

    pub fn start(&mut self) {
        if let Some(index) = self.instance.start {
            match self.store.funcs[index].clone().as_ref() {
                FuncInst::HostFunc { name, .. } => self.env.call(&name, &mut self.instance.stack),
                FuncInst::InnerFunc { func, .. } => match self.exec(&func.as_ref().body.0) {
                    Ok(_) => {}
                    Err(trap) => println!("RuntimeError: {}", trap),
                },
            }
        }
    }

    pub fn invoke(&mut self, name: &str, params: Vec<Value>) -> Result<Vec<Value>, Trap> {
        if let Some(export) = self
            .instance
            .exports
            .iter()
            .filter(|export| &export.name == name)
            .next()
        {
            if let ExportDesc::Func(index) = export.desc {
                self.instance.stack.set_params(params);
                match self.store.funcs[self.instance.funcaddrs[index as usize]]
                    .clone()
                    .as_ref()
                {
                    FuncInst::HostFunc { name, .. } => {
                        self.env.call(&name, &mut self.instance.stack)
                    }
                    FuncInst::InnerFunc { func, .. } => {
                        self.exec(&func.as_ref().body.0)?;
                    }
                }
                Ok(self.instance.stack.get_returns())
            } else {
                panic!("Error: {} is not a function", name);
            }
        } else {
            panic!("Error: A function named {} was not found", name);
        }
    }

    pub fn exec(&mut self, instrs: &Vec<Instr>) -> Result<ExecState, Trap> {
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

    pub fn binary_op<F: Fn(T, T) -> T, T: From<Value> + Into<Value>>(&mut self, func: F) {
        let lhs = self.instance.stack.pop_value::<T>();
        let rhs = self.instance.stack.pop_value::<T>();
        self.instance.stack.push_value(func(lhs, rhs));
    }

    pub fn block_to_arity(&self, bt: &Block) -> usize {
        match bt {
            Block::Empty => 0,
            Block::ValType(_) => 1,
            Block::TypeIdx(idx) => self.instance.types[*idx as usize].1 .0.len(),
        }
    }

    pub fn jump(&mut self, l: usize) {
        let label = self.instance.stack.th_label(l);
        let mut values: Vec<Value> = vec![];
        for _ in 0..label.n {
            values.push(self.instance.stack.pop_value());
        }

        let len = self.instance.stack.values_len() - label.offset;
        for _ in 0..len {
            self.instance.stack.pop_value::<Value>();
        }

        for _ in 0..=l {
            self.instance.stack.pop_label();
        }

        for value in values.into_iter().rev() {
            self.instance.stack.push_value(value);
        }
    }

    pub fn step(&mut self, instrs: &Vec<Instr>, next: usize) -> Result<ExecState, Trap> {
        match &instrs[next] {
            Instr::I32Const(a) => self.instance.stack.push_value(*a),
            Instr::I32Add => self.binary_op(|a: i32, b: i32| a + b),
            Instr::Nop => {}
            Instr::Unreachable => return Err(Trap::Unreachable),
            Instr::Block { in1, bt } => {
                self.instance.stack.push_label(Label {
                    n: self.block_to_arity(bt),
                    offset: self.instance.stack.values_len(),
                });
                match self.exec(in1)? {
                    ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                    _ => {}
                }
            }
            Instr::Loop { in1, .. } => loop {
                match self.exec(in1)? {
                    ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                    ExecState::Return => return Ok(ExecState::Return),
                    _ => {}
                }
            },
            Instr::If { in1, in2, .. } => {
                let c = self.instance.stack.pop_value::<i32>();
                if c != 0 {
                    match self.exec(in1)? {
                        ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                        ExecState::Return => return Ok(ExecState::Return),
                        _ => {}
                    }
                } else if let Some(in2) = in2 {
                    match self.exec(in2)? {
                        ExecState::Breaking(l) if l > 0 => {
                            return Ok(ExecState::Breaking(l - 1));
                        }
                        ExecState::Return => return Ok(ExecState::Return),
                        _ => {}
                    }
                }
            }
            Instr::Br(l) => {
                self.jump(*l as usize);
                return Ok(ExecState::Breaking(*l));
            }
            Instr::BrIf(l) => {
                let c = self.instance.stack.pop_value::<i32>();
                if c != 0 {
                    self.jump(*l as usize);
                    return Ok(ExecState::Breaking(*l));
                }
            }
            Instr::BrTable { indexs, default } => {
                let i = self.instance.stack.pop_value::<i32>() as usize;
                return if i <= indexs.len() {
                    self.jump(indexs[i] as usize);
                    Ok(ExecState::Breaking(indexs[i]))
                } else {
                    self.jump(*default as usize);
                    Ok(ExecState::Breaking(*default))
                };
            }
            Instr::Return => return Ok(ExecState::Return),
            Instr::Call(a) => {
                let func = self.store.funcs[*a as usize].clone();
                match func.as_ref() {
                    FuncInst::HostFunc { name, .. } => {
                        self.env.call(name, &mut self.instance.stack);
                    }
                    FuncInst::InnerFunc {
                        functype,
                        instance,
                        func,
                    } => {
                        let mut local = vec![];
                        for _ in 0..functype.0 .0.len() {
                            local.push(self.instance.stack.pop_value());
                        }
                        let frame = Frame {
                            instance: instance.clone(),
                            local,
                        };
                        self.instance.stack.push_frame(frame);
                        self.exec(&func.body.0)?;
                    }
                }
            }
            Instr::LocalGet(l) => {
                let frame = self.instance.stack.top_frame();
                let value = frame.local[*l as usize];
                self.instance.stack.push_value(value);
            }
            _ => return Err(Trap::NotImplemented),
        }
        Ok(ExecState::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::{Runtime, HOST_MODULE};
    use crate::exec::host_env::DebugHostEnv;
    use crate::exec::importer::DefaultImporter;
    use crate::exec::runtime::debug_runtime;
    use crate::exec::stack::Value;
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
        let mut runtime = debug_runtime(module).unwrap();
        runtime.start();
    }

    #[test]
    fn instr() {
        let wasm = wat2wasm(
            r#"(module
                   (func (export "main") (result i32)
                       i32.const 10
                       i32.const 20
                       i32.add
                   )
             )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(30)]))
    }

    #[test]
    fn branch() {
        let wasm = wat2wasm(
            r#"(module
                    (func (export "main") (result i32 i32 i32)
                        (block (result i32 i32 i32)
                            i32.const 0
                            (block (result i32 i32)
                                i32.const 1
                                (block (param i32) (result i32)
                                    i32.const 2
                                    i32.add
                                    i32.const 5
                                    i32.const 6
                                    br 2
                                )
                                i32.const 10
                            )
                         )
                    )
                )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(
            runtime.invoke("main", vec![]),
            Ok(vec![Value::I32(3), Value::I32(5), Value::I32(6)])
        );
    }

    #[test]
    fn call_func() {
        let wasm = wat2wasm(
            r#"(module
                (func $triple (result i32 i32 i32)
                    (block (result i32 i32 i32)
                        i32.const 0
                        (block (result i32 i32)
                            i32.const 1
                            (block (param i32) (result i32)
                                i32.const 2
                                i32.add
                                i32.const 5
                                i32.const 6
                                br 2
                            )
                            i32.const 10
                        )
                    )
                )
                (func (export "main") (result i32 i32 i32 i32 i32 i32)
                     (block (result i32 i32 i32)
                        i32.const 0
                        (block (result i32 i32)
                            i32.const 1
                            (block (param i32) (result i32)
                                call $triple
                                i32.add
                                br 2
                            )
                            i32.const 10
                        )
                    )
                    call $triple
                )
        )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(
            runtime.invoke("main", vec![]),
            Ok(vec![
                Value::I32(1),
                Value::I32(3),
                Value::I32(11),
                Value::I32(3),
                Value::I32(5),
                Value::I32(6)
            ])
        );
    }

    #[test]
    fn extern_module() {
        let math = Parser::new(
            &wat2wasm(
                r#"(module
                        (func (export "add") (param i32 i32) (result i32)
                            local.get 0
                            local.get 1
                            i32.add
                        )
                    )"#,
            )
            .unwrap(),
        )
        .module()
        .unwrap();

        let main = Parser::new(
            &wat2wasm(
                r#"(module
                        (import "math" "add" (func $add (param i32 i32) (result i32)))
                        (func (export "main") (result i32)
                            i32.const 2
                            i32.const 4
                            call $add
                        )
                    )"#,
            )
            .unwrap(),
        )
        .module()
        .unwrap();

        let mut importer = DefaultImporter::new();
        importer.add_module(math, "math");

        let mut runtime = Runtime::new(importer, DebugHostEnv {}, main).unwrap();
        println!("{:#?}", runtime);
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(6)]));
    }
}
