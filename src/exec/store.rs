use super::env::Env;
use super::importer::Importer;
use super::memory::{data_active, data_passiv};
use super::opt_vec::OptVec;
use super::runtime::{eval_const, Addr, Runtime, RuntimeError, PAGE_SIZE};
use super::table::{elem_active, elem_passiv};
use super::value::{Ref, Value};
use crate::binary::FuncType;
use crate::binary::ValType;
use crate::binary::{Data, DataMode, Elem, Limits, Memory, Table};
use crate::binary::{ElemMode, RefType};
use crate::binary::{Global, GlobalType};
#[cfg(not(feature = "std"))]
use crate::lib::*;
use core::fmt::Debug;

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

impl FuncInst {
    pub fn functype(&self) -> &FuncType {
        match self {
            FuncInst::InnerFunc { functype, .. } | FuncInst::HostFunc { functype, .. } => functype,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct GlobalInst {
    pub globaltype: GlobalType,
    pub value: Value,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TableInst {
    pub tabletype: Table,
    pub elem: Vec<Ref>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ElemInst {
    pub reftype: RefType,
    pub elem: Vec<Ref>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct MemInst {
    pub limits: Limits,
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct DataInst {
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Store {
    pub globals: OptVec<GlobalInst>,
    pub tables: OptVec<TableInst>,
    pub mems: OptVec<MemInst>,
    pub funcs: OptVec<FuncInst>,
    pub elems: OptVec<ElemInst>,
    pub datas: OptVec<DataInst>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            funcs: OptVec::new(),
            globals: OptVec::new(),
            tables: OptVec::new(),
            mems: OptVec::new(),
            elems: OptVec::new(),
            datas: OptVec::new(),
        }
    }

    pub fn update_func_inst(&mut self, funcaddrs: &Vec<Addr>, instance_addr: Addr) {
        for &funcaddr in funcaddrs {
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

    pub fn allocate_global(&mut self, global: Global) -> Result<Addr, RuntimeError> {
        Ok(self.globals.push(GlobalInst {
            globaltype: global.type_,
            value: eval_const(&global.value)?,
        }))
    }

    pub fn allocate_table(&mut self, table: Table) -> Addr {
        let min = table.limits.min() as usize;
        self.tables.push(TableInst {
            tabletype: table,
            elem: vec![Ref::Null; min],
        })
    }

    pub fn allocate_elem(&mut self, elem: Elem) -> Result<Option<Addr>, RuntimeError> {
        match &elem.mode {
            ElemMode::Passiv => Ok(Some(elem_passiv(&mut self.elems, elem)?)),
            ElemMode::Active { tableidx, offset } => {
                let offset = match eval_const(&offset)? {
                    Value::I32(v) => v,
                    _ => unreachable!(),
                } as usize;
                elem_active(&mut self.tables[*tableidx as usize], offset, elem)?;
                Ok(None)
            }
            ElemMode::Declarative => Ok(None),
        }
    }

    pub fn allocate_mem(&mut self, mem: &Memory) -> Addr {
        let min = mem.0.min() as usize;
        self.mems.push(MemInst {
            limits: mem.0.clone(),
            data: vec![0; min * PAGE_SIZE],
        })
    }

    pub fn allocate_data(&mut self, data: Data) -> Result<Option<Addr>, RuntimeError> {
        match &data.mode {
            DataMode::Passive => Ok(Some(data_passiv(&mut self.datas, data))),
            DataMode::Active { memidx, offset } => {
                let offset = match eval_const(&offset)? {
                    Value::I32(v) => v,
                    _ => unreachable!(),
                } as usize;
                data_active(&mut self.mems[*memidx as usize], data, offset);
                Ok(None)
            }
        }
    }

    pub fn free_runtime<E: Env, I: Importer>(&mut self, runtime: Runtime<E, I>) {
        for inst in runtime.instances() {
            for faddr in inst.funcaddrs {
                self.funcs.remove(faddr);
            }
            for daddr in inst.dataaddrs {
                self.datas.remove(daddr);
            }
            for eaddr in inst.elemaddrs {
                self.elems.remove(eaddr);
            }
            for gaddr in inst.globaladdrs {
                self.globals.remove(gaddr);
            }
            for taddr in inst.tableaddrs {
                self.tables.remove(taddr);
            }
            if let Some(maddr) = inst.memaddr {
                self.mems.remove(maddr);
            }
        }
    }
}
