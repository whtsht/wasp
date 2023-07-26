#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::error::Error;

pub struct Parser<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

pub trait Target {
    fn target(parser: &mut Parser, target: Self) -> Option<()>;
}

impl Target for &[u8] {
    fn target(parser: &mut Parser, target: Self) -> Option<()> {
        (&parser.bytes[parser.cursor..])
            .strip_prefix(target)
            .and_then(|rest| {
                parser.cursor = parser.bytes.len() - rest.len();
                Some(())
            })
    }
}

impl<const N: usize> Target for &[u8; N] {
    fn target(parser: &mut Parser, target: Self) -> Option<()> {
        (&parser.bytes[parser.cursor..])
            .strip_prefix(target)
            .and_then(|rest| {
                parser.cursor = parser.bytes.len() - rest.len();
                Some(())
            })
    }
}

impl Target for u8 {
    fn target(parser: &mut Parser, target: Self) -> Option<()> {
        match parser.byte() {
            Some(byte) if byte == target => Some(()),
            _ => None,
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn next(&mut self) -> Option<u8> {
        if let Some(&value) = self.bytes.get(self.cursor) {
            self.cursor += 1;
            Some(value)
        } else {
            None
        }
    }

    pub fn peek(&mut self) -> Option<u8> {
        if let Some(&value) = self.bytes.get(self.cursor) {
            Some(value)
        } else {
            None
        }
    }

    pub fn skip(&mut self, step: usize) {
        self.cursor += step;
    }

    pub fn target<T: Target>(&mut self, target: T) -> Option<()> {
        Target::target(self, target)
    }

    pub fn rest(&self) -> &[u8] {
        &self.bytes[self.cursor..]
    }

    pub fn vec<T, F>(&mut self, mut f: F) -> Result<Vec<T>, Error>
    where
        F: FnMut(&mut Self) -> Result<T, Error>,
    {
        let len = self.u32()?;
        let mut vec = Vec::new();
        for _ in 0..len {
            vec.push(f(self)?);
        }
        Ok(vec)
    }

    pub fn take_while0<T, F, C>(&mut self, mut f: F, cond: C) -> Result<Vec<T>, Error>
    where
        F: FnMut(&mut Self) -> Result<T, Error>,
        C: Fn(u8) -> bool,
    {
        let mut vec = Vec::new();
        loop {
            match self.peek() {
                Some(b) if cond(b) => {
                    self.next();
                    break;
                }
                Some(_) => {
                    vec.push(f(self)?);
                }
                None => return Err(Error::Expected(format!("next element or terminator"))),
            }
        }
        Ok(vec)
    }

    pub fn or<A, B, T>(&mut self, mut a: A, mut b: B) -> Result<T, Error>
    where
        A: FnMut(&mut Self) -> Result<T, Error>,
        B: FnMut(&mut Self) -> Result<T, Error>,
    {
        match a(self) {
            Ok(ok) => Ok(ok),
            Err(err1) => match b(self) {
                Ok(ok) => Ok(ok),
                Err(err2) => Err(Error::Or(Box::new(err1), Box::new(err2))),
            },
        }
    }

    pub fn many0<T, F>(&mut self, mut f: F) -> Vec<T>
    where
        F: FnMut(&mut Self) -> Result<T, Error>,
    {
        let mut vec = vec![];
        let mut last = self.cursor;
        while let Ok(v) = f(self) {
            vec.push(v);
            last = self.cursor;
        }
        self.cursor = last;
        vec
    }
}

#[cfg(test)]
mod tests {
    use crate::loader::{error::Error, parser::Parser};

    #[test]
    fn test_target() {
        let mut parser = Parser::new(b"abcdef");
        assert_eq!(parser.target(b"abc"), Some(()));
        assert_eq!(parser.rest(), b"def");
    }

    #[test]
    fn test_peek() {
        let mut parser = Parser::new(b"abcdef");
        assert_eq!(parser.peek(), Some(b'a'));
        assert_eq!(parser.peek(), Some(b'a'));
        assert_eq!(parser.peek(), Some(b'a'));
        assert_eq!(parser.peek(), Some(b'a'));
        assert_eq!(parser.peek(), Some(b'a'));
        assert_eq!(parser.rest(), b"abcdef");
    }

    #[test]
    fn test_take_while() {
        let mut parser = Parser::new(b"abcabcabce");
        assert_eq!(
            parser.take_while0(
                |p| p.target(b"abc").ok_or(Error::Expected(format!("abc"))),
                |b| b == b'e'
            ),
            Ok(vec![(), (), ()])
        );
    }

    #[test]
    fn test_or() {
        let mut parser = Parser::new(b"abcdef");
        assert_eq!(
            parser.or(
                |p| p.target(b"def").ok_or(Error::Expected(format!("def"))),
                |p| p.target(b"abc").ok_or(Error::Expected(format!("abc")))
            ),
            Ok(())
        );
        assert_eq!(parser.rest(), b"def");
        assert_eq!(
            parser.or(
                |p| p.target(b"abc").ok_or(Error::Expected(format!("abc"))),
                |p| p.target(b"def").ok_or(Error::Expected(format!("def"))),
            ),
            Ok(())
        );
        assert_eq!(parser.rest(), b"");
    }

    #[test]
    fn test_name() {
        assert_eq!(
            Parser::new(&[0x03, 0x61, 0x64, 0x64]).name(),
            Ok(String::from("add"))
        );
        assert_eq!(
            Parser::new(&[0x04, 0x6d, 0x61, 0x69, 0x6e]).name(),
            Ok(String::from("main"))
        );
        assert_eq!(
            Parser::new(&[0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74]).name(),
            Ok(String::from("_start"))
        );
    }
}
