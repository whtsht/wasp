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
        Ok(Expr(self.take_while0(Self::instr, |b| b == 0x0B)?))
    }

    pub fn instr(&mut self) -> Result<Instr, Error> {
        match self.next() {
            // Control Instructions
            Some(0x00) => Ok(Instr::Unreachable),
            Some(0x01) => Ok(Instr::Nop),
            Some(0x02) => Ok(Instr::Block {
                bt: self.blocktype()?,
                in1: self.take_while0(Self::instr, |b| b == 0x0B)?,
            }),
            Some(0x03) => Ok(Instr::Loop {
                bt: self.blocktype()?,
                in1: self.take_while0(Self::instr, |b| b == 0x0B)?,
            }),
            Some(0x04) => self.or(
                |p| {
                    Ok(Instr::If {
                        bt: p.blocktype()?,
                        in1: p.take_while0(Self::instr, |b| b == 0x05)?,
                        in2: Some(p.take_while0(Self::instr, |b| b == 0x0B)?),
                    })
                },
                |p| {
                    Ok(Instr::If {
                        bt: p.blocktype()?,
                        in1: p.take_while0(Self::instr, |b| b == 0x0B)?,
                        in2: None,
                    })
                },
            ),
            Some(0x0C) => Ok(Instr::Br(self.labelidx()?)),
            Some(0x0D) => Ok(Instr::BrIf(self.labelidx()?)),
            Some(0x0E) => Ok(Instr::BrTable {
                indexs: self.vec(Self::labelidx)?,
                default: self.labelidx()?,
            }),
            Some(0x0F) => Ok(Instr::Return),
            Some(0x10) => Ok(Instr::Call(self.funcidx()?)),
            Some(0x11) => Ok(Instr::CallIndirect(self.typeidx()?, self.tableidx()?)),
            // Reference Instructions
            Some(0xD0) => Ok(Instr::RefNull(self.reftype()?)),
            Some(0xD1) => Ok(Instr::RefIsNull),
            Some(0xD2) => Ok(Instr::RefFunc(self.funcidx()?)),
            // Parametric Instructions
            Some(0x1A) => Ok(Instr::Drop),
            Some(0x1B) => Ok(Instr::Select),
            // Variable Instructions
            Some(0x20) => Ok(Instr::LocalGet(self.localidx()?)),
            Some(0x21) => Ok(Instr::LocalSet(self.localidx()?)),
            Some(0x22) => Ok(Instr::LocalTee(self.localidx()?)),
            Some(0x23) => Ok(Instr::GlobalGet(self.globalidx()?)),
            Some(0x24) => Ok(Instr::GlobalSet(self.globalidx()?)),
            // Table Instructions
            Some(0x25) => Ok(Instr::TableGet(self.tableidx()?)),
            Some(0x26) => Ok(Instr::TableSet(self.tableidx()?)),

            // Memory Instructions
            Some(0x28) => Ok(Instr::I32Load(self.memarg()?)),
            Some(0x29) => Ok(Instr::I64Load(self.memarg()?)),
            Some(0x2A) => Ok(Instr::F32Load(self.memarg()?)),
            Some(0x2B) => Ok(Instr::F64Load(self.memarg()?)),
            Some(0x2C) => Ok(Instr::I32Load8S(self.memarg()?)),
            Some(0x2D) => Ok(Instr::I32Load8U(self.memarg()?)),
            Some(0x2E) => Ok(Instr::I32Load16S(self.memarg()?)),
            Some(0x2F) => Ok(Instr::I32Load16U(self.memarg()?)),
            Some(0x30) => Ok(Instr::I64Load8S(self.memarg()?)),
            Some(0x31) => Ok(Instr::I64Load8U(self.memarg()?)),
            Some(0x32) => Ok(Instr::I64Load16S(self.memarg()?)),
            Some(0x33) => Ok(Instr::I64Load16U(self.memarg()?)),
            Some(0x34) => Ok(Instr::I64Load32S(self.memarg()?)),
            Some(0x35) => Ok(Instr::I64Load32U(self.memarg()?)),
            Some(0x36) => Ok(Instr::I32Store(self.memarg()?)),
            Some(0x37) => Ok(Instr::I64Store(self.memarg()?)),
            Some(0x38) => Ok(Instr::F32Store(self.memarg()?)),
            Some(0x39) => Ok(Instr::F64Store(self.memarg()?)),
            Some(0x3A) => Ok(Instr::I32Store8(self.memarg()?)),
            Some(0x3B) => Ok(Instr::I32Store16(self.memarg()?)),
            Some(0x3C) => Ok(Instr::I64Store8(self.memarg()?)),
            Some(0x3D) => Ok(Instr::I64Store16(self.memarg()?)),
            Some(0x3E) => Ok(Instr::I64Store32(self.memarg()?)),
            Some(0x3F) => {
                self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                Ok(Instr::MemorySize)
            }
            Some(0x40) => {
                self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                Ok(Instr::MemoryGrow)
            }
            // Numeric Instructions
            Some(0x41) => Ok(Instr::I32Const(self.i32()?)),
            Some(0x42) => Ok(Instr::I64Const(self.i64()?)),
            Some(0x43) => Ok(Instr::F32Const(self.f32()?)),
            Some(0x44) => Ok(Instr::F64Const(self.f64()?)),

            Some(0x45) => Ok(Instr::I32Eqz),
            Some(0x46) => Ok(Instr::I32Eq),
            Some(0x47) => Ok(Instr::I32Ne),
            Some(0x48) => Ok(Instr::I32LtS),
            Some(0x49) => Ok(Instr::I32LtU),
            Some(0x4A) => Ok(Instr::I32GtS),
            Some(0x4B) => Ok(Instr::I32GtU),
            Some(0x4C) => Ok(Instr::I32LeS),
            Some(0x4D) => Ok(Instr::I32LeU),
            Some(0x4E) => Ok(Instr::I32GeS),
            Some(0x4F) => Ok(Instr::I32GeU),

            Some(0x50) => Ok(Instr::I64Eqz),
            Some(0x51) => Ok(Instr::I64Eq),
            Some(0x52) => Ok(Instr::I64Ne),
            Some(0x53) => Ok(Instr::I64LtS),
            Some(0x54) => Ok(Instr::I64LtU),
            Some(0x55) => Ok(Instr::I64GtS),
            Some(0x56) => Ok(Instr::I64GtU),
            Some(0x57) => Ok(Instr::I64LeS),
            Some(0x58) => Ok(Instr::I64LeU),
            Some(0x59) => Ok(Instr::I64GeS),
            Some(0x5A) => Ok(Instr::I64GeU),

            Some(0x5B) => Ok(Instr::F32Eq),
            Some(0x5C) => Ok(Instr::F32Ne),
            Some(0x5D) => Ok(Instr::F32Lt),
            Some(0x5E) => Ok(Instr::F32Gt),
            Some(0x5F) => Ok(Instr::F32Le),
            Some(0x60) => Ok(Instr::F32Ge),

            Some(0x61) => Ok(Instr::F64Eq),
            Some(0x62) => Ok(Instr::F64Ne),
            Some(0x63) => Ok(Instr::F64Lt),
            Some(0x64) => Ok(Instr::F64Gt),
            Some(0x65) => Ok(Instr::F64Le),
            Some(0x66) => Ok(Instr::F64Ge),

            Some(0x67) => Ok(Instr::I32Clz),
            Some(0x68) => Ok(Instr::I32Ctz),
            Some(0x69) => Ok(Instr::I32Popcnt),
            Some(0x6A) => Ok(Instr::I32Add),
            Some(0x6B) => Ok(Instr::I32Sub),
            Some(0x6C) => Ok(Instr::I32Mul),
            Some(0x6D) => Ok(Instr::I32DivS),
            Some(0x6E) => Ok(Instr::I32DivU),
            Some(0x6F) => Ok(Instr::I32RemS),
            Some(0x70) => Ok(Instr::I32RemU),
            Some(0x71) => Ok(Instr::I32And),
            Some(0x72) => Ok(Instr::I32Or),
            Some(0x73) => Ok(Instr::I32Xor),
            Some(0x74) => Ok(Instr::I32Shl),
            Some(0x75) => Ok(Instr::I32ShrS),
            Some(0x76) => Ok(Instr::I32ShrU),
            Some(0x77) => Ok(Instr::I32RotL),
            Some(0x78) => Ok(Instr::I32RotR),

            Some(0x79) => Ok(Instr::I64Clz),
            Some(0x7A) => Ok(Instr::I64Ctz),
            Some(0x7B) => Ok(Instr::I64Popcnt),
            Some(0x7C) => Ok(Instr::I64Add),
            Some(0x7D) => Ok(Instr::I64Sub),
            Some(0x7E) => Ok(Instr::I64Mul),
            Some(0x7F) => Ok(Instr::I64DivS),
            Some(0x80) => Ok(Instr::I64DivU),
            Some(0x81) => Ok(Instr::I64RemS),
            Some(0x82) => Ok(Instr::I64RemU),
            Some(0x83) => Ok(Instr::I64And),
            Some(0x84) => Ok(Instr::I64Or),
            Some(0x85) => Ok(Instr::I64Xor),
            Some(0x86) => Ok(Instr::I64Shl),
            Some(0x87) => Ok(Instr::I64ShrS),
            Some(0x88) => Ok(Instr::I64ShrU),
            Some(0x89) => Ok(Instr::I64RotL),
            Some(0x8A) => Ok(Instr::I64RotR),

            Some(0x8B) => Ok(Instr::F32Abs),
            Some(0x8C) => Ok(Instr::F32Neg),
            Some(0x8D) => Ok(Instr::F32Ceil),
            Some(0x8E) => Ok(Instr::F32Floor),
            Some(0x8F) => Ok(Instr::F32Trunc),
            Some(0x90) => Ok(Instr::F32Nearest),
            Some(0x91) => Ok(Instr::F32Sqrt),
            Some(0x92) => Ok(Instr::F32Add),
            Some(0x93) => Ok(Instr::F32Sub),
            Some(0x94) => Ok(Instr::F32Mul),
            Some(0x95) => Ok(Instr::F32Div),
            Some(0x96) => Ok(Instr::F32Min),
            Some(0x97) => Ok(Instr::F32Max),
            Some(0x98) => Ok(Instr::F32Copysign),

            Some(0x99) => Ok(Instr::F64Abs),
            Some(0x9A) => Ok(Instr::F64Neg),
            Some(0x9B) => Ok(Instr::F64Ceil),
            Some(0x9C) => Ok(Instr::F64Floor),
            Some(0x9D) => Ok(Instr::F64Trunc),
            Some(0x9E) => Ok(Instr::F64Nearest),
            Some(0x9F) => Ok(Instr::F64Sqrt),
            Some(0xA0) => Ok(Instr::F64Add),
            Some(0xA1) => Ok(Instr::F64Sub),
            Some(0xA2) => Ok(Instr::F64Mul),
            Some(0xA3) => Ok(Instr::F64Div),
            Some(0xA4) => Ok(Instr::F64Min),
            Some(0xA5) => Ok(Instr::F64Max),
            Some(0xA6) => Ok(Instr::F64Copysign),

            Some(0xA7) => Ok(Instr::I32WrapI64),
            Some(0xA8) => Ok(Instr::I32TruncF32S),
            Some(0xA9) => Ok(Instr::I32TruncF32U),
            Some(0xAA) => Ok(Instr::I32TruncF64S),
            Some(0xAB) => Ok(Instr::I32TruncF64U),
            Some(0xAC) => Ok(Instr::I64ExtendI32S),
            Some(0xAD) => Ok(Instr::I64ExtendI32U),
            Some(0xAE) => Ok(Instr::I64TruncF32S),
            Some(0xAF) => Ok(Instr::I64TruncF32U),
            Some(0xB0) => Ok(Instr::I64TruncF64S),
            Some(0xB1) => Ok(Instr::I64TruncF64U),
            Some(0xB2) => Ok(Instr::F32ConvertI32S),
            Some(0xB3) => Ok(Instr::F32ConvertI32U),
            Some(0xB4) => Ok(Instr::F32ConvertI64S),
            Some(0xB5) => Ok(Instr::F32ConvertI64U),
            Some(0xB6) => Ok(Instr::F32DemoteF64),
            Some(0xB7) => Ok(Instr::F64ConvertI32S),
            Some(0xB8) => Ok(Instr::F64ConvertI32U),
            Some(0xB9) => Ok(Instr::F64ConvertI64S),
            Some(0xBA) => Ok(Instr::F64ConvertI64U),
            Some(0xBB) => Ok(Instr::F64PromoteF32),
            Some(0xBC) => Ok(Instr::I32ReinterpretF32),
            Some(0xBD) => Ok(Instr::I64ReinterpretF64),
            Some(0xBE) => Ok(Instr::F32ReinterpretI32),
            Some(0xBF) => Ok(Instr::F64ReinterpretI64),

            Some(0xC0) => Ok(Instr::I32Extend8S),
            Some(0xC1) => Ok(Instr::I32Extend16S),
            Some(0xC2) => Ok(Instr::I64Extend8S),
            Some(0xC3) => Ok(Instr::I64Extend16S),
            Some(0xC4) => Ok(Instr::I64Extend32S),

            // 0xFC Instructions
            Some(0xFC) => match self.u32() {
                // Numeric Instructions
                Ok(0) => Ok(Instr::I32TruncSatF32S),
                Ok(1) => Ok(Instr::I32TruncSatF32U),
                Ok(2) => Ok(Instr::I32TruncSatF64S),
                Ok(3) => Ok(Instr::I32TruncSatF64U),
                Ok(4) => Ok(Instr::I64TruncSatF32S),
                Ok(5) => Ok(Instr::I64TruncSatF32U),
                Ok(6) => Ok(Instr::I64TruncSatF64S),
                Ok(7) => Ok(Instr::I64TruncSatF64U),
                // Memory Instructions
                Ok(8) => {
                    let ret = Ok(Instr::MemoryInit(self.dataidx()?));
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    ret
                }
                Ok(9) => Ok(Instr::DataDrop(self.dataidx()?)),
                Ok(10) => {
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    Ok(Instr::MemoryCopy)
                }
                Ok(11) => {
                    self.target(0x00).ok_or(Error::Expected(format!("0x00")))?;
                    Ok(Instr::MemoryFill)
                }
                // Table Instructions
                Ok(12) => Ok(Instr::TableInit(self.elemidx()?, self.tableidx()?)),
                Ok(13) => Ok(Instr::ElemDrop(self.elemidx()?)),
                Ok(14) => Ok(Instr::TableCopy(self.tableidx()?, self.tableidx()?)),
                Ok(15) => Ok(Instr::TableGrow(self.tableidx()?)),
                Ok(16) => Ok(Instr::TableSize(self.tableidx()?)),
                Ok(17) => Ok(Instr::TableFill(self.tableidx()?)),
                _ => unreachable!(),
            },
            v => panic!("not implemented{:?}", v),
        }
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
                    in1: vec![Instr::I32Const(1), Instr::Call(0)],
                    in2: Some(vec![Instr::I32Const(0), Instr::Call(0)])
                }
            ]))
        );
    }
}
