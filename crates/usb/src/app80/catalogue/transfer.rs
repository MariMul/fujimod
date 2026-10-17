//! Direct service-transfer controls. These are firmware requests, not USB data
//! upload/download compositions or assertions that a hardware callee is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferOperation {
    Operation0,
    Operation1,
    Operation2,
    Operation3,
    Operation6,
}
impl TransferOperation {
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Operation0 => 0,
            Self::Operation1 => 1,
            Self::Operation2 => 2,
            Self::Operation3 => 3,
            Self::Operation6 => 255,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferChannel {
    Operation1,
    Operation3,
    Operation4,
}
impl TransferChannel {
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Operation1 => 0,
            Self::Operation3 => 1,
            Self::Operation4 => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferPrepareResult {
    Success,
    PreparationError,
    Status7,
    Unknown(u8),
}
impl TransferPrepareResult {
    pub(super) fn decode(raw: u8) -> Self {
        match raw {
            0 => Self::Success,
            2 => Self::PreparationError,
            3 => Self::Status7,
            n => Self::Unknown(n),
        }
    }
}
/// The operation selector's callback-error path can leave its output unchanged.
/// Unknown bytes, including the initial 255, never request a host poll.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferResult {
    Success,
    Failed,
    Unknown(u8),
}
impl TransferResult {
    pub(super) fn decode(raw: u8) -> Self {
        match raw {
            0 => Self::Success,
            1 => Self::Failed,
            n => Self::Unknown(n),
        }
    }
}
