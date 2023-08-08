#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    UndefinedElement,
    IntegerOverflow,
    InvalidConversionInt,
    DivideByZeroInt,
    TableOutOfRange,
    TableNullRef,
    MemoryOutOfBounds,
    IndirectCallTypeMismatch,
    NotFundRef,
    Env(&'static str),
}

impl core::fmt::Display for Trap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Trap::Unreachable => write!(f, "unreachable"),
            Trap::UndefinedElement => write!(f, "undefined element"),
            Trap::IntegerOverflow => write!(f, "integer overflow"),
            Trap::InvalidConversionInt => write!(f, "invalid conversion to integer"),
            Trap::DivideByZeroInt => write!(f, "integer divide by zero"),
            Trap::TableOutOfRange => write!(f, "failed to refer to table: out of range"),
            Trap::TableNullRef => write!(f, "failed to refer to table: null reference"),
            Trap::MemoryOutOfBounds => write!(f, "out of bounds memory access"),
            Trap::NotFundRef => write!(f, "attempted to call null or external reference"),
            Trap::IndirectCallTypeMismatch => write!(f, "indirect call type mismatch"),
            Trap::Env(env) => write!(f, "environment error: {}", env),
        }
    }
}
