use super::{error::Error, parser::Parser};
use crate::binary::*;
#[cfg(not(feature = "std"))]
use crate::lib::*;

impl<'a> Parser<'a> {
    pub fn reftype(&mut self) -> Result<RefType, Error> {
        if let Some(byte) = self.byte() {
            FromByte::from_byte(byte).ok_or(Error::Expected(format!("reftype")))
        } else {
            Err(Error::UnexpectedEof(format!("reftype")))
        }
    }

    pub fn valtype(&mut self) -> Result<ValType, Error> {
        if let Some(byte) = self.byte() {
            FromByte::from_byte(byte).ok_or(Error::Expected(format!("valtype")))
        } else {
            Err(Error::UnexpectedEof(format!("valtype")))
        }
    }

    pub fn is_valtype(&self, byte: u8) -> bool {
        byte == 0x7F
            || byte == 0x7E
            || byte == 0x7D
            || byte == 0x7c
            || byte == 0x7B
            || byte == 0x70
            || byte == 0x6F
    }

    pub fn result_types(&mut self) -> Result<ResultType, Error> {
        Ok(ResultType(self.vec(Self::valtype)?))
    }

    pub fn functype(&mut self) -> Result<FuncType, Error> {
        if let Some(byte) = self.byte() {
            if byte != 0x60 {
                return Err(Error::Expected(format!("0x60")));
            }
        }

        Ok(FuncType(self.result_types()?, self.result_types()?))
    }

    pub fn limits(&mut self) -> Result<Limits, Error> {
        match self.byte() {
            Some(0x00) => Ok(Limits::Min(self.u32()?)),
            Some(0x01) => Ok(Limits::MinMax(self.u32()?, self.u32()?)),
            Some(_) => Err(Error::Expected(format!("limits"))),
            None => Err(Error::UnexpectedEof(format!("limits"))),
        }
    }

    pub fn memory(&mut self) -> Result<Memory, Error> {
        Ok(Memory(self.limits()?))
    }

    pub fn table(&mut self) -> Result<Table, Error> {
        Ok(Table {
            reftype: self.reftype()?,
            limits: self.limits()?,
        })
    }

    pub fn mut_(&mut self) -> Result<Mut, Error> {
        match self.byte() {
            Some(0x00) => Ok(Mut::Const),
            Some(0x01) => Ok(Mut::Var),
            _ => Err(Error::Expected(format!("0x00 or 0x01"))),
        }
    }

    pub fn globaltype(&mut self) -> Result<GlobalType, Error> {
        Ok(GlobalType {
            valtype: self.valtype()?,
            mut_: self.mut_()?,
        })
    }
}
