//! Direct service lens requests. Callback event delivery does not establish
//! successful lens movement; the firmware often discards callback status.
use super::{ArgumentKind, CommandError};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LensSelectionIndex(u8);
impl LensSelectionIndex {
    pub fn new(value: u8) -> Result<Self, CommandError> {
        // 255 cannot be encoded as index+1; 160 is the initialization sentinel.
        if value != 160 && value != 255 {
            Ok(Self(value))
        } else {
            Err(CommandError::Argument(ArgumentKind::LensSelectionIndex))
        }
    }
    pub fn value(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LensTarget {
    Initialize,
    Index { index: LensSelectionIndex },
}
impl LensTarget {
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Initialize => 0,
            Self::Index { index } => index.value() + 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LensSelectionRange {
    NotRejected,
    OutOfRange,
    Unknown(u8),
}
impl LensSelectionRange {
    pub(super) fn decode(raw: u8) -> Self {
        match raw {
            0 => Self::NotRejected,
            1 => Self::OutOfRange,
            n => Self::Unknown(n),
        }
    }
}
