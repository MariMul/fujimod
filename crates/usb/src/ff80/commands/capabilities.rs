//! Select the firmware capability data for a subsequent E1 read.
//!
//! The table's layout and meaning depend on firmware. This command reports its
//! byte length; it neither reads the table nor changes host transfer timeouts.
use super::{
    diagnostic::{CommandStatus, DiagnosticReply},
    memory_request, word,
};
use crate::{Error, Result, error::ProtocolError};

/// Metadata from one successfully completed capabilities request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitiesReply {
    pub byte_length: u32,
    pub raw: [u8; 32],
}

pub(crate) fn request() -> Vec<u8> {
    // Preserve the request shape used by fffw. Known firmware ignores this
    // parameter; it is not a host timeout, table size or per-body default.
    let mut parameters = [0; 16];
    parameters[..4].copy_from_slice(&0x100u32.to_le_bytes());
    memory_request(0x580000, 0, &parameters, &[])
}

pub(crate) fn decode(bytes: Vec<u8>) -> Result<CapabilitiesReply> {
    let reply = DiagnosticReply::decode(bytes, 0x580000)?;
    if word(&reply.raw, 16) != 0 {
        return Err(ProtocolError::FunctionEcho {
            expected: 0,
            actual: word(&reply.raw, 16),
        }
        .into());
    }
    if let CommandStatus::DeviceError { primary, secondary } = reply.status {
        return Err(Error::DeviceStatus {
            command: 0x580000,
            primary,
            secondary,
        });
    }
    Ok(CapabilitiesReply {
        byte_length: word(&reply.raw, 20),
        raw: reply.raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(length: u32) -> Vec<u8> {
        let mut bytes = vec![0; 32];
        bytes[..4].copy_from_slice(&0x580000u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&length.to_le_bytes());
        bytes
    }

    #[test]
    fn length_is_firmware_supplied_and_unknown_fields_are_preserved() {
        for size in [0, 48, 128, u32::MAX] {
            let mut bytes = response(size);
            bytes[24..32].fill(0x5a);
            let decoded = decode(bytes.clone()).unwrap();
            assert_eq!(decoded.byte_length, size);
            assert_eq!(decoded.raw.as_slice(), bytes);
        }
    }

    #[test]
    fn device_error_is_not_interpreted_as_a_length_or_pending_state() {
        let mut bytes = response(48);
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0xffffu32.to_le_bytes());
        assert!(matches!(
            decode(bytes),
            Err(Error::DeviceStatus {
                command: 0x580000,
                primary: 1,
                secondary: 0xffff
            })
        ));
    }

    #[test]
    fn malformed_metadata_is_rejected() {
        let mut wrong = response(48);
        wrong[16] = 1;
        assert!(matches!(
            decode(wrong),
            Err(Error::Protocol(ProtocolError::FunctionEcho { .. }))
        ));
        for length in [0, 20, 31, 33, 36] {
            let mut bytes = response(48);
            bytes.resize(length, 0);
            assert!(decode(bytes).is_err());
        }
    }
}
