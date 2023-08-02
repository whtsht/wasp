#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::{Env, EnvError};
use super::importer::Importer;
use super::stack::{Frame, Label, Stack, Value};
use super::trap::Trap;
use crate::binary::{Block, Export};
use crate::binary::{ExportDesc, FuncType, ImportDesc, Instr, Module};
use crate::binary::{Expr, ValType};
use crate::binary::{Global, GlobalType};

pub type Addr = usize;

#[derive(Debug, PartialEq, Eq)]
pub enum ExecState {
    Continue,
    Terminate,
}

impl Stack {
    pub fn binary_op<F: Fn(T, T) -> T, T: From<Value> + Into<Value>>(&mut self, func: F) {
        let rhs = self.pop_value::<T>();
        let lhs = self.pop_value::<T>();
        let r = func(lhs, rhs);
        self.push_value(r);
    }

    pub fn rel_op<F: Fn(T, T) -> T, T: From<Value> + Into<Value>>(&mut self, func: F) {
        let rhs = self.pop_value::<T>();
        let lhs = self.pop_value::<T>();
        let r = func(lhs, rhs);
        self.push_value(r);
    }

    pub fn jump(&mut self, l: usize) -> usize {
        let label = self.th_label(l);
        let mut values: Vec<Value> = vec![];
        for _ in 0..label.n {
            values.push(self.pop_value());
        }

        let len = self.values_len() - label.offset;
        for _ in 0..len {
            self.pop_value::<Value>();
        }

        for _ in 0..=l {
            self.pop_label();
        }

        for value in values.into_iter().rev() {
            self.push_value(value);
        }
        label.pc
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Instance {
    funcaddrs: Vec<Addr>,
    globaladdrs: Vec<Addr>,
    types: Vec<FuncType>,
    start: Option<usize>,
    exports: Vec<Export>,
}

impl Instance {
    pub fn block_to_arity(&self, bt: &Block) -> usize {
        match bt {
            Block::Empty => 0,
            Block::ValType(_) => 1,
            Block::TypeIdx(idx) => self.types[*idx as usize].1 .0.len(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum FuncInst {
    InnerFunc {
        instance_addr: Addr,
        start: usize,
        functype: FuncType,
        locals: Vec<ValType>,
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

    pub fn allocate_env_func(&mut self, functype: FuncType, name: String) -> Addr {
        self.funcs.push(FuncInst::HostFunc { functype, name });
        self.funcs.len() - 1
    }

    pub fn allocate_global(&mut self, global: Global) -> Result<Addr, RuntimeError> {
        self.globals.push(GlobalInst {
            globaltype: global.type_,
            value: eval_const(global.value)?,
        });
        Ok(self.globals.len() - 1)
    }
}

use core::fmt::Debug;
#[derive(Debug)]
pub struct Runtime<E: Env + Debug, I: Importer + Debug> {
    env_name: String,
    instrs: Vec<Instr>,
    root: usize,
    instances: Vec<Instance>,
    store: Store,
    stack: Stack,
    importer: I,
    env: E,
    pc: usize,
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
        instrs: vec![],
        env_name: "env".into(),
        instances: vec![],
        store: Store::new(),
        importer: DefaultImporter::new(),
        env: DebugEnv {},
        stack: Stack::new(),
        pc: 0,
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
    pub fn allocate_func(
        &mut self,
        functype: FuncType,
        locals: Vec<ValType>,
        instrs: Vec<Instr>,
        instance_addr: Addr,
    ) -> Addr {
        let start = self.instrs.len();
        self.instrs.extend(instrs);
        self.instrs.extend(vec![Instr::Return]);
        self.store.funcs.push(FuncInst::InnerFunc {
            instance_addr,
            start,
            functype,
            locals,
        });
        self.store.funcs.len() - 1
    }

    pub fn new<S: Into<String>>(
        importer: I,
        env: E,
        env_name: S,
        module: Module,
    ) -> Result<Self, RuntimeError> {
        let mut runtime = Runtime {
            root: 0,
            instrs: vec![],
            instances: vec![],
            store: Store::new(),
            importer,
            env,
            env_name: env_name.into(),
            stack: Stack::new(),
            pc: 0,
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
                        let addr = self
                            .store
                            .allocate_env_func(module.types[idx as usize].clone(), import.name);
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
            globaladdrs.push(self.store.allocate_global(global)?);
        }

        let mut inner_funcaddr = vec![];
        for func in module.funcs {
            let functype = module.types[func.typeidx as usize].clone();
            let addr = self.allocate_func(functype, func.locals, func.body.0, self.instances.len());
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
                        pc: 0,
                    };
                    self.env
                        .call(&name, frame)
                        .map_err(|err| RuntimeError::Env(err))?;
                }
                FuncInst::InnerFunc { start, .. } => {
                    let frame = Frame {
                        n: 0,
                        instance_addr: self.root,
                        local: vec![],
                        pc: 0,
                    };

                    self.exec(frame, start)
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
                    let results = match self.store.funcs
                        [self.instances[self.root].funcaddrs[index as usize]]
                        .clone()
                    {
                        FuncInst::HostFunc { name, .. } => {
                            let frame = Frame {
                                n: 0,
                                instance_addr: self.root,
                                local: vec![],
                                pc: 0,
                            };
                            self.env
                                .call(&name, frame)
                                .map_err(|err| RuntimeError::Env(err))?
                        }
                        FuncInst::InnerFunc {
                            start, functype, ..
                        } => {
                            let frame = Frame {
                                n: functype.1 .0.len(),
                                instance_addr: self.root,
                                local: params,
                                pc: 0,
                            };
                            self.exec(frame, start)
                                .map_err(|trap| RuntimeError::Trap(trap))?
                        }
                    };
                    Ok(results)
                }
                _ => Err(RuntimeError::NotFunction(name.into(), export.desc.clone())),
            }
        } else {
            Err(RuntimeError::FunctionNotFound(name.into()))
        }
    }

    pub fn exec(&mut self, frame: Frame, start: usize) -> Result<Vec<Value>, Trap> {
        let mut stack = Stack::new();
        stack.push_frame(frame);
        self.stack = stack;
        self.pc = start;
        while self.step()? == ExecState::Continue {}
        Ok(self.stack.get_returns())
    }

    pub fn step(&mut self) -> Result<ExecState, Trap> {
        if let Some(new_pc) = step(
            &mut self.env,
            &mut self.instances,
            &self.instrs,
            self.pc,
            &mut self.store,
            &mut self.stack,
        )? {
            self.pc = new_pc;
            Ok(ExecState::Continue)
        } else {
            Ok(ExecState::Terminate)
        }
    }
}

pub fn step<E: Env + Debug>(
    env: &mut E,
    instances: &mut Vec<Instance>,
    instrs: &Vec<Instr>,
    pc: usize,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<Option<usize>, Trap> {
    let frame = stack.top_frame().clone();
    let instance = &mut instances[frame.instance_addr];
    match &instrs[pc] {
        Instr::I32Const(a) => stack.push_value(*a),
        Instr::I64Const(a) => stack.push_value(*a),
        Instr::F32Const(a) => stack.push_value(*a),
        Instr::F64Const(a) => stack.push_value(*a),
        Instr::I32Add => stack.binary_op(|a: i32, b: i32| a.wrapping_add(b)),
        Instr::I64Add => stack.binary_op(|a: i64, b: i64| a.wrapping_add(b)),
        Instr::I32Sub => stack.binary_op(|a: i32, b: i32| a.wrapping_sub(b)),
        Instr::I64Sub => stack.binary_op(|a: i64, b: i64| a.wrapping_sub(b)),
        Instr::I32And => stack.binary_op(|a: i32, b: i32| a & b),
        Instr::I64And => stack.binary_op(|a: i64, b: i64| a & b),
        Instr::I64Xor => stack.binary_op(|a: i64, b: i64| a ^ b),
        Instr::I32Mul => stack.binary_op(|a: i32, b: i32| a.wrapping_mul(b)),
        Instr::I64Mul => stack.binary_op(|a: i64, b: i64| a.wrapping_mul(b)),
        Instr::I32DivU => stack.binary_op(|a: i32, b: i32| a / b),
        Instr::I64RemS => stack.binary_op(|a: i64, b: i64| a.wrapping_rem(b)),
        Instr::I32LeS => stack.rel_op(|a: i32, b: i32| if a <= b { 1 } else { 0 }),
        Instr::Nop => {}
        Instr::Drop => {
            stack.pop_value::<Value>();
        }
        Instr::Unreachable => return Err(Trap::Unreachable),
        Instr::Block { bt, end_offset } => {
            stack.push_label(Label {
                n: instance.block_to_arity(bt),
                offset: stack.values_len(),
                pc: end_offset + pc,
            });
        }
        Instr::Loop { bt } => {
            stack.push_label(Label {
                n: instance.block_to_arity(bt),
                offset: stack.values_len(),
                pc,
            });
        }
        Instr::If {
            bt,
            else_offset,
            end_offset,
        } => {
            let c = stack.pop_value::<i32>();
            if c != 0 {
                stack.push_label(Label {
                    n: instance.block_to_arity(bt),
                    offset: stack.values_len(),
                    pc: end_offset + pc,
                });
            } else if let Some(else_offset) = else_offset {
                stack.push_label(Label {
                    n: instance.block_to_arity(bt),
                    offset: stack.values_len(),
                    pc: end_offset + pc,
                });
                return Ok(Some(else_offset + pc));
            } else {
                return Ok(Some(end_offset + pc));
            }
        }
        Instr::Br(l) => {
            let new_pc = stack.jump(*l as usize);
            return Ok(Some(new_pc));
        }
        Instr::BrIf(l) => {
            let c = stack.pop_value::<i32>();
            if c != 0 {
                let new_pc = stack.jump(*l as usize);
                return Ok(Some(new_pc));
            }
        }
        Instr::BrTable { indexs, default } => {
            let i = stack.pop_value::<i32>() as usize;
            return if i <= indexs.len() {
                let new_pc = stack.jump(indexs[i] as usize);
                Ok(Some(new_pc))
            } else {
                let new_pc = stack.jump(*default as usize);
                Ok(Some(new_pc))
            };
        }
        Instr::Return => {
            let n = frame.n;
            let mut results: Vec<Value> = vec![];
            for _ in 0..n {
                results.push(stack.pop_value());
            }
            for _ in 0..n {
                stack.push_value(results.pop().unwrap());
            }
            stack.pop_frame();
            if stack.frames_len() == 0 {
                return Ok(None);
            } else {
                return Ok(Some(frame.pc));
            }
        }
        Instr::Call(a) => {
            let func = store.funcs[*a as usize].clone();
            match func {
                FuncInst::HostFunc { name, functype } => {
                    let mut local = vec![];
                    for _ in 0..functype.0 .0.len() {
                        local.push(stack.pop_value());
                    }
                    let new_frame = Frame {
                        n: functype.1 .0.len(),
                        instance_addr: frame.instance_addr,
                        local,
                        pc: pc + 1,
                    };
                    let results = env
                        .call(name.as_str(), new_frame)
                        .map_err(|err| Trap::Env(err))?;
                    for result in results {
                        stack.push_value(result);
                    }
                }
                FuncInst::InnerFunc {
                    instance_addr,
                    functype,
                    locals,
                    start,
                } => {
                    let mut local = vec![];
                    for _ in 0..functype.0 .0.len() {
                        local.push(stack.pop_value());
                    }
                    for val in locals.iter() {
                        match val {
                            ValType::I32 => local.push(Value::I32(0)),
                            ValType::I64 => local.push(Value::I64(0)),
                            ValType::F32 => local.push(Value::F32(0.0)),
                            ValType::F64 => local.push(Value::F64(0.0)),
                            _ => todo!(),
                        }
                    }
                    let new_frame = Frame {
                        n: functype.1 .0.len(),
                        instance_addr,
                        local,
                        pc: pc + 1,
                    };
                    stack.push_frame(new_frame);
                    return Ok(Some(start));
                }
            }
        }
        Instr::LocalGet(l) => {
            let value = frame.local[*l as usize];
            stack.push_value(value);
        }
        Instr::LocalSet(l) => {
            let value = stack.pop_value();
            stack.top_frame_mut().local[*l as usize] = value;
        }
        Instr::LocalTee(l) => {
            let value: Value = stack.pop_value();
            stack.push_value(value);
            stack.top_frame_mut().local[*l as usize] = value;
        }
        Instr::GlobalGet(i) => {
            let globalindex = instance.globaladdrs[*i as usize];
            stack.push_value(store.globals[globalindex].value);
        }
        Instr::GlobalSet(i) => {
            let value = stack.pop_value();
            let globalindex = instance.globaladdrs[*i as usize];
            store.globals[globalindex].value = value;
        }
        i => return Err(Trap::NotImplemented(format!("{:?}", i))),
    }
    Ok(Some(pc + 1))
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
    fn if_else() {
        let wasm = wat2wasm(
            r#"(module
                    (func (export "main") (result i32)
                        i32.const 0
                        (if
                            (then
                                i32.const 1
                            )
                            (else
                                i32.const 2
                            )
                        )
                    )
                )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(2)]));
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
    fn loop_behavior() {
        let wasm = wat2wasm(
            r#"(module
                    (func $sum_bad (param $n i32) (result i32)  (local $i i32) (local $sum i32)
                        (loop $loop
                          (br_if $loop (i32.le_s (get_local $n) (get_local $i)))
                          (set_local $sum (i32.add (get_local $sum) (get_local $i)))
                          (set_local $i (i32.add (get_local $i) (i32.const 1))))
                        (return (get_local $sum))
                    )
                    (func (export "main") (result i32)
                        i32.const 10
                        call $sum_bad
                    )
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(0)]));

        let wasm = wat2wasm(
            r#"(module
                    (func $sum_good (param $n i32) (result i32)  (local $i i32) (local $sum i32)
                        (loop $loop
                            (set_local $sum (i32.add (get_local $sum) (get_local $i)))
                            (set_local $i (i32.add (get_local $i) (i32.const 1)))
                            (br_if $loop (i32.le_s (get_local $i) (get_local $n)))
                        )
                        (return (get_local $sum))
                    )
                    (func (export "main") (result i32)
                        i32.const 10
                        call $sum_good
                    )
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(55)]));
    }
}
