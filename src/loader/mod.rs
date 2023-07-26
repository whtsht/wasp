pub mod error;
pub mod instructions;
pub mod leb128;
pub mod module;
pub mod parser;
pub mod sections;
pub mod types;
pub mod values;

use super::binary::Module;
use error::Error;
use parser::Parser;

pub fn parse(input: &[u8]) -> Result<Module, Error> {
    let mut parser = Parser::new(input);
    parser.module()
}
