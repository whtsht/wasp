#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::store::MemInst;
use super::value::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum EnvError {
    NotFound,
}

pub trait Env {
    fn call(
        &mut self,
        name: &str,
        params: Vec<Value>,
        memory: Option<&mut MemInst>,
    ) -> Result<Vec<Value>, EnvError>;
}

#[derive(Debug)]
#[cfg(feature = "std")]
pub struct DebugEnv {}

#[cfg(feature = "std")]
impl Env for DebugEnv {
    fn call(
        &mut self,
        name: &str,
        params: Vec<Value>,
        _memory: Option<&mut MemInst>,
    ) -> Result<Vec<Value>, EnvError> {
        match name {
            "start" => {
                println!("hello world");
            }
            "print" => {
                println!("{:?}", params[0]);
            }
            _ => return Err(EnvError::NotFound),
        }
        Ok(vec![])
    }
}
