#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::{error::Error, leb128::*, parser::Parser};

impl<'a> Parser<'a> {
    pub fn byte(&mut self) -> Option<u8> {
        self.next()
    }

    pub fn u32(&mut self) -> Result<u32, Error> {
        let (value, bytes) = u32::read_leb128(self.rest())?;
        self.skip(bytes);
        Ok(value)
    }

    pub fn u32_bytes(&mut self) -> Result<(u32, usize), Error> {
        let (value, bytes) = u32::read_leb128(self.rest())?;
        self.skip(bytes);
        Ok((value, bytes))
    }

    pub fn s32(&mut self) -> Result<i32, Error> {
        let (value, bytes) = i32::read_leb128(self.rest())?;
        self.skip(bytes);
        Ok(value)
    }

    pub fn u64(&mut self) -> Result<u64, Error> {
        let (value, bytes) = u64::read_leb128(self.rest())?;
        self.skip(bytes);
        Ok(value)
    }

    pub fn s64(&mut self) -> Result<i64, Error> {
        let (value, bytes) = i64::read_leb128(self.rest())?;
        self.skip(bytes);
        Ok(value)
    }

    pub fn i32(&mut self) -> Result<i32, Error> {
        self.s32()
    }

    pub fn i64(&mut self) -> Result<i64, Error> {
        self.s64()
    }

    pub fn f32(&mut self) -> Result<f32, Error> {
        if self.rest().len() >= 4 {
            let bytes: [u8; 4] = self.rest()[0..4].try_into().unwrap();
            self.skip(4);
            Ok(f32::from_le_bytes(bytes))
        } else {
            Err(Error::UnexpectedEof(format!("f32")))
        }
    }

    pub fn f64(&mut self) -> Result<f64, Error> {
        if self.rest().len() >= 8 {
            let bytes: [u8; 8] = self.rest()[0..8].try_into().unwrap();
            self.skip(8);
            Ok(f64::from_le_bytes(bytes))
        } else {
            Err(Error::UnexpectedEof(format!("f64")))
        }
    }

    pub fn name(&mut self) -> Result<String, Error> {
        let byte = |self_: &mut Self| {
            self_
                .byte()
                .ok_or(Error::UnexpectedEof(format!("part of utf8-encoded bytes")))
        };
        let name = self.vec(byte)?;
        Ok(core::str::from_utf8(&name)
            .and_then(|v| Ok(v.to_string()))
            .map_err(|e| Error::InvalidUtf8(e))?)
    }
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use super::Parser;

    #[test]
    fn test_f32_ok() {
        let mut parser = Parser::new(&[0x00, 0x00, 0x48, 0x41]);
        assert_eq!(
            parser.f32().map(|s| s.partial_cmp(&12.5)),
            Ok(Some(Ordering::Equal))
        );
    }

    #[test]
    fn test_f32_err() {
        let mut parser = Parser::new(&[0x00, 0x00]);
        assert!(matches!(parser.f32(), Err(..)))
    }

    #[test]
    fn test_f64_ok() {
        let mut parser = Parser::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40]);
        assert_eq!(
            parser.f64().map(|s| s.partial_cmp(&12.5)),
            Ok(Some(Ordering::Equal))
        );
    }

    #[test]
    fn test_f64_err() {
        let mut parser = Parser::new(&[0x00, 0x00, 0x48, 0x41]);
        assert!(matches!(parser.f64(), Err(..)))
    }
}
