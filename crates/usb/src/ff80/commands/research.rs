//! Explicit escape hatch for unclassified ff80 commands.
//!
//! This validates only the common wire header. Command-specific fields, effects
//! and completion semantics must come from the selected firmware's definition.
use super::{diagnostic::CommandStatus, status, word};
use crate::{Error, Result};
use std::num::NonZeroU32;

/// An ff80 command identifier, distinct from an App80 mailbox command byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandCode(pub u32);

#[derive(Clone, Debug)]
pub struct Request {
    code: CommandCode,
    body: Vec<u8>,
    reply_capacity: NonZeroU32,
}
impl Request {
    /// `body` begins at byte 16, after the common command/status header. At
    /// least 16 bytes are required by ff80. No padding or truncation is implicit.
    pub fn new(code: CommandCode, body: Vec<u8>, reply_capacity: NonZeroU32) -> Result<Self> {
        if body.len() < 16 || reply_capacity.get() < 16 {
            return Err(Error::InvalidLength);
        }
        u32::try_from(body.len())
            .ok()
            .and_then(|n| n.checked_add(16))
            .ok_or(Error::InvalidLength)?;
        Ok(Self {
            code,
            body,
            reply_capacity,
        })
    }
    pub fn code(&self) -> CommandCode {
        self.code
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn reply_capacity(&self) -> NonZeroU32 {
        self.reply_capacity
    }
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![0; 16];
        bytes[..4].copy_from_slice(&self.code.0.to_le_bytes());
        bytes.extend_from_slice(&self.body);
        bytes
    }
    pub(crate) fn decode(&self, raw: Vec<u8>) -> Result<Reply> {
        let command_status = match status(&raw, self.code.0, 16) {
            Ok(()) => CommandStatus::Success,
            Err(Error::DeviceStatus {
                primary, secondary, ..
            }) => CommandStatus::DeviceError { primary, secondary },
            Err(error) => return Err(error),
        };
        // The session also bounds the received transfer; keep this codec's
        // invariant independent of its caller.
        crate::error::check_limit(
            u32::try_from(raw.len()).map_err(|_| Error::InvalidLength)?,
            self.reply_capacity.get(),
        )?;
        Ok(Reply {
            code: CommandCode(word(&raw, 0)),
            status: command_status,
            raw,
        })
    }
}

/// A completed, common-header-validated reply. A successful exchange says
/// nothing about firmware work started by the command. No status means "wait".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reply {
    pub code: CommandCode,
    pub status: CommandStatus,
    raw: Vec<u8>,
}
impl Reply {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn body(&self) -> &[u8] {
        &self.raw[16..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        Request::new(
            CommandCode(0x12345678),
            vec![7; 16],
            NonZeroU32::new(64).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn research_wire_is_exact_and_has_no_implicit_padding() {
        let command = request();
        assert_eq!(
            &command.encode()[..16],
            &[0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&command.encode()[16..], command.body());
        assert!(Request::new(command.code(), vec![0; 15], NonZeroU32::new(64).unwrap()).is_err());
        assert!(Request::new(command.code(), vec![0; 16], NonZeroU32::new(15).unwrap()).is_err());
    }
    #[test]
    fn device_status_is_retained_without_becoming_a_host_wait_instruction() {
        let command = request();
        let mut raw = command.encode();
        raw[4..8].copy_from_slice(&1u32.to_le_bytes());
        raw[8..12].copy_from_slice(&0xffffu32.to_le_bytes());
        let reply = command.decode(raw.clone()).unwrap();
        assert_eq!(
            reply.status,
            CommandStatus::DeviceError {
                primary: 1,
                secondary: 0xffff
            }
        );
        assert_eq!(reply.raw(), raw);
        assert_eq!(reply.body(), command.body());
    }
    #[test]
    fn foreign_and_reserved_headers_are_rejected_even_on_errors() {
        let command = request();
        for offset in [0, 12] {
            let mut bytes = command.encode();
            bytes[4] = 1;
            bytes[offset] ^= 1;
            assert!(command.decode(bytes).is_err());
        }
        assert!(command.decode(vec![0; 15]).is_err());
    }
}
