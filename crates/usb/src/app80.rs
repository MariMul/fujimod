//! App80's command byte and 32-byte argument mailbox.
//!
//! Command identifiers and argument meanings come from the selected firmware's
//! service table. They are independent of ff80 status and callback dispositions.
use crate::{
    Error, Result,
    error::{ProtocolError, check_limit},
};

/// A firmware service-table identifier. Zero is a valid identifier on some bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CommandCode(pub u8);

/// One firmware request, before choosing how to carry it to the camera.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    code: CommandCode,
    arguments: [u8; 32],
}
impl Request {
    /// Unused argument bytes are zeroed; oversize input is rejected, never truncated.
    pub fn new(code: CommandCode, arguments: &[u8]) -> Result<Self> {
        let length = u32::try_from(arguments.len()).map_err(|_| Error::InvalidLength)?;
        check_limit(length, 32)?;
        let mut padded = [0; 32];
        padded[..arguments.len()].copy_from_slice(arguments);
        Ok(Self {
            code,
            arguments: padded,
        })
    }
    pub fn code(&self) -> CommandCode {
        self.code
    }
    pub fn arguments(&self) -> &[u8; 32] {
        &self.arguments
    }
    pub fn encode(&self) -> [u8; 33] {
        let mut bytes = [0; 33];
        bytes[0] = self.code.0;
        bytes[1..].copy_from_slice(&self.arguments);
        bytes
    }
}

/// Exact mailbox bytes. Command-specific results are decoded by firmware APIs;
/// neither the first byte nor any argument word is interpreted as readiness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reply {
    raw: [u8; 33],
}
impl Reply {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let raw = bytes.try_into().map_err(|_| ProtocolError::ReplyLength {
            expected: 33,
            actual: bytes.len(),
        })?;
        Ok(Self { raw })
    }
    pub fn raw(&self) -> &[u8; 33] {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_keep_full_command_byte_and_pad_arguments() {
        let request = Request::new(CommandCode(0), &[1, 2]).unwrap();
        assert_eq!(request.code(), CommandCode(0));
        assert_eq!(&request.encode()[..4], &[0, 1, 2, 0]);
        assert!(request.encode()[3..].iter().all(|&b| b == 0));
        assert_eq!(
            Request::new(CommandCode(255), &[42; 32]).unwrap().encode()[0],
            255
        );
        assert!(matches!(
            Request::new(CommandCode(1), &[0; 33]),
            Err(Error::LengthLimit {
                requested: 33,
                limit: 32
            })
        ));
    }
    #[test]
    fn reply_is_an_exact_opaque_mailbox_not_a_pending_status() {
        for first in [0, 1, 0x80, 0xff] {
            let mut bytes = [42; 33];
            bytes[0] = first;
            assert_eq!(Reply::decode(&bytes).unwrap().raw(), &bytes);
        }
        assert!(Reply::decode(&[0; 32]).is_err());
        assert!(Reply::decode(&[0; 34]).is_err());
    }
}
