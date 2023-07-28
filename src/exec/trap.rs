#[derive(Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    NotImplemented,
}

impl core::fmt::Display for Trap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Trap::Unreachable => writeln!(f, "unreachable"),
            Trap::NotImplemented => writeln!(f, "not implemented"),
        }
    }
}
