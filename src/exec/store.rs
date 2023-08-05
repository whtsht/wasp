use super::memory::{data_active, data_passiv};
use super::runtime::{eval_const, Addr, RuntimeError, PAGE_SIZE};
use super::table::{elem_active, elem_passiv};
use super::value::{Ref, Value};
use crate::binary::FuncType;
use crate::binary::ValType;
use crate::binary::{Data, DataMode, Elem, Limits, Memory, Table};
use crate::binary::{ElemMode, RefType};
use crate::binary::{Global, GlobalType};
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
    pub funcs: Vec<FuncInst>,
    pub globals: Vec<GlobalInst>,
    pub tables: Vec<TableInst>,
    pub elems: Vec<ElemInst>,
    pub mems: Vec<MemInst>,
    pub datas: Vec<DataInst>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            funcs: vec![],
            globals: vec![],
            tables: vec![],
            elems: vec![],
            mems: vec![],
            datas: vec![],
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

    pub fn allocate_global(&mut self, global: Global) -> Result<Addr, RuntimeError> {
        self.globals.push(GlobalInst {
            globaltype: global.type_,
            value: eval_const(&global.value)?,
        });
        Ok(self.globals.len() - 1)
    }

    pub fn allocate_table(&mut self, table: Table) -> Addr {
        let min = table.limits.min() as usize;
        self.tables.push(TableInst {
            tabletype: table,
            elem: vec![Ref::Null; min],
        });
        self.tables.len() - 1
    }

    pub fn allocate_elem(&mut self, elem: Elem) -> Result<Option<Addr>, RuntimeError> {
        match &elem.mode {
            ElemMode::Passiv => {
                elem_passiv(&mut self.elems, elem)?;
                Ok(Some(self.elems.len() - 1))
            }
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
        });
        self.mems.len() - 1
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
}
