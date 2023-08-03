#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::EnvError;

#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    DivByZero,
    OutOfRange,
    NotImplemented(String),
    Env(EnvError),
}

impl core::fmt::Display for Trap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Trap::Unreachable => writeln!(f, "unreachable"),
            Trap::DivByZero => writeln!(f, "divide by zero"),
            Trap::OutOfRange => writeln!(f, "failed to convert number: out of range"),
            Trap::NotImplemented(s) => writeln!(f, "not implemented :{}", s),
            Trap::Env(env) => writeln!(f, "environment error: {:?}", env),
        }
    }
}
