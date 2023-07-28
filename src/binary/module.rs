#[cfg(not(feature = "std"))]
use crate::lib::*;

#[cfg(feature = "std")]
use std::vec::IntoIter;

use super::{
    instr::Expr,
    types::{FuncType, GlobalType, Limits, RefType, ValType},
};

pub type TypeIdx = u32;
pub type FuncIdx = u32;
pub type TableIdx = u32;
pub type MemIdx = u32;
pub type GlobalIdx = u32;
pub type ElemIdx = u32;
pub type DataIdx = u32;
pub type LocalIdx = u32;
pub type LabelIdx = u32;

#[derive(Debug, PartialEq, Clone)]
pub struct Func {
    pub typeidx: TypeIdx,
    pub locals: Vec<ValType>,
    pub body: Expr,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Global {
    pub type_: GlobalType,
    pub value: Expr,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub desc: ImportDesc,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Export {
    pub name: String,
    pub desc: ExportDesc,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Elem {
    pub type_: RefType,
    pub init: Vec<Expr>,
    pub mode: ElemMode,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ElemMode {
    Passiv,
    Active { table: TableIdx, offset: Expr },
    Declarative,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ImportDesc {
    TypeIdx(u32),
    TableType(Table),
    MemType(Memory),
    GlobalType(GlobalType),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExportDesc {
    Func(FuncIdx),
    Table(TableIdx),
    Mem(MemIdx),
    Global(GlobalIdx),
}

#[derive(Debug, PartialEq)]
pub struct Code {
    pub size: u32,
    pub func: Func0,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Local {
    pub n: u32,
    pub type_: ValType,
}

#[derive(Debug, PartialEq)]
pub struct Func0 {
    pub locals: Vec<Local>,
    pub body: Expr,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Data {
    pub init: Vec<u8>,
    pub mode: DataMode,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DataMode {
    Passive,
    Active { memory: MemIdx, offset: Expr },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Table {
    pub reftype: RefType,
    pub limits: Limits,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Memory(pub Limits);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Custom {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Section<T> {
    pub size: u32,
    pub value: T,
}

impl<T> IntoIterator for Section<Vec<T>> {
    type Item = T;

    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter()
    }
}

pub type TypeSec = Section<Vec<FuncType>>;

pub type ImportSec = Section<Vec<Import>>;

pub type FuncSec = Section<Vec<TypeIdx>>;

pub type TableSec = Section<Vec<Table>>;

pub type MemSec = Section<Vec<Memory>>;

pub type GlobalSec = Section<Vec<Global>>;

pub type ExportSec = Section<Vec<Export>>;

pub type StartSec = Section<FuncIdx>;

pub type ElemSec = Section<Vec<Elem>>;

pub type CodeSec = Section<Vec<Code>>;

pub type DataSec = Section<Vec<Data>>;

pub type DataCountSec = Section<u32>;

pub type CustomSec = Section<Custom>;

#[derive(Debug, PartialEq)]
pub struct CustomSecList {
    pub sec1: Vec<Custom>,
    pub sec2: Vec<Custom>,
    pub sec3: Vec<Custom>,
    pub sec4: Vec<Custom>,
    pub sec5: Vec<Custom>,
    pub sec6: Vec<Custom>,
    pub sec7: Vec<Custom>,
    pub sec8: Vec<Custom>,
    pub sec9: Vec<Custom>,
    pub sec10: Vec<Custom>,
    pub sec11: Vec<Custom>,
    pub sec12: Vec<Custom>,
    pub sec13: Vec<Custom>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Module {
    pub version: u8,
    pub types: Vec<FuncType>,
    pub funcs: Vec<Func>,
    pub tables: Vec<Table>,
    pub mems: Vec<Memory>,
    pub globals: Vec<Global>,
    pub elems: Vec<Elem>,
    pub data: Vec<Data>,
    pub start: Option<FuncIdx>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}
