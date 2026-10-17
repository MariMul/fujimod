use super::status;
use crate::{Result, error::ProtocolError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum InfoSelector {
    Vendor = 0x01,
    Manufacturer = 0x02,
    Product = 0x04,
    InternalVersion = 0x08,
    DeviceType = 0x10,
    Serial = 0x20,
    Framework = 0x40,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub text: String,
    /// Preserve the complete reply, including status and unknown padding.
    pub raw: Vec<u8>,
}
impl InfoSelector {
    pub(crate) fn encode(self) -> Vec<u8> {
        let mut request = vec![0; 32];
        request[..4].copy_from_slice(&0x150000u32.to_le_bytes());
        request[16..20].copy_from_slice(&(self as u32).to_le_bytes());
        request
    }
    pub(crate) fn decode(self, response: Vec<u8>) -> Result<DeviceInfo> {
        status(&response, 0x150000, 16)?;
        let mut words = Vec::new();
        for pair in response[16..].chunks_exact(2) {
            let value = u16::from_le_bytes([pair[0], pair[1]]);
            if value == 0 {
                let text = String::from_utf16(&words).map_err(|_| ProtocolError::InvalidUtf16)?;
                return Ok(DeviceInfo {
                    text,
                    raw: response,
                });
            }
            words.push(value);
        }
        Err(ProtocolError::UnterminatedString.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_terminator_must_be_aligned() {
        let mut reply = vec![0; 16];
        reply[..4].copy_from_slice(&0x150000u32.to_le_bytes());
        reply.extend_from_slice(&[0x41, 0, 0, 1, 0, 0]);
        assert_eq!(
            InfoSelector::Product.decode(reply).unwrap().text,
            "A\u{100}"
        );
    }
    #[test]
    fn malformed_utf16_is_rejected() {
        let mut reply = vec![0; 16];
        reply[..4].copy_from_slice(&0x150000u32.to_le_bytes());
        reply.extend_from_slice(&[0, 0xd8, 0, 0]);
        assert!(InfoSelector::Product.decode(reply).is_err());
    }
}
