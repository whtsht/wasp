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
    pub modname: String,
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
pub struct Runtime<E: Env, I: Importer> {
    env_name: String,
    instrs: Vec<Instr>,
    instances: Vec<Instance>,
    root: usize,
    stack: Stack,
    pc: usize,
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

impl<E: Env, I: Importer> Runtime<E, I> {
    pub fn allocate_func(
        &mut self,
        functype: FuncType,
        locals: Vec<ValType>,
        instrs: Vec<Instr>,
        instance_addr: Addr,
        store: &mut Store,
    ) -> Addr {
        let start = self.instrs.len();
        self.instrs.extend(instrs);
        self.instrs.extend(vec![Instr::Return]);
        store.funcs.push(FuncInst::InnerFunc {
            instance_addr,
            start,
            functype,
            locals,
        })
    }

    pub fn instances(self) -> Vec<Instance> {
        self.instances
    }

    pub fn new<S: Into<String>>(
        store: &mut Store,
        importer: I,
        env: E,
        env_name: S,
        module: Module,
        modname: &str,
    ) -> Result<Self, RuntimeError> {
        let mut runtime = Runtime {
            root: 0,
            instrs: vec![],
            instances: vec![],
            importer,
            env,
            env_name: env_name.into(),
            stack: Stack::new(),
            pc: 0,
        };

        let instance = runtime.new_instance(store, module, modname)?;
        runtime.instances.push(instance);
        runtime.root = runtime.instances.len() - 1;

        Ok(runtime)
    }

    pub fn without_module<S: Into<String>>(importer: I, env: E, env_name: S) -> Self {
        Runtime {
            root: 0,
            instrs: vec![],
            instances: vec![],
            importer,
            env,
            env_name: env_name.into(),
            stack: Stack::new(),
            pc: 0,
        }
    }

    pub fn resister_module(
        &mut self,
        store: &mut Store,
        modname: &str,
    ) -> Result<(), RuntimeError> {
        let module = self
            .importer
            .import(modname)
            .ok_or(RuntimeError::ModuleNotFound(modname.into()))?;
        let instance = self.new_instance(store, module, modname)?;

        self.instances.push(instance);

        self.root = self.instances.len() - 1;
        Ok(())
    }

    fn new_instance(
        &mut self,
        store: &mut Store,
        module: Module,
        modname: &str,
    ) -> Result<Instance, RuntimeError> {
        let mut funcaddrs = vec![];
        let mut globaladdrs = vec![];
        let mut tableaddrs = vec![];
        let mut memaddr = None;

        for import in module.imports {
            if import.module == self.env_name {
                match import.desc {
                    ImportDesc::Func(ty) => funcaddrs.push(self.import_env_func(
                        store,
                        module.types[ty as usize].clone(),
                        import.name,
                    )),
                    ImportDesc::Table(_) => {}
                    ImportDesc::Mem(_) => {}
                    ImportDesc::Global(_) => {}
                }
            } else {
                match import.desc {
                    ImportDesc::Func(_) => funcaddrs.push(self.import_func(store, &import)?),
                    ImportDesc::Mem(_) => memaddr = Some(self.import_memory(store, &import)?),
                    ImportDesc::Table(_) => tableaddrs.push(self.import_table(store, &import)?),
                    ImportDesc::Global(_) => globaladdrs.push(self.import_global(store, &import)?),
                }
            }
        }

        for global in module.globals {
            globaladdrs.push(store.allocate_global(global)?);
        }

        for table in module.tables {
            tableaddrs.push(store.allocate_table(table));
        }

        let mut elemaddrs = vec![];
        for elem in module.elems {
            if let Some(addr) = store.allocate_elem(elem)? {
                elemaddrs.push(addr);
            }
        }

        let mut inner_funcaddr = vec![];
        for func in module.funcs {
            let functype = module.types[func.typeidx as usize].clone();
            let addr = self.allocate_func(
                functype,
                func.locals,
                func.body.0,
                self.instances.len(),
                store,
            );
            inner_funcaddr.push(addr);
            funcaddrs.push(addr);
        }
        let instance_addr = self.instances.len();
        store.update_func_inst(&inner_funcaddr, instance_addr);

        if module.mems.len() > 0 {
            memaddr = Some(store.allocate_mem(&module.mems[0]))
        }

        let mut dataaddrs = vec![];
        for data in module.datas {
            if let Some(addr) = store.allocate_data(data)? {
                dataaddrs.push(addr);
            }
        }

        Ok(Instance {
            modname: modname.into(),
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

    pub fn import_env_func(&mut self, store: &mut Store, functype: FuncType, name: String) -> Addr {
        store.funcs.push(FuncInst::HostFunc { functype, name })
    }

    pub fn import_func(
        &mut self,
        store: &mut Store,
        import: &Import,
    ) -> Result<usize, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(store, module, &import.module)?;
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

    pub fn import_memory(
        &mut self,
        store: &mut Store,
        import: &Import,
    ) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(store, module, &import.module)?;
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

    pub fn import_table(
        &mut self,
        store: &mut Store,
        import: &Import,
    ) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(store, module, &import.module)?;
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

    pub fn import_global(
        &mut self,
        store: &mut Store,
        import: &Import,
    ) -> Result<Addr, RuntimeError> {
        let module = self
            .importer
            .import(&import.module)
            .ok_or_else(|| RuntimeError::ModuleNotFound(import.module.clone()))?;
        let instance = self.new_instance(store, module, &import.module)?;
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

    pub fn start(&mut self, store: &mut Store) -> Result<(), RuntimeError> {
        let instance = &self.instances[self.root];
        self.stack = Stack::new();
        if let Some(index) = instance.start {
            let func = &store.funcs[index];
            let memory = instance.memaddr.map(|a| &mut store.mems[a]);
            if let Some(start) = attach(func, &mut self.stack, memory, &mut self.env, self.pc)
                .map_err(|trap| RuntimeError::Trap(trap))?
            {
                self.exec(store, start)
                    .map_err(|trap| RuntimeError::Trap(trap))?;
            }
        }
        Ok(())
    }

    pub fn invoke(
        &mut self,
        store: &mut Store,
        name: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Value>, RuntimeError> {
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
                    let func = &store.funcs[instance.funcaddrs[index as usize]];
                    let memory = instance.memaddr.map(|a| &mut store.mems[a]);
                    self.stack.extend_values(params);
                    if let Some(start) =
                        attach(func, &mut self.stack, memory, &mut self.env, self.pc)
                            .map_err(|trap| RuntimeError::Trap(trap))?
                    {
                        self.exec(store, start)
                            .map_err(|trap| RuntimeError::Trap(trap))
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

    pub fn exec(&mut self, store: &mut Store, start: usize) -> Result<Vec<Value>, Trap> {
        self.pc = start;
        while self.step(store)? == ExecState::Continue {}
        Ok(self.stack.get_returns())
    }

    pub fn step(&mut self, store: &mut Store) -> Result<ExecState, Trap> {
        if let Some(new_pc) = step(
            &mut self.env,
            &mut self.instances,
            &self.instrs,
            self.pc,
            store,
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
    use crate::binary::Module;
    use crate::exec::env::DebugEnv;
    use crate::exec::importer::Importer;
    use crate::exec::store::Store;
    use crate::exec::value::Value;
    use crate::loader::parser::Parser;
    use crate::tests::wat2wasm;

    #[test]
    fn store() {
        let wasm = wat2wasm(
            r#"(module
                  (memory 1)
                  (global $x (mut i32) (i32.const -12))
                  (table 2 anyfunc)
                  (func $f1 (result i32) i32.const 42)
                  (func $f2 (result i32) i32.const 13)
                  (elem (i32.const 0) $f1 $f2)
                  (data (i32.const 0) "hello world\n")

                  (func (export "main") (result i32)
                      i32.const 3
                  )
                  )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);

        #[derive(Debug)]
        struct TestImporter {
            module: Module,
        }
        impl Importer for TestImporter {
            fn import(&mut self, modname: &str) -> Option<crate::binary::Module> {
                if modname == "debug" {
                    Some(self.module.clone())
                } else {
                    None
                }
            }
        }
        let mut store = Store::new();

        let module = parser.module().unwrap();
        let impoter = TestImporter { module };
        let mut runtime = Runtime::without_module(impoter, DebugEnv {}, "env");
        runtime.resister_module(&mut store, "debug").unwrap();
        assert_eq!(
            runtime.invoke(&mut store, "main", vec![]),
            Ok(vec![Value::I32(3)])
        );
        store.free_runtime(runtime);
        assert_eq!(store.funcs.to_vec().len(), 0);
        assert_eq!(store.elems.to_vec().len(), 0);
        assert_eq!(store.datas.to_vec().len(), 0);
        assert_eq!(store.globals.to_vec().len(), 0);
        assert_eq!(store.mems.to_vec().len(), 0);
        assert_eq!(store.tables.to_vec().len(), 0);
    }
}
