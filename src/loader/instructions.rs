use super::{error::Error, parser::Parser};
use crate::binary::*;
#[cfg(not(feature = "std"))]
use crate::lib::*;

impl<'a> Parser<'a> {
    pub fn blocktype(&mut self) -> Result<Block, Error> {
        match self.peek() {
            Some(0x40) => {
                self.next();
                Ok(Block::Empty)
            }
            Some(t) if self.is_valtype(t) => {
                self.next();
                Ok(Block::ValType(ValType::from_byte(t).unwrap()))
            }
            // TODO
            // It is treated as a 33 bit signed integer.
            Some(_) => Ok(Block::TypeIdx(self.s32()? as u32)),
            None => Err(Error::UnexpectedEof(format!("blocktype"))),
        }
    }

    pub fn memarg(&mut self) -> Result<MemArg, Error> {
        Ok(MemArg {
            align: self.u32()?,
            offset: self.u32()?,
        })
    }

    pub fn expr(&mut self) -> Result<Expr, Error> {
        Ok(Expr(
            self.take_while0(Self::instr, |b| b == 0x0B)?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }

    pub fn instr(&mut self) -> Result<Vec<Instr>, Error> {
        let instr = match self.next() {
            // Control Instructions
            Some(0x00) => Instr::Unreachable,
            Some(0x01) => Instr::Nop,
            Some(0x02) => {
                let bt = self.blocktype()?;
                let inner: Vec<Instr> = self
                    .take_while0(Self::instr, |b| b == 0x0B)?
                    .into_iter()
                    .flatten()
                    .collect();
                let mut instrs = vec![Instr::Block {
                    bt,
                    end_offset: inner.len() + 1,
                }];
                instrs.extend(inner.into_iter());
                return Ok(instrs);
            }
            Some(0x03) => {
                let bt = self.blocktype()?;
                let inner: Vec<Instr> = self
                    .take_while0(Self::instr, |b| b == 0x0B)?
                    .into_iter()
                    .flatten()
                    .collect();
                let mut instrs = vec![Instr::Loop { bt }];
                instrs.extend(inner.into_iter());
                return Ok(instrs);
            }
            Some(0x04) => {
                return self.or(
                    |p| {
                        let bt = p.blocktype()?;
                        let mut then_instrs: Vec<Instr> = p
                            .take_while0(Self::instr, |b| b == 0x05)?
                            .into_iter()
                            .flatten()
                            .collect();

                        let else_instrs: Vec<Instr> = p
                            .take_while0(Self::instr, |b| b == 0x0B)?
                            .into_iter()
                            .flatten()
                            .collect();
                        then_instrs.push(Instr::RJump(else_instrs.len() + 1));
                        let mut instrs = vec![Instr::If {
                            bt,
                            else_offset: Some(then_instrs.len() + 1),
                            end_offset: then_instrs.len() + else_instrs.len() + 1,
                        }];
                        instrs.extend(then_instrs.into_iter());
                        instrs.extend(else_instrs.into_iter());
                        Ok(instrs)
                    },
                    |p| {
                        let bt = p.blocktype()?;
                        let then_instrs: Vec<Instr> = p
                            .take_while0(Self::instr, |b| b == 0x0B)?
                            .into_iter()
                            .flatten()
                            .collect();
                        let mut instrs = vec![Instr::If {
                            bt,
                            else_offset: None,
                            end_offset: then_instrs.len() + 1,
                        }];
                        instrs.extend(then_instrs.into_iter());
                        Ok(instrs)
                    },
                )
            }
            Some(0x0C) => Instr::Br(self.labelidx()?),
            Some(0x0D) => Instr::BrIf(self.labelidx()?),
            Some(0x0E) => Instr::BrTable {
                indexs: self.vec(Self::labelidx)?,
                default: self.labelidx()?,
            },
            Some(0x0F) => Instr::Return,
            Some(0x10) => Instr::Call(self.funcidx()?),
            Some(0x11) => Instr::CallIndirect(self.typeidx()?, self.tableidx()?),
            // Reference Instructions
            Some(0xD0) => Instr::RefNull(self.reftype()?),
            Some(0xD1) => Instr::RefIsNull,
            Some(0xD2) => Instr::RefFunc(self.funcidx()?),
            // Parametric Instructions
            Some(0x1A) => Instr::Drop,
            Some(0x1B) => Instr::Select,
            // Variable Instructions
            Some(0x20) => Instr::LocalGet(self.localidx()?),
            Some(0x21) => Instr::LocalSet(self.localidx()?),
            Some(0x22) => Instr::LocalTee(self.localidx()?),
            Some(0x23) => Instr::GlobalGet(self.globalidx()?),
            Some(0x24) => Instr::GlobalSet(self.globalidx()?),
            // Table Instructions
            Some(0x25) => Instr::TableGet(self.tableidx()?),
            Some(0x26) => Instr::TableSet(self.tableidx()?),

            // Memory Instructions
            Some(0x28) => Instr::I32Load(self.memarg()?),
            Some(0x29) => Instr::I64Load(self.memarg()?),
            Some(0x2A) => Instr::F32Load(self.memarg()?),
            Some(0x2B) => Instr::F64Load(self.memarg()?),
            Some(0x2C) => Instr::I32Load8S(self.memarg()?),
            Some(0x2D) => Instr::I32Load8U(self.memarg()?),
            Some(0x2E) => Instr::I32Load16S(self.memarg()?),
            Some(0x2F) => Instr::I32Load16U(self.memarg()?),
            Some(0x30) => Instr::I64Load8S(self.memarg()?),
            Some(0x31) => Instr::I64Load8U(self.memarg()?),
            Some(0x32) => Instr::I64Load16S(self.memarg()?),
            Some(0x33) => Instr::I64Load16U(self.memarg()?),
            Some(0x34) => Instr::I64Load32S(self.memarg()?),
            Some(0x35) => Instr::I64Load32U(self.memarg()?),
            Some(0x36) => Instr::I32Store(self.memarg()?),
            Some(0x37) => Instr::I64Store(self.memarg()?),
            Some(0x38) => Instr::F32Store(self.memarg()?),
            Some(0x39) => Instr::F64Store(self.memarg()?),
            Some(0x3A) => Instr::I32Store8(self.memarg()?),
            Some(0x3B) => Instr::I32Store16(self.memarg()?),
            Some(0x3C) => Instr::I64Store8(self.memarg()?),
            Some(0x3D) => Instr::I64Store16(self.memarg()?),
            Some(0x3E) => Instr::I64Store32(self.memarg()?),
            Some(0x3F) => {
                self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                Instr::MemorySize
            }
            Some(0x40) => {
                self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                Instr::MemoryGrow
            }
            // Numeric Instructions
            Some(0x41) => Instr::I32Const(self.i32()?),
            Some(0x42) => Instr::I64Const(self.i64()?),
            Some(0x43) => Instr::F32Const(self.f32()?),
            Some(0x44) => Instr::F64Const(self.f64()?),

            Some(0x45) => Instr::I32Eqz,
            Some(0x46) => Instr::I32Eq,
            Some(0x47) => Instr::I32Ne,
            Some(0x48) => Instr::I32LtS,
            Some(0x49) => Instr::I32LtU,
            Some(0x4A) => Instr::I32GtS,
            Some(0x4B) => Instr::I32GtU,
            Some(0x4C) => Instr::I32LeS,
            Some(0x4D) => Instr::I32LeU,
            Some(0x4E) => Instr::I32GeS,
            Some(0x4F) => Instr::I32GeU,

            Some(0x50) => Instr::I64Eqz,
            Some(0x51) => Instr::I64Eq,
            Some(0x52) => Instr::I64Ne,
            Some(0x53) => Instr::I64LtS,
            Some(0x54) => Instr::I64LtU,
            Some(0x55) => Instr::I64GtS,
            Some(0x56) => Instr::I64GtU,
            Some(0x57) => Instr::I64LeS,
            Some(0x58) => Instr::I64LeU,
            Some(0x59) => Instr::I64GeS,
            Some(0x5A) => Instr::I64GeU,

            Some(0x5B) => Instr::F32Eq,
            Some(0x5C) => Instr::F32Ne,
            Some(0x5D) => Instr::F32Lt,
            Some(0x5E) => Instr::F32Gt,
            Some(0x5F) => Instr::F32Le,
            Some(0x60) => Instr::F32Ge,

            Some(0x61) => Instr::F64Eq,
            Some(0x62) => Instr::F64Ne,
            Some(0x63) => Instr::F64Lt,
            Some(0x64) => Instr::F64Gt,
            Some(0x65) => Instr::F64Le,
            Some(0x66) => Instr::F64Ge,

            Some(0x67) => Instr::I32Clz,
            Some(0x68) => Instr::I32Ctz,
            Some(0x69) => Instr::I32Popcnt,
            Some(0x6A) => Instr::I32Add,
            Some(0x6B) => Instr::I32Sub,
            Some(0x6C) => Instr::I32Mul,
            Some(0x6D) => Instr::I32DivS,
            Some(0x6E) => Instr::I32DivU,
            Some(0x6F) => Instr::I32RemS,
            Some(0x70) => Instr::I32RemU,
            Some(0x71) => Instr::I32And,
            Some(0x72) => Instr::I32Or,
            Some(0x73) => Instr::I32Xor,
            Some(0x74) => Instr::I32Shl,
            Some(0x75) => Instr::I32ShrS,
            Some(0x76) => Instr::I32ShrU,
            Some(0x77) => Instr::I32RotL,
            Some(0x78) => Instr::I32RotR,

            Some(0x79) => Instr::I64Clz,
            Some(0x7A) => Instr::I64Ctz,
            Some(0x7B) => Instr::I64Popcnt,
            Some(0x7C) => Instr::I64Add,
            Some(0x7D) => Instr::I64Sub,
            Some(0x7E) => Instr::I64Mul,
            Some(0x7F) => Instr::I64DivS,
            Some(0x80) => Instr::I64DivU,
            Some(0x81) => Instr::I64RemS,
            Some(0x82) => Instr::I64RemU,
            Some(0x83) => Instr::I64And,
            Some(0x84) => Instr::I64Or,
            Some(0x85) => Instr::I64Xor,
            Some(0x86) => Instr::I64Shl,
            Some(0x87) => Instr::I64ShrS,
            Some(0x88) => Instr::I64ShrU,
            Some(0x89) => Instr::I64RotL,
            Some(0x8A) => Instr::I64RotR,

            Some(0x8B) => Instr::F32Abs,
            Some(0x8C) => Instr::F32Neg,
            Some(0x8D) => Instr::F32Ceil,
            Some(0x8E) => Instr::F32Floor,
            Some(0x8F) => Instr::F32Trunc,
            Some(0x90) => Instr::F32Nearest,
            Some(0x91) => Instr::F32Sqrt,
            Some(0x92) => Instr::F32Add,
            Some(0x93) => Instr::F32Sub,
            Some(0x94) => Instr::F32Mul,
            Some(0x95) => Instr::F32Div,
            Some(0x96) => Instr::F32Min,
            Some(0x97) => Instr::F32Max,
            Some(0x98) => Instr::F32Copysign,

            Some(0x99) => Instr::F64Abs,
            Some(0x9A) => Instr::F64Neg,
            Some(0x9B) => Instr::F64Ceil,
            Some(0x9C) => Instr::F64Floor,
            Some(0x9D) => Instr::F64Trunc,
            Some(0x9E) => Instr::F64Nearest,
            Some(0x9F) => Instr::F64Sqrt,
            Some(0xA0) => Instr::F64Add,
            Some(0xA1) => Instr::F64Sub,
            Some(0xA2) => Instr::F64Mul,
            Some(0xA3) => Instr::F64Div,
            Some(0xA4) => Instr::F64Min,
            Some(0xA5) => Instr::F64Max,
            Some(0xA6) => Instr::F64Copysign,

            Some(0xA7) => Instr::I32WrapI64,
            Some(0xA8) => Instr::I32TruncF32S,
            Some(0xA9) => Instr::I32TruncF32U,
            Some(0xAA) => Instr::I32TruncF64S,
            Some(0xAB) => Instr::I32TruncF64U,
            Some(0xAC) => Instr::I64ExtendI32S,
            Some(0xAD) => Instr::I64ExtendI32U,
            Some(0xAE) => Instr::I64TruncF32S,
            Some(0xAF) => Instr::I64TruncF32U,
            Some(0xB0) => Instr::I64TruncF64S,
            Some(0xB1) => Instr::I64TruncF64U,
            Some(0xB2) => Instr::F32ConvertI32S,
            Some(0xB3) => Instr::F32ConvertI32U,
            Some(0xB4) => Instr::F32ConvertI64S,
            Some(0xB5) => Instr::F32ConvertI64U,
            Some(0xB6) => Instr::F32DemoteF64,
            Some(0xB7) => Instr::F64ConvertI32S,
            Some(0xB8) => Instr::F64ConvertI32U,
            Some(0xB9) => Instr::F64ConvertI64S,
            Some(0xBA) => Instr::F64ConvertI64U,
            Some(0xBB) => Instr::F64PromoteF32,
            Some(0xBC) => Instr::I32ReinterpretF32,
            Some(0xBD) => Instr::I64ReinterpretF64,
            Some(0xBE) => Instr::F32ReinterpretI32,
            Some(0xBF) => Instr::F64ReinterpretI64,

            Some(0xC0) => Instr::I32Extend8S,
            Some(0xC1) => Instr::I32Extend16S,
            Some(0xC2) => Instr::I64Extend8S,
            Some(0xC3) => Instr::I64Extend16S,
            Some(0xC4) => Instr::I64Extend32S,

            // 0xFC Instructions
            Some(0xFC) => match self.u32() {
                // Numeric Instructions
                Ok(0) => Instr::I32TruncSatF32S,
                Ok(1) => Instr::I32TruncSatF32U,
                Ok(2) => Instr::I32TruncSatF64S,
                Ok(3) => Instr::I32TruncSatF64U,
                Ok(4) => Instr::I64TruncSatF32S,
                Ok(5) => Instr::I64TruncSatF32U,
                Ok(6) => Instr::I64TruncSatF64S,
                Ok(7) => Instr::I64TruncSatF64U,
                // Memory Instructions
                Ok(8) => {
                    let ret = Instr::MemoryInit(self.dataidx()?);
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    ret
                }
                Ok(9) => Instr::DataDrop(self.dataidx()?),
                Ok(10) => {
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    Instr::MemoryCopy
                }
                Ok(11) => {
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    Instr::MemoryFill
                }
                // Table Instructions
                Ok(12) => Instr::TableInit(self.elemidx()?, self.tableidx()?),
                Ok(13) => Instr::ElemDrop(self.elemidx()?),
                Ok(14) => Instr::TableCopy(self.tableidx()?, self.tableidx()?),
                Ok(15) => Instr::TableGrow(self.tableidx()?),
                Ok(16) => Instr::TableSize(self.tableidx()?),
                Ok(17) => Instr::TableFill(self.tableidx()?),
                _ => unreachable!(),
            },
            v => panic!("not implemented{:?}", v),
        };
        Ok(vec![instr])
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        binary::{Block, Expr, Instr},
        loader::parser::Parser,
    };

    #[test]
    fn branch() {
        let mut parasr = Parser::new(&[
            0x41, 0x0, 0x4, 0x40, 0x41, 0x1, 0x10, 0x0, 0x5, 0x41, 0x0, 0x10, 0x0, 0xb, 0xb,
        ]);

        assert_eq!(
            parasr.expr(),
            Ok(Expr(vec![
                Instr::I32Const(0),
                Instr::If {
                    bt: Block::Empty,
                    else_offset: Some(4),
                    end_offset: 6
                },
                Instr::I32Const(1),
                Instr::Call(0),
                Instr::RJump(3),
                Instr::I32Const(0),
                Instr::Call(0)
            ]))
        );
    }
}
