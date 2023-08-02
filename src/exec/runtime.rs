#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::{Env, EnvError};
use super::importer::Importer;
use super::stack::{Frame, Label, Stack, Value};
use super::trap::Trap;
use crate::binary::Expr;
use crate::binary::GlobalType;
use crate::binary::{Block, Export};
use crate::binary::{ExportDesc, Func, FuncType, ImportDesc, Instr, Module};

pub type Addr = usize;

#[derive(Debug)]
pub enum ExecState {
    Breaking(u32),
    Continue,
    Return,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Instance {
    funcaddrs: Vec<Addr>,
    globaladdrs: Vec<Addr>,
    types: Vec<FuncType>,
    start: Option<usize>,
    exports: Vec<Export>,
    stack: Stack,
}

impl Instance {
    pub fn binary_op<F: Fn(T, T) -> T, T: From<Value> + Into<Value> + Debug>(&mut self, func: F) {
        let rhs = self.stack.pop_value::<T>();
        let lhs = self.stack.pop_value::<T>();
        let r = func(lhs, rhs);
        self.stack.push_value(r);
    }

    pub fn block_to_arity(&self, bt: &Block) -> usize {
        match bt {
            Block::Empty => 0,
            Block::ValType(_) => 1,
            Block::TypeIdx(idx) => self.types[*idx as usize].1 .0.len(),
        }
    }

    pub fn jump(&mut self, l: usize) {
        let label = self.stack.th_label(l);
        let mut values: Vec<Value> = vec![];
        for _ in 0..label.n {
            values.push(self.stack.pop_value());
        }

        let len = self.stack.values_len() - label.offset;
        for _ in 0..len {
            self.stack.pop_value::<Value>();
        }

        for _ in 0..=l {
            self.stack.pop_label();
        }

        for value in values.into_iter().rev() {
            self.stack.push_value(value);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FuncInst {
    InnerFunc {
        functype: FuncType,
        instance_addr: Addr,
        func: Func,
    },
    HostFunc {
        functype: FuncType,
        name: String,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlobalInst {
    pub globaltype: GlobalType,
    pub value: Value,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Store {
    funcs: Vec<FuncInst>,
    globals: Vec<GlobalInst>,
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

impl Allocatable for GlobalInst {
    fn allocate(store: &mut Store, value: Self) -> Addr {
        store.globals.push(value);
        store.globals.len() - 1
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            funcs: vec![],
            globals: vec![],
        }
    }

    pub fn update_func_inst(&mut self, funcaddrs: Vec<Addr>, instance_addr: Addr) {
        for funcaddr in funcaddrs {
            match &mut self.funcs[funcaddr as usize] {
                FuncInst::InnerFunc {
                    instance_addr: addr,
                    ..
                } => {
                    *addr = instance_addr;
                }
                _ => {}
            }
        }
    }

    pub fn allocate<T: Allocatable>(&mut self, value: T) -> Addr {
        Allocatable::allocate(self, value)
    }
}

use core::fmt::Debug;
#[derive(Debug)]
pub struct Runtime<E: Env + Debug, I: Importer + Debug> {
    env_name: String,
    root: usize,
    instances: Vec<Instance>,
    store: Store,
    importer: I,
    env: E,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    ModuleNotFound(String),
    FunctionNotFound(String),
    NotFunction(String, ExportDesc),
    Env(EnvError),
    ConstantExpression,
    Trap(Trap),
}

#[cfg(feature = "std")]
use super::env::DebugEnv;
#[cfg(feature = "std")]
use super::importer::DefaultImporter;

#[cfg(feature = "std")]
pub fn debug_runtime(module: Module) -> Result<Runtime<DebugEnv, DefaultImporter>, RuntimeError> {
    let mut runtime = Runtime {
        root: 0,
        env_name: "env".into(),
        instances: vec![],
        store: Store::new(),
        importer: DefaultImporter::new(),
        env: DebugEnv {},
    };

    let instance = runtime.new_instance(module)?;
    runtime.instances.push(instance);

    Ok(runtime)
}

pub fn eval_const(expr: Expr) -> Result<Value, RuntimeError> {
    Ok(match expr.0[0] {
        Instr::I32Const(value) => Value::I32(value),
        Instr::I64Const(value) => Value::I64(value),
        Instr::F32Const(value) => Value::F32(value),
        Instr::F64Const(value) => Value::F64(value),
        _ => return Err(RuntimeError::ConstantExpression),
    })
}

impl<E: Env + Debug, I: Importer + Debug> Runtime<E, I> {
    pub fn new<S: Into<String>>(
        importer: I,
        env: E,
        env_name: S,
        module: Module,
    ) -> Result<Self, RuntimeError> {
        let mut runtime = Runtime {
            root: 0,
            instances: vec![],
            store: Store::new(),
            importer,
            env,
            env_name: env_name.into(),
        };

        let instance = runtime.new_instance(module)?;

        runtime.instances.push(instance);

        runtime.root = runtime.instances.len() - 1;

        Ok(runtime)
    }

    pub fn new_instance(&mut self, module: Module) -> Result<Instance, RuntimeError> {
        let mut funcaddrs = vec![];

        for import in module.imports {
            match import.desc {
                ImportDesc::TypeIdx(idx) => {
                    if import.module == self.env_name {
                        let addr = self.store.allocate(FuncInst::HostFunc {
                            functype: module.types[idx as usize].clone(),
                            name: import.name,
                        });
                        funcaddrs.push(addr);
                    } else {
                        funcaddrs.push(self.get_func_addr(&import.module, &import.name)?);
                    }
                }
                _ => {}
            }
        }

        let mut globaladdrs = vec![];
        for global in module.globals {
            globaladdrs.push(self.store.allocate(GlobalInst {
                globaltype: global.type_,
                value: eval_const(global.value)?,
            }));
        }

        let mut inner_funcaddr = vec![];
        for func in module.funcs {
            let addr = self.store.allocate(FuncInst::InnerFunc {
                functype: module.types[func.typeidx as usize].clone(),
                instance_addr: self.instances.len(),
                func,
            });
            inner_funcaddr.push(addr);
            funcaddrs.push(addr);
        }

        let instance_addr = self.instances.len();
        self.store.update_func_inst(inner_funcaddr, instance_addr);

        Ok(Instance {
            funcaddrs,
            globaladdrs,
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
            .ok_or_else(|| RuntimeError::ModuleNotFound(modname.into()))?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|export| export.name == funcname)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Func(index) = desc {
                let ret = Ok(instance.funcaddrs[*index as usize]);
                self.instances.push(instance);
                return ret;
            } else {
                Err(RuntimeError::NotFunction(funcname.into(), desc.clone()))
            }
        } else {
            Err(RuntimeError::FunctionNotFound(funcname.into()))
        }
    }

    pub fn start(&mut self) -> Result<(), RuntimeError> {
        if let Some(index) = self.instances[self.root].start {
            match self.store.funcs[index].clone() {
                FuncInst::HostFunc { name, .. } => {
                    let frame = Frame {
                        n: 0,
                        instance_addr: self.root,
                        local: vec![],
                    };
                    self.env
                        .call(&name, frame)
                        .map_err(|err| RuntimeError::Env(err))?;
                }
                FuncInst::InnerFunc { func, .. } => {
                    let mut frame = Frame {
                        n: 0,
                        instance_addr: self.root,
                        local: vec![],
                    };

                    exec(
                        &mut self.env,
                        &mut self.instances,
                        &mut self.store,
                        &func.body.0,
                        &mut frame,
                    )
                    .map_err(|trap| RuntimeError::Trap(trap))?;
                }
            }
        }
        Ok(())
    }

    pub fn invoke(&mut self, name: &str, params: Vec<Value>) -> Result<Vec<Value>, RuntimeError> {
        if let Some(export) = self.instances[self.root]
            .exports
            .iter()
            .filter(|export| &export.name == name)
            .next()
        {
            match export.desc {
                ExportDesc::Func(index) => {
                    match self.store.funcs[self.instances[self.root].funcaddrs[index as usize]]
                        .clone()
                    {
                        FuncInst::HostFunc { name, .. } => {
                            let frame = Frame {
                                n: 0,
                                instance_addr: self.root,
                                local: vec![],
                            };
                            self.env
                                .call(&name, frame)
                                .map_err(|err| RuntimeError::Env(err))?;
                        }
                        FuncInst::InnerFunc { func, functype, .. } => {
                            let mut frame = Frame {
                                n: functype.1 .0.len(),
                                instance_addr: self.root,
                                local: params,
                            };
                            exec(
                                &mut self.env,
                                &mut self.instances,
                                &mut self.store,
                                &func.body.0,
                                &mut frame,
                            )
                            .map_err(|trap| RuntimeError::Trap(trap))?;
                        }
                    }
                    Ok(self.instances[self.root].stack.get_returns())
                }
                _ => Err(RuntimeError::NotFunction(name.into(), export.desc.clone())),
            }
        } else {
            Err(RuntimeError::FunctionNotFound(name.into()))
        }
    }
}

pub fn exec<E: Env + Debug>(
    env: &mut E,
    instances: &mut Vec<Instance>,
    store: &mut Store,
    instrs: &Vec<Instr>,
    frame: &mut Frame,
) -> Result<ExecState, Trap> {
    let mut next = 0;
    loop {
        if next >= instrs.len() {
            return Ok(ExecState::Return);
        }
        match step(env, instances, &instrs[next], frame, store)? {
            ExecState::Continue => {}
            ret => return Ok(ret),
        }
        next += 1;
    }
}

pub fn step<E: Env + Debug>(
    env: &mut E,
    instances: &mut Vec<Instance>,
    instr: &Instr,
    frame: &mut Frame,
    store: &mut Store,
) -> Result<ExecState, Trap> {
    let instance = &mut instances[frame.instance_addr];
    match instr {
        Instr::I32Const(a) => instance.stack.push_value(*a),
        Instr::I64Const(a) => instance.stack.push_value(*a),
        Instr::F32Const(a) => instance.stack.push_value(*a),
        Instr::F64Const(a) => instance.stack.push_value(*a),
        Instr::I32Add => instance.binary_op(|a: i32, b: i32| a.wrapping_add(b)),
        Instr::I64Add => instance.binary_op(|a: i64, b: i64| a.wrapping_add(b)),
        Instr::I32Sub => instance.binary_op(|a: i32, b: i32| a.wrapping_sub(b)),
        Instr::I64Sub => instance.binary_op(|a: i64, b: i64| a.wrapping_sub(b)),
        Instr::I32And => instance.binary_op(|a: i32, b: i32| a & b),
        Instr::I64And => instance.binary_op(|a: i64, b: i64| a & b),
        Instr::I64Xor => instance.binary_op(|a: i64, b: i64| a ^ b),
        Instr::I32Mul => instance.binary_op(|a: i32, b: i32| a.wrapping_mul(b)),
        Instr::I64Mul => instance.binary_op(|a: i64, b: i64| a.wrapping_mul(b)),
        Instr::I32DivU => instance.binary_op(|a: i32, b: i32| a / b),
        Instr::I64RemS => instance.binary_op(|a: i64, b: i64| a.wrapping_rem(b)),
        Instr::Nop => {}
        Instr::Drop => {
            instance.stack.pop_value::<Value>();
        }
        Instr::Unreachable => return Err(Trap::Unreachable),
        Instr::Block { in1, bt } => {
            instance.stack.push_label(Label {
                n: instance.block_to_arity(bt),
                offset: instance.stack.values_len(),
            });
            match exec(env, instances, store, in1, frame)? {
                ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                _ => {}
            }
        }
        Instr::Loop { in1, bt } => loop {
            let n = instances[frame.instance_addr].block_to_arity(bt);
            let offset = instances[frame.instance_addr].stack.values_len();
            instances[frame.instance_addr]
                .stack
                .push_label(Label { n, offset });
            match exec(env, instances, store, in1, frame)? {
                ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                ExecState::Return => return Ok(ExecState::Return),
                _ => {}
            }
        },
        Instr::If { in1, in2, .. } => {
            let c = instance.stack.pop_value::<i32>();
            if c != 0 {
                match exec(env, instances, store, in1, frame)? {
                    ExecState::Breaking(l) if l > 0 => return Ok(ExecState::Breaking(l - 1)),
                    ExecState::Return => return Ok(ExecState::Return),
                    _ => {}
                }
            } else if let Some(in2) = in2 {
                match exec(env, instances, store, in2, frame)? {
                    ExecState::Breaking(l) if l > 0 => {
                        return Ok(ExecState::Breaking(l - 1));
                    }
                    ExecState::Return => return Ok(ExecState::Return),
                    _ => {}
                }
            }
        }
        Instr::Br(l) => {
            instance.jump(*l as usize);
            return Ok(ExecState::Breaking(*l));
        }
        Instr::BrIf(l) => {
            let c = instance.stack.pop_value::<i32>();
            if c != 0 {
                instance.jump(*l as usize);
                return Ok(ExecState::Breaking(*l));
            }
        }
        Instr::BrTable { indexs, default } => {
            let i = instance.stack.pop_value::<i32>() as usize;
            return if i <= indexs.len() {
                instance.jump(indexs[i] as usize);
                Ok(ExecState::Breaking(indexs[i]))
            } else {
                instance.jump(*default as usize);
                Ok(ExecState::Breaking(*default))
            };
        }
        Instr::Return => {
            let n = frame.n;
            let mut results: Vec<Value> = vec![];
            for _ in 0..n {
                results.push(instance.stack.pop_value());
            }

            for _ in 0..n {
                instance.stack.push_value(results.pop().unwrap());
            }
            return Ok(ExecState::Return);
        }
        Instr::Call(a) => {
            let func = store.funcs[*a as usize].clone();
            match func {
                FuncInst::HostFunc { name, functype } => {
                    let mut local = vec![];
                    for _ in 0..functype.0 .0.len() {
                        local.push(instance.stack.pop_value());
                    }
                    let new_frame = Frame {
                        n: functype.1 .0.len(),
                        instance_addr: frame.instance_addr,
                        local,
                    };
                    let results = env
                        .call(name.as_str(), new_frame)
                        .map_err(|err| Trap::Env(err))?;

                    for result in results {
                        instance.stack.push_value(result);
                    }
                }
                FuncInst::InnerFunc {
                    functype,
                    instance_addr,
                    func,
                } => {
                    let mut local = vec![];
                    for _ in 0..functype.0 .0.len() {
                        local.push(instance.stack.pop_value());
                    }
                    let mut new_frame = Frame {
                        n: functype.1 .0.len(),
                        instance_addr,
                        local,
                    };
                    exec(env, instances, store, &func.body.0, &mut new_frame)?;

                    if frame.instance_addr != new_frame.instance_addr {
                        unsafe {
                            let origin_instance =
                                core::ptr::addr_of_mut!(instances[frame.instance_addr]);
                            let derived_instance =
                                core::ptr::addr_of_mut!(instances[new_frame.instance_addr]);
                            for result in (*derived_instance).stack.get_returns() {
                                (*origin_instance).stack.push_value(result)
                            }
                        }
                    }
                }
            }
        }
        Instr::LocalGet(l) => {
            let value = frame.local[*l as usize];
            instance.stack.push_value(value);
        }
        Instr::LocalSet(l) => {
            let value = instance.stack.pop_value();
            frame.local[*l as usize] = value;
        }
        Instr::LocalTee(l) => {
            let value: Value = instance.stack.pop_value();
            instance.stack.push_value(value);
            frame.local[*l as usize] = value;
        }
        Instr::GlobalGet(i) => {
            let globalindex = instance.globaladdrs[*i as usize];
            instance.stack.push_value(store.globals[globalindex].value);
        }
        Instr::GlobalSet(i) => {
            let value = instance.stack.pop_value();
            let globalindex = instance.globaladdrs[*i as usize];
            store.globals[globalindex].value = value;
        }
        i => return Err(Trap::NotImplemented(format!("{:?}", i))),
    }
    Ok(ExecState::Continue)
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::exec::env::DebugEnv;
    use crate::exec::importer::DefaultImporter;
    use crate::exec::runtime::debug_runtime;
    use crate::exec::stack::Value;
    use crate::loader::parser::Parser;
    use crate::tests::wat2wasm;

    #[test]
    fn simple() {
        let wasm = wat2wasm(
            r#"(module
                       (import "env" "start" (func $start))
                       (start $start)
                   )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        runtime.start().unwrap();
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

        let mut runtime = Runtime::new(importer, DebugEnv {}, "env", main).unwrap();

        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(6)]));
    }

    #[test]
    fn global_module() {
        let inc = Parser::new(
            &wat2wasm(
                r#"(module
                        (global $v (mut i32) (i32.const 0))
                        (func (export "inc") (result i32)
                            global.get $v
                            i32.const 1
                            i32.add
                            global.set $v
                            global.get $v
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
                        (import "inc" "inc" (func $inc (result i32)))
                        (func (export "main") (result i32 i32 i32 i32 i32)
                             call $inc
                             call $inc
                             call $inc
                             call $inc
                             call $inc
                        )
                    )"#,
            )
            .unwrap(),
        )
        .module()
        .unwrap();

        let mut importer = DefaultImporter::new();
        importer.add_module(inc, "inc");

        let mut runtime = Runtime::new(importer, DebugEnv {}, "env", main).unwrap();
        assert_eq!(
            runtime.invoke("main", vec![]),
            Ok(vec![
                Value::I32(1),
                Value::I32(2),
                Value::I32(3),
                Value::I32(4),
                Value::I32(5)
            ])
        );
    }

    #[test]
    fn host_function() {
        let main = Parser::new(
            &wat2wasm(
                r#"(module
                        (import "inc" "inc" (func $inc (result i32)))
                        (func (export "main") (result i32 i32 i32 i32 i32)
                             call $inc
                             call $inc
                             call $inc
                             call $inc
                             call $inc
                        )
                    )"#,
            )
            .unwrap(),
        )
        .module()
        .unwrap();

        let mut runtime = Runtime::new(DefaultImporter::new(), DebugEnv {}, "env", main).unwrap();
        assert_eq!(
            runtime.invoke("main", vec![]),
            Ok(vec![
                Value::I32(1),
                Value::I32(2),
                Value::I32(3),
                Value::I32(4),
                Value::I32(5)
            ])
        );
    }
}
