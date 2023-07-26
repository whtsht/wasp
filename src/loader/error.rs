use super::leb128::Type;
#[cfg(not(feature = "std"))]
use crate::lib::*;
use core::str::Utf8Error;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidMagicNumber,
    InvalidVersion,
    InvalidSectionHeader,
    IntOverflow(Type),
    InvalidUtf8(Utf8Error),
    UnexpectedEof(String),
    Expected(String),
    Other(String),
    Or(Box<Error>, Box<Error>),
}
