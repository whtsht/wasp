use crate::binary::FuncType;
#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::EnvError;

#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    DivByZero,
    OutOfRange,
    TableOutOfRange,
    TableNullRef,
    MemoryOutOfRange,
    NotFundRef,
    NotImplemented(String),
    FuncTypeNotMatch(FuncType, FuncType),
    Env(EnvError),
}

impl core::fmt::Display for Trap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Trap::Unreachable => writeln!(f, "unreachable"),
            Trap::DivByZero => writeln!(f, "divide by zero"),
            Trap::OutOfRange => writeln!(f, "failed to convert number: out of range"),
            Trap::TableOutOfRange => writeln!(f, "failed to refer to table: out of range"),
            Trap::TableNullRef => writeln!(f, "failed to refer to table: null reference"),
            Trap::MemoryOutOfRange => writeln!(f, "failed to reference memory: out of range"),
            Trap::NotFundRef => writeln!(f, "attempted to call null or external reference"),
            Trap::FuncTypeNotMatch(expected, found) => writeln!(
                f,
                "function type not match: expected {:?}, found {:?}",
                expected, found
            ),
            Trap::NotImplemented(s) => writeln!(f, "not implemented :{}", s),
            Trap::Env(env) => writeln!(f, "environment error: {:?}", env),
        }
    }
}
