#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::env::EnvError;

#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    NotImplemented(String),
    Env(EnvError),
}

impl core::fmt::Display for Trap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Trap::Unreachable => writeln!(f, "unreachable"),
            Trap::NotImplemented(s) => writeln!(f, "not implemented :{}", s),
            Trap::Env(env) => writeln!(f, "env error: {:?}", env),
        }
    }
}
