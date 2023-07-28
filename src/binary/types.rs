#[cfg(not(feature = "std"))]
use crate::lib::*;
pub trait FromByte: Sized {
    fn from_byte(b: u8) -> Option<Self>;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RefType {
    FuncRef,
    ExternRef,
}

impl FromByte for RefType {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x70 => Some(RefType::FuncRef),
            0x6F => Some(RefType::ExternRef),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
    // TODO
    // Vector instruction
    FuncRef,
    ExternRef,
}

impl FromByte for ValType {
    fn from_byte(n: u8) -> Option<Self> {
        match n {
            // Number Type
            0x7F => Some(ValType::I32),
            0x7E => Some(ValType::I64),
            0x7D => Some(ValType::F32),
            0x7c => Some(ValType::F64),
            // Vector Type
            0x70 => Some(ValType::FuncRef),
            // Reference Type
            0x6F => Some(ValType::ExternRef),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FuncType(pub ResultType, pub ResultType);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ResultType(pub Vec<ValType>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Limits {
    Min(u32),
    MinMax(u32, u32),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Mut {
    Const,
    Var,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GlobalType {
    pub valtype: ValType,
    pub mut_: Mut,
}
