#[cfg(not(feature = "std"))]
use crate::lib::*;

use crate::binary::*;

use super::{error::Error, parser::Parser};

impl<'a> Parser<'a> {
    /// 1. Type Section
    pub fn typesec(&mut self) -> Result<TypeSec, Error> {
        self.target(1)
            .ok_or(Error::Expected(format!("section id: 1")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::functype)?,
        })
    }

    /// 2. Import Section
    pub fn importsec(&mut self) -> Result<ImportSec, Error> {
        self.target(2)
            .ok_or(Error::Expected(format!("section id: 2")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::import)?,
        })
    }

    pub fn import(&mut self) -> Result<Import, Error> {
        Ok(Import {
            module: self.name()?,
            name: self.name()?,
            desc: self.importdesc()?,
        })
    }

    pub fn importdesc(&mut self) -> Result<ImportDesc, Error> {
        match self.byte() {
            Some(0x00) => Ok(ImportDesc::Func(self.typeidx()?)),
            Some(0x01) => Ok(ImportDesc::Table(self.table()?)),
            Some(0x02) => Ok(ImportDesc::Mem(self.memory()?)),
            Some(0x03) => Ok(ImportDesc::Global(self.globaltype()?)),
            Some(_) => Err(Error::Expected(format!("importdesc"))),
            None => Err(Error::UnexpectedEof(format!("importdesc"))),
        }
    }

    /// 3. Function Section
    pub fn funcsec(&mut self) -> Result<FuncSec, Error> {
        self.target(3)
            .ok_or(Error::Expected(format!("section id: 3")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::u32)?,
        })
    }

    /// 4. Table Section
    pub fn tablesec(&mut self) -> Result<TableSec, Error> {
        self.target(4)
            .ok_or(Error::Expected(format!("section id: 4")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::table)?,
        })
    }

    /// 5. Memory Section
    pub fn memsec(&mut self) -> Result<MemSec, Error> {
        self.target(5)
            .ok_or(Error::Expected(format!("section id: 5")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::memory)?,
        })
    }

    /// 6. Global Section
    pub fn globalsec(&mut self) -> Result<GlobalSec, Error> {
        self.target(6)
            .ok_or(Error::Expected(format!("section id: 6")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(|p| {
                Ok(Global {
                    type_: p.globaltype()?,
                    value: p.expr()?,
                })
            })?,
        })
    }

    /// 7. Export Section
    pub fn exportsec(&mut self) -> Result<ExportSec, Error> {
        self.target(7)
            .ok_or(Error::Expected(format!("section id: 7")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::export)?,
        })
    }

    pub fn export(&mut self) -> Result<Export, Error> {
        Ok(Export {
            name: self.name()?,
            desc: self.exportdesc()?,
        })
    }

    pub fn exportdesc(&mut self) -> Result<ExportDesc, Error> {
        match self.byte() {
            Some(0x00) => Ok(ExportDesc::Func(self.funcidx()?)),
            Some(0x01) => Ok(ExportDesc::Table(self.tableidx()?)),
            Some(0x02) => Ok(ExportDesc::Mem(self.memidx()?)),
            Some(0x03) => Ok(ExportDesc::Global(self.globalidx()?)),
            _ => unreachable!(),
        }
    }

    /// 8. Start Section
    pub fn startsec(&mut self) -> Result<Option<StartSec>, Error> {
        if let Some(8) = self.peek() {
            self.next();
            Ok(Some(Section {
                size: self.u32()?,
                value: self.funcidx()?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 9. Element Section
    pub fn elemsec(&mut self) -> Result<ElemSec, Error> {
        self.target(9)
            .ok_or(Error::Expected(format!("section id: 9")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::elem)?,
        })
    }

    pub fn elem(&mut self) -> Result<Elem, Error> {
        match self.byte() {
            Some(0) => {
                let offset = self.expr()?;
                let init = self.elem_init()?;
                Ok(Elem {
                    type_: RefType::FuncRef,
                    init,
                    mode: ElemMode::Active {
                        tableidx: 0,
                        offset,
                    },
                })
            }
            Some(1) => {
                let type_ = self.elemkind()?;
                let init = self.elem_init()?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Passiv,
                })
            }
            Some(2) => {
                let table = self.tableidx()?;
                let offset = self.expr()?;
                let type_ = self.elemkind()?;
                let init = self.elem_init()?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Active {
                        tableidx: table,
                        offset,
                    },
                })
            }
            Some(3) => {
                let type_ = self.elemkind()?;
                let init = self.elem_init()?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Declarative,
                })
            }
            Some(4) => {
                let offset = self.expr()?;
                let init = self.vec(Self::expr)?;
                Ok(Elem {
                    type_: RefType::FuncRef,
                    init,
                    mode: ElemMode::Active {
                        tableidx: 0,
                        offset,
                    },
                })
            }
            Some(5) => {
                let type_ = self.reftype()?;
                let init = self.vec(Self::expr)?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Passiv,
                })
            }
            Some(6) => {
                let table = self.tableidx()?;
                let offset = self.expr()?;
                let type_ = self.reftype()?;
                let init = self.vec(Self::expr)?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Active {
                        tableidx: table,
                        offset,
                    },
                })
            }
            Some(7) => {
                let type_ = self.reftype()?;
                let init = self.vec(Self::expr)?;
                Ok(Elem {
                    type_,
                    init,
                    mode: ElemMode::Declarative,
                })
            }
            _ => unreachable!(),
        }
    }

    pub fn elemkind(&mut self) -> Result<RefType, Error> {
        Ok(self
            .target(0x00)
            .ok_or(Error::Expected(format!("0x00")))
            .map(|_| RefType::FuncRef)?)
    }

    pub fn elem_init(&mut self) -> Result<Vec<Expr>, Error> {
        Ok(self
            .vec(Self::funcidx)?
            .into_iter()
            .map(|y| Expr::new(vec![Instr::RefFunc(y)]))
            .collect())
    }

    /// 10. Code Section
    pub fn codesec(&mut self) -> Result<CodeSec, Error> {
        self.target(10)
            .ok_or(Error::Expected(format!("section id: 10")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::code)?,
        })
    }

    pub fn code(&mut self) -> Result<Code, Error> {
        Ok(Code {
            size: self.u32()?,
            func: self.func0()?,
        })
    }

    pub fn func0(&mut self) -> Result<Func0, Error> {
        Ok(Func0 {
            locals: self.vec(Self::local)?,
            body: self.expr()?,
        })
    }

    pub fn local(&mut self) -> Result<Local, Error> {
        Ok(Local {
            n: self.u32()?,
            type_: self.valtype()?,
        })
    }

    /// 11. Data Section
    pub fn datasec(&mut self) -> Result<DataSec, Error> {
        self.target(11)
            .ok_or(Error::Expected(format!("section id: 11")))?;
        Ok(Section {
            size: self.u32()?,
            value: self.vec(Self::data)?,
        })
    }

    pub fn data(&mut self) -> Result<Data, Error> {
        match self.byte() {
            Some(0) => {
                let offset = self.expr()?;
                let init = self.vec(|p| p.byte().ok_or(Error::Expected(format!("byte"))))?;
                Ok(Data {
                    init,
                    mode: DataMode::Active { memidx: 0, offset },
                })
            }
            Some(1) => {
                let init = self.vec(|p| p.byte().ok_or(Error::Expected(format!("byte"))))?;
                Ok(Data {
                    init,
                    mode: DataMode::Passive,
                })
            }
            Some(2) => {
                let memory = self.memidx()?;
                let offset = self.expr()?;
                let init = self.vec(|p| p.byte().ok_or(Error::Expected(format!("byte"))))?;
                Ok(Data {
                    init,
                    mode: DataMode::Active {
                        memidx: memory,
                        offset,
                    },
                })
            }
            _ => unreachable!(),
        }
    }

    /// 12. Data Count Section
    pub fn datacountsec(&mut self) -> Result<Option<DataCountSec>, Error> {
        if let Some(12) = self.peek() {
            self.next();
            Ok(Some(Section {
                size: self.u32()?,
                value: self.u32()?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 0. Custom Section
    pub fn custom_section(&mut self) -> Result<CustomSec, Error> {
        self.target(0)
            .ok_or(Error::Expected(format!("section id: 0")))?;
        let (size, bytes) = self.u32_bytes()?;
        let name = self.name()?;
        let name_len = name.len();
        Ok(Section {
            size,
            value: Custom {
                name,
                bytes: (&self.rest()[..(size as usize - name_len - bytes)]).into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::binary::{FuncType, Mut, ResultType, ValType};
    use crate::loader::{parser::Parser, sections::*};
    use crate::tests::wat2wasm;

    #[test]
    fn test_type_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (func $add (param $lhs i32) (param $rhs i32) (result i32)
                    local.get $lhs
                    local.get $rhs
                    i32.add
                )
                (export "add" (func $add))
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        assert_eq!(
            parser.typesec(),
            Ok(Section {
                size: 7,
                value: vec![FuncType(
                    ResultType(vec![ValType::I32, ValType::I32]),
                    ResultType(vec![ValType::I32])
                )]
            })
        );
    }

    #[test]
    fn test_import_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (global (import "test" "global") (mut i32))
            )"#,
        )
        .unwrap();
        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        assert_eq!(
            parser.importsec(),
            Ok(Section {
                size: 16,
                value: vec![Import {
                    module: "test".into(),
                    name: "global".into(),
                    desc: ImportDesc::Global(GlobalType {
                        valtype: ValType::I32,
                        mut_: Mut::Var
                    })
                }]
            })
        );
        assert_eq!(parser.rest().len(), 0);
    }

    #[test]
    fn test_function_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (func $add (param $lhs i32) (param $rhs i32) (result i32)
                    local.get $lhs
                    local.get $rhs
                    i32.add
                )
                (export "add" (func $add))
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.typesec().unwrap();
        assert_eq!(
            parser.funcsec(),
            Ok(Section {
                size: 2,
                value: vec![0]
            })
        );
    }

    #[test]
    fn test_table_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (table 2 funcref)
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        assert_eq!(
            parser.tablesec(),
            Ok(Section {
                size: 4,
                value: vec![Table {
                    reftype: RefType::FuncRef,
                    limits: Limits::Min(2)
                }]
            })
        );
    }

    #[test]
    fn test_memory_section() {
        let wasm = wat2wasm(r#"(module (memory 1 2))"#).unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        assert_eq!(
            parser.memsec(),
            Ok(Section {
                size: 4,
                value: vec![Memory(Limits::MinMax(1, 2))]
            })
        );
    }

    #[test]
    fn test_global_section() {
        let wasm = wat2wasm(
            r#"
            (module 
                (global i32 (i32.const 100))
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        assert_eq!(
            parser.globalsec(),
            Ok(Section {
                size: 7,
                value: vec![Global {
                    type_: GlobalType {
                        valtype: ValType::I32,
                        mut_: Mut::Const
                    },
                    value: Expr::new(vec![Instr::I32Const(100)])
                }]
            })
        );
    }

    #[test]
    fn test_export_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (func $add (param $lhs i32) (param $rhs i32) (result i32)
                    local.get $lhs
                    local.get $rhs
                    i32.add
                )
                (export "add" (func $add))
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.typesec().unwrap();
        parser.funcsec().unwrap();
        assert_eq!(
            parser.exportsec(),
            Ok(Section {
                size: 7,
                value: vec![Export {
                    name: "add".into(),
                    desc: ExportDesc::Func(0)
                }]
            })
        );
    }

    #[test]
    fn test_start_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (func $foo)
                (start $foo)
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.typesec().unwrap();
        parser.funcsec().unwrap();
        assert_eq!(parser.startsec(), Ok(Some(Section { size: 1, value: 0 })));
    }

    #[test]
    fn test_element_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (table 2 funcref)
                (func $f1 (result i32)
                      i32.const 42)
                (func $f2 (result i32)
                      i32.const 13)
                (elem (i32.const 10) $f1)
                (elem (i32.const 20) $f2)
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.typesec().unwrap();
        parser.funcsec().unwrap();
        parser.tablesec().unwrap();
        assert_eq!(
            parser.elemsec(),
            Ok(Section {
                size: 13,
                value: vec![
                    Elem {
                        type_: RefType::FuncRef,
                        init: vec![Expr::new(vec![Instr::RefFunc(0)])],
                        mode: ElemMode::Active {
                            tableidx: 0,
                            offset: Expr::new(vec![Instr::I32Const(10)])
                        }
                    },
                    Elem {
                        type_: RefType::FuncRef,
                        init: vec![Expr::new(vec![Instr::RefFunc(1)])],
                        mode: ElemMode::Active {
                            tableidx: 0,
                            offset: Expr::new(vec![Instr::I32Const(20)])
                        }
                    }
                ]
            })
        )
    }

    #[test]
    fn test_code_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (func (param $p i32)
                    (result i32) (local $loc f64)
                    local.get $p
                    local.get $p
                    i32.add)
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.typesec().unwrap();
        parser.funcsec().unwrap();
        assert_eq!(
            parser.codesec(),
            Ok(Section {
                size: 11,
                value: vec![Code {
                    size: 9,
                    func: Func0 {
                        locals: vec![Local {
                            n: 1,
                            type_: ValType::F64
                        }],
                        body: Expr::new(vec![
                            Instr::LocalGet(0,),
                            Instr::LocalGet(0,),
                            Instr::I32Add,
                        ],),
                    },
                },],
            },)
        );
    }

    #[test]
    fn test_data_section() {
        let wasm = wat2wasm(
            r#"
            (module
                (memory 1)
                (data (i32.const 0) "Hello")
            )"#,
        )
        .unwrap();

        let mut parser = Parser::new(&wasm);
        parser.magic().unwrap();
        parser.version().unwrap();
        parser.memsec().unwrap();
        assert_eq!(
            parser.datasec(),
            Ok(Section {
                size: 11,
                value: vec![Data {
                    init: vec![b'H', b'e', b'l', b'l', b'o'],
                    mode: DataMode::Active {
                        memidx: 0,
                        offset: Expr::new(vec![Instr::I32Const(0)]),
                    },
                },],
            })
        );
    }
}
