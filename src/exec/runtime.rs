#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::{Env, EnvError};
use super::importer::Importer;
use super::instr::{attach, step};
use super::stack::Stack;
use super::store::{FuncInst, Store};
use super::trap::Trap;
use super::value::{Ref, Value};
use crate::binary::{Block, Export, Import};
use crate::binary::{ExportDesc, FuncType, ImportDesc, Instr, Module};
use crate::binary::{Expr, ValType};
use core::fmt::Debug;

pub type Addr = usize;
pub const PAGE_SIZE: usize = 65536;

#[derive(Debug, PartialEq, Eq)]
pub enum ExecState {
    Continue,
    Terminate,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Instance {
    pub funcaddrs: Vec<Addr>,
    pub types: Vec<FuncType>,
    pub globaladdrs: Vec<Addr>,
    pub tableaddrs: Vec<Addr>,
    pub elemaddrs: Vec<Addr>,
    // In the current version of WebAssembly, all memory instructions
    // implicitly operate on memory index 0. This restriction may be
    // lifted in future versions.
    pub memaddr: Option<Addr>,
    pub dataaddrs: Vec<Addr>,
    pub start: Option<usize>,
    pub exports: Vec<Export>,
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

#[derive(Debug)]
pub struct Runtime<E: Env + Debug, I: Importer + Debug> {
    env_name: String,
    instrs: Vec<Instr>,
    instances: Vec<Instance>,
    root: usize,
    stack: Stack,
    pc: usize,
    store: Store,
    importer: I,
    env: E,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    ModuleNotFound(String),
    NotFound(ImportType),
    Env(EnvError),
    ConstantExpression,
    Trap(Trap),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImportType {
    Func(String),
    Table(String),
    Global(String),
    Mem,
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

pub fn eval_const(expr: &Expr) -> Result<Value, RuntimeError> {
    Ok(match expr.0[0] {
        Instr::I32Const(value) => Value::I32(value),
        Instr::I64Const(value) => Value::I64(value),
        Instr::F32Const(value) => Value::F32(value),
        Instr::F64Const(value) => Value::F64(value),
        Instr::RefNull(_) => Value::Ref(Ref::Null),
        Instr::RefFunc(idx) => Value::I32(idx as i32),
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

    pub fn without_module<S: Into<String>>(importer: I, env: E, env_name: S) -> Self {
        Runtime {
            root: 0,
            instrs: vec![],
            instances: vec![],
            store: Store::new(),
            importer,
            env,
            env_name: env_name.into(),
            stack: Stack::new(),
            pc: 0,
        }
    }

    pub fn resister_module(&mut self, module: Module) -> Result<(), RuntimeError> {
        let instance = self.new_instance(module)?;

        self.instances.push(instance);

        self.root = self.instances.len() - 1;
        Ok(())
    }

    fn new_instance(&mut self, module: Module) -> Result<Instance, RuntimeError> {
        let mut funcaddrs = vec![];
        let mut globaladdrs = vec![];
        let mut tableaddrs = vec![];
        let mut memaddr = None;

        for import in module.imports {
            if import.module == self.env_name {
                match import.desc {
                    ImportDesc::Func(ty) => funcaddrs
                        .push(self.import_env_func(module.types[ty as usize].clone(), import.name)),
                    ImportDesc::Table(_) => {}
                    ImportDesc::Mem(_) => {}
                    ImportDesc::Global(_) => {}
                }
            } else {
                match import.desc {
                    ImportDesc::Func(_) => funcaddrs.push(self.import_func(&import)?),
                    ImportDesc::Mem(_) => memaddr = Some(self.import_memory(&import)?),
                    ImportDesc::Table(_) => tableaddrs.push(self.import_table(&import)?),
                    ImportDesc::Global(_) => globaladdrs.push(self.import_global(&import)?),
                }
            }
        }

        for global in module.globals {
            globaladdrs.push(self.store.allocate_global(global)?);
        }

        for table in module.tables {
            tableaddrs.push(self.store.allocate_table(table));
        }

        let mut elemaddrs = vec![];
        for elem in module.elems {
            if let Some(addr) = self.store.allocate_elem(elem)? {
                elemaddrs.push(addr);
            }
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

        if module.mems.len() > 0 {
            memaddr = Some(self.store.allocate_mem(&module.mems[0]))
        }

        let mut dataaddrs = vec![];
        for data in module.datas {
            if let Some(addr) = self.store.allocate_data(data)? {
                dataaddrs.push(addr);
            }
        }

        Ok(Instance {
            funcaddrs,
            types: module.types,
            globaladdrs,
            tableaddrs,
            elemaddrs,
            memaddr,
            dataaddrs,
            start: module.start.map(|idx| idx as usize),
            exports: module.exports,
        })
    }

    pub fn import_env_func(&mut self, functype: FuncType, name: String) -> Addr {
        self.store.funcs.push(FuncInst::HostFunc { functype, name });
        self.store.funcs.len() - 1
    }

    pub fn import_func(&mut self, import: &Import) -> Result<usize, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|export| export.name == import.name)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Func(index) = desc {
                let ret = Ok(instance.funcaddrs[*index as usize]);
                self.instances.push(instance);
                return ret;
            }
        }
        Err(RuntimeError::NotFound(ImportType::Func(
            import.name.clone(),
        )))
    }

    pub fn import_memory(&mut self, import: &Import) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|e| e.name == import.name)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Mem(_) = desc {
                if let Some(addr) = instance.memaddr {
                    self.instances.push(instance);
                    return Ok(addr);
                }
            }
        }
        Err(RuntimeError::NotFound(ImportType::Mem))
    }

    pub fn import_table(&mut self, import: &Import) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|export| export.name == import.name)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Table(addr) = desc {
                let ret = Ok(instance.tableaddrs[*addr as usize]);
                self.instances.push(instance);
                return ret;
            }
        }
        Err(RuntimeError::NotFound(ImportType::Table(
            import.name.clone(),
        )))
    }

    pub fn import_global(&mut self, import: &Import) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(module)?;
        if let Some(desc) = instance
            .exports
            .iter()
            .filter(|export| export.name == import.name)
            .map(|export| &export.desc)
            .next()
        {
            if let ExportDesc::Global(addr) = desc {
                let ret = Ok(instance.globaladdrs[*addr as usize]);
                self.instances.push(instance);
                return ret;
            }
        }
        Err(RuntimeError::NotFound(ImportType::Global(
            import.name.clone(),
        )))
    }

    pub fn start(&mut self) -> Result<(), RuntimeError> {
        let instance = &self.instances[self.root];
        self.stack = Stack::new();
        if let Some(index) = instance.start {
            let func = &self.store.funcs[index];
            let memory = instance.memaddr.map(|a| &mut self.store.mems[a]);
            if let Some(start) = attach(func, &mut self.stack, memory, &mut self.env, self.pc)
                .map_err(|trap| RuntimeError::Trap(trap))?
            {
                self.exec(start).map_err(|trap| RuntimeError::Trap(trap))?;
            }
        }
        Ok(())
    }

    pub fn invoke(&mut self, name: &str, params: Vec<Value>) -> Result<Vec<Value>, RuntimeError> {
        let instance = &self.instances[self.root];
        self.stack = Stack::new();
        if let Some(export) = instance
            .exports
            .iter()
            .filter(|export| &export.name == name)
            .next()
        {
            match export.desc {
                ExportDesc::Func(index) => {
                    let func = &self.store.funcs[instance.funcaddrs[index as usize]];
                    let memory = instance.memaddr.map(|a| &mut self.store.mems[a]);
                    self.stack.extend_values(params);
                    if let Some(start) =
                        attach(func, &mut self.stack, memory, &mut self.env, self.pc)
                            .map_err(|trap| RuntimeError::Trap(trap))?
                    {
                        self.exec(start).map_err(|trap| RuntimeError::Trap(trap))
                    } else {
                        Ok(self.stack.get_returns())
                    }
                }
                _ => Err(RuntimeError::NotFound(ImportType::Func(name.into()))),
            }
        } else {
            Err(RuntimeError::NotFound(ImportType::Func(name.into())))
        }
    }

    pub fn exec(&mut self, start: usize) -> Result<Vec<Value>, Trap> {
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

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::exec::env::{DebugEnv, Env};
    use crate::exec::importer::DefaultImporter;
    use crate::exec::runtime::debug_runtime;
    use crate::exec::value::Value;
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

    #[test]
    fn table() {
        let wasm = wat2wasm(
            r#"(module
                    (table 2 anyfunc)
                    (func $f1 (result i32) i32.const 42)
                    (func $f2 (result i32) i32.const 13)
                    (type $return_i32 (func (result i32)))
                    (elem (i32.const 0) $f1 $f2)

                    (func (export "main") (result i32 i32)
                        i32.const 1
                        call_indirect (type $return_i32)
                        i32.const 0
                        call_indirect (type $return_i32)
                    )
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(
            runtime.invoke("main", vec![]),
            Ok(vec![Value::I32(13), Value::I32(42)])
        );
    }

    #[test]
    fn memory() {
        let wasm = wat2wasm(
            r#"(module
                    ;; print(offset, length)
                    (import "env" "print" (func $print (param i32 i32)))
                    (memory 1)
                    (export "memory" (memory 0))
                    (data (i32.const 0) "hello world\n")
                    (func (export "main")
                        (call $print
                            (i32.const 0)
                            (i32.const 12)
                        )
                    )
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        #[derive(Debug)]
        struct TestEnv {}
        impl Env for TestEnv {
            fn call(
                &mut self,
                name: &str,
                params: Vec<Value>,
                memory: Option<&mut crate::exec::store::MemInst>,
            ) -> Result<Vec<Value>, crate::exec::env::EnvError> {
                if name == "print" {
                    let offset = i32::from(params[0]) as usize;
                    let length = i32::from(params[1]) as usize;
                    let memory = memory.as_ref().unwrap();
                    assert_eq!(&memory.data[offset..(offset + length)], b"hello world\n");
                }
                Ok(vec![])
            }
        }
        let mut runtime = Runtime::new(DefaultImporter::new(), TestEnv {}, "env", module).unwrap();

        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![]));

        let wasm = wat2wasm(
            r#"(module
                    (memory 1)
                    (global $x (mut i32) (i32.const -12))
                    (func (export "main") (result i32)
                       (memory.grow (memory.grow (i32.const 0)))
                    )
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(1)]));
    }

    #[test]
    fn global() {
        let wasm = wat2wasm(
            r#"(module
                    (global $x (mut i32) (i32.const -12))
                    (func (export "set-x") (param i32) (global.set $x (local.get 0)))
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("set-x", vec![Value::I32(6)]), Ok(vec![]));
    }

    #[test]
    fn loop1() {
        let wasm = wat2wasm(
            r#"(module
                  (memory 1)
                  (func (export "main") (result i32)
                       i32.const 2
                       memory.grow
                       i32.const 0
                       memory.grow
                  )

                  )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        let module = parser.module().unwrap();
        let mut runtime = debug_runtime(module).unwrap();
        assert_eq!(runtime.invoke("main", vec![]), Ok(vec![Value::I32(3)]));
    }
}
