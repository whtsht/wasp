#[cfg(not(feature = "std"))]
use crate::lib::*;

use crate::binary::*;

use super::{error::Error, parser::Parser};

impl<'a> Parser<'a> {
    pub fn typeidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("typeidx")))?)
    }

    pub fn funcidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("funcidx")))?)
    }

    pub fn tableidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("tableidx")))?)
    }

    pub fn memidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self.u32().map_err(|_| Error::Expected(format!("memidx")))?)
    }

    pub fn globalidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("globalidx")))?)
    }

    pub fn elemidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("elemidx")))?)
    }

    pub fn dataidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("dataidx")))?)
    }

    pub fn localidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("localidx")))?)
    }

    pub fn labelidx(&mut self) -> Result<TypeIdx, Error> {
        Ok(self
            .u32()
            .map_err(|_| Error::Expected(format!("labelidx")))?)
    }

    pub fn custom_sections(&mut self) -> Vec<Custom> {
        self.many0(Self::custom_section)
            .into_iter()
            .map(|s| s.value)
            .collect()
    }

    pub fn ignore_custom_sections(&mut self) {
        self.many0(Self::custom_section);
    }

    pub fn magic(&mut self) -> Result<(), Error> {
        self.target(b"\0asm").ok_or(Error::InvalidMagicNumber)
    }

    pub fn version(&mut self) -> Result<u8, Error> {
        self.target(&[0x01, 0x00, 0x00, 0x00])
            .map(|_| 1)
            .ok_or(Error::InvalidVersion)
    }

    pub fn module(&mut self) -> Result<Module, Error> {
        // magic
        self.magic()?;
        // version
        let version = self.version()?;
        self.ignore_custom_sections();

        // types
        let types = self.many0(Self::typesec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // imports
        let imports = self.many0(Self::importsec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // funcs 1
        let funcs = self
            .many0(Self::funcsec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        self.ignore_custom_sections();

        // tables
        let tables = self.many0(Self::tablesec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // mems
        let mems = self.many0(Self::memsec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // globals
        let globals = self.many0(Self::globalsec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // exports
        let exports = self.many0(Self::exportsec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // start
        let start = self.startsec()?.map(|s| s.value);
        self.ignore_custom_sections();

        // elems
        let elems = self.many0(Self::elemsec).into_iter().flatten().collect();
        self.ignore_custom_sections();

        // datacount
        let data_count = self.datacountsec()?.map(|s| s.value);
        self.ignore_custom_sections();

        // funcs 2
        let codes = self
            .many0(Self::codesec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        self.ignore_custom_sections();

        // funcs validation
        if funcs.len() != codes.len() {
            return Err(Error::Other(format!("functypes length != codes length")));
        }

        let funcs = funcs
            .into_iter()
            .zip(codes.into_iter())
            .map(|(typeidx, code)| Func {
                typeidx,
                locals: code
                    .func
                    .locals
                    .into_iter()
                    .map(|local| vec![local.type_; local.n as usize])
                    .flatten()
                    .collect(),
                body: code.func.body,
            })
            .collect();

        // data
        let data = self
            .many0(Self::datasec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        self.ignore_custom_sections();

        // data validation
        if let Some(count) = data_count {
            if count as usize != data.len() {
                return Err(Error::Other(format!("datacount != data length")));
            }
        }

        Ok(Module {
            version,
            types,
            funcs,
            tables,
            mems,
            globals,
            elems,
            data,
            start,
            imports,
            exports,
        })
    }

    pub fn module_with_customs(&mut self) -> Result<(Module, CustomSecList), Error> {
        // magic
        self.magic()?;
        // version
        let version = self.version()?;
        let sec1 = self.custom_sections();

        // types
        let types = self.many0(Self::typesec).into_iter().flatten().collect();
        let sec2 = self.custom_sections();

        // imports
        let imports = self.many0(Self::importsec).into_iter().flatten().collect();
        let sec3 = self.custom_sections();

        // funcs 1
        let funcs = self
            .many0(Self::funcsec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let sec4 = self.custom_sections();

        // tables
        let tables = self.many0(Self::tablesec).into_iter().flatten().collect();
        let sec5 = self.custom_sections();

        // mems
        let mems = self.many0(Self::memsec).into_iter().flatten().collect();
        let sec6 = self.custom_sections();

        // globals
        let globals = self.many0(Self::globalsec).into_iter().flatten().collect();
        let sec7 = self.custom_sections();

        // exports
        let exports = self.many0(Self::exportsec).into_iter().flatten().collect();
        let sec8 = self.custom_sections();

        // start
        let start = self.startsec()?.map(|s| s.value);
        let sec9 = self.custom_sections();

        // elems
        let elems = self.many0(Self::elemsec).into_iter().flatten().collect();
        let sec10 = self.custom_sections();

        // datacount
        let data_count = self.datacountsec()?.map(|s| s.value);
        let sec11 = self.custom_sections();

        // funcs 2
        let codes = self
            .many0(Self::codesec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let sec12 = self.custom_sections();

        // funcs validation
        if funcs.len() != codes.len() {
            return Err(Error::Other(format!("functypes length != codes length")));
        }

        let funcs = funcs
            .into_iter()
            .zip(codes.into_iter())
            .map(|(typeidx, code)| Func {
                typeidx,
                locals: code
                    .func
                    .locals
                    .into_iter()
                    .map(|local| vec![local.type_; local.n as usize])
                    .flatten()
                    .collect(),
                body: code.func.body,
            })
            .collect();

        // data
        let data = self
            .many0(Self::datasec)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let sec13 = self.custom_sections();

        // data validation
        if let Some(count) = data_count {
            if count as usize != data.len() {
                return Err(Error::Other(format!("datacount != data length")));
            }
        }

        Ok((
            Module {
                version,
                types,
                funcs,
                tables,
                mems,
                globals,
                elems,
                data,
                start,
                imports,
                exports,
            },
            CustomSecList {
                sec1,
                sec2,
                sec3,
                sec4,
                sec5,
                sec6,
                sec7,
                sec8,
                sec9,
                sec10,
                sec11,
                sec12,
                sec13,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::loader::{module::Module, parser::Parser};
    use crate::tests::wat2wasm;

    #[test]
    fn magic() {
        let mut parser = Parser::new(b"\0asm");
        assert_eq!(parser.magic(), Ok(()));

        let mut parser = Parser::new(b"invalid");
        assert!(parser.magic().is_err());
    }

    #[test]
    fn version() {
        let mut parser = Parser::new(&[
            0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x73, 0x6D, 0x61, 0x99,
        ]);
        parser.magic().ok();
        assert_eq!(
            parser.rest(),
            &[0x01, 0x00, 0x00, 0x00, 0x73, 0x6D, 0x61, 0x99]
        );
        assert_eq!(parser.version(), Ok(1));
        assert_eq!(parser.rest(), &[0x73, 0x6D, 0x61, 0x99]);
    }

    #[test]
    fn integer_ok() {
        let mut parser = Parser::new(&[0xc0, 0xbb, 0x78, 0x12, 0x34, 0xff]);
        assert_eq!(parser.s32(), Ok(-123456));
        assert_eq!(parser.rest().len(), 3);
    }

    #[test]
    fn module() {
        let wasm = wat2wasm(
            r#"
            (module
              (import "console" "log" (func $log (param i32)))
              (func $add (param i32) (param i32) (result i32)
                local.get 0
                local.get 1
                i32.add
              )
              (func $main
                ;; load `10` and `3` onto the stack
                i32.const 10
                i32.const 3

                i32.add ;; add up both numbers
                call $log ;; log the result
              )
              (start $main)
            )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        assert!(matches!(
            parser.module(),
            Ok(
                Module {
                    version: 1,
                    types,
                    funcs,
                    start: Some(2),
                    ..
                }
            )
            if funcs.len() == 2
                && types.len() == 3
        ));
    }

    #[test]
    fn branch() {
        let wasm = wat2wasm(
            r#"
            (module
                   (import "env" "print" (func $print (param i32)))
                   (func $main
                        i32.const 0
                        (if
                            (then
                                i32.const 1
                                call $print
                            )
                            (else
                                i32.const 0
                                call $print
                            )
                        )
                   )
                   (start $main)
            )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        assert!(matches!(
            parser.module(),
            Ok(Module {
                version: 1,
                start: Some(1),
                ..
            })
        ));
    }

    #[test]
    fn do_not_anything() {
        let wasm = wat2wasm(r#"(module (func) (start 0))"#).unwrap();
        let mut parser = Parser::new(&wasm);
        assert!(matches!(
            parser.module(),
            Ok(
                Module {
                    version: 1,
                    types,
                    funcs,
                    start: Some(0),
                    ..
                }
            )
            if funcs.len() == 1
                && types.len() == 1
        ));
    }
}
