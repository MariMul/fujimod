//! Diagnostic queries and bulk-test setup are separate from E0/E1 transfers.
use super::word;
use crate::{Result, error::ProtocolError};
use std::num::NonZeroU32;

/// Known protocol selectors, with an explicit escape for research selectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticTag {
    BulkSource,
    Research(u32),
}
impl DiagnosticTag {
    pub fn value(self) -> u32 {
        match self {
            Self::BulkSource => 0x80,
            Self::Research(n) => n,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStatus {
    Success,
    DeviceError { primary: u32, secondary: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticReply {
    pub status: CommandStatus,
    pub fields: [u32; 4],
    pub raw: [u8; 32],
}
impl DiagnosticReply {
    pub(crate) fn decode(bytes: Vec<u8>, command: u32) -> Result<Self> {
        let actual = bytes.len();
        let raw: [u8; 32] = bytes.try_into().map_err(|_| ProtocolError::ReplyLength {
            expected: 32,
            actual,
        })?;
        if word(&raw, 0) != command {
            return Err(ProtocolError::CommandEcho {
                expected: command,
                actual: word(&raw, 0),
            }
            .into());
        }
        if word(&raw, 12) != 0 {
            return Err(ProtocolError::ReservedField(word(&raw, 12)).into());
        }
        let (primary, secondary) = (word(&raw, 4), word(&raw, 8));
        let status = if primary == 0 && secondary == 0 {
            CommandStatus::Success
        } else {
            CommandStatus::DeviceError { primary, secondary }
        };
        Ok(Self {
            status,
            fields: [
                word(&raw, 16),
                word(&raw, 20),
                word(&raw, 24),
                word(&raw, 28),
            ],
            raw,
        })
    }
}

pub(crate) fn query(tag: DiagnosticTag) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    bytes[..4].copy_from_slice(&0x450006u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&tag.value().to_le_bytes());
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DataReadMode {
    Pattern = 0,
    Echo = 0x100,
}

/// Destination-buffer treatment is independent of read-test generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DataWriteMode {
    Preserve = 0,
    FillOnes = 1,
    Echo = 0x100,
}

pub(crate) fn read_test(mode: DataReadMode) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    bytes[..4].copy_from_slice(&0x550001u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&(mode as u32).to_le_bytes());
    bytes
}
pub(crate) fn write_test(length: NonZeroU32, mode: DataWriteMode) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    bytes[..4].copy_from_slice(&0x760001u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&length.get().to_le_bytes());
    bytes[24..28].copy_from_slice(&(mode as u32).to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_result_one_is_not_interpreted_as_pending() {
        let mut reply = query(DiagnosticTag::BulkSource);
        reply[4..8].copy_from_slice(&1u32.to_le_bytes());
        reply[8..12].copy_from_slice(&0xffffu32.to_le_bytes());
        assert_eq!(
            DiagnosticReply::decode(reply, 0x450006).unwrap().status,
            CommandStatus::DeviceError {
                primary: 1,
                secondary: 0xffff
            }
        );
    }
    #[test]
    fn research_tags_are_preserved_and_wrong_commands_rejected() {
        let bytes = query(DiagnosticTag::Research(0xdeadbeef));
        assert_eq!(word(&bytes, 16), 0xdeadbeef);
        assert!(DiagnosticReply::decode(bytes, 0x550001).is_err());
    }
}
