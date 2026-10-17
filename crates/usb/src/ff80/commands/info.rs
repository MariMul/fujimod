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
        selector_request(self as u32)
    }
    pub(crate) fn decode(self, response: Vec<u8>) -> Result<DeviceInfo> {
        decode_reply(response)
    }
}

fn selector_request(selector: u32) -> Vec<u8> {
    let mut request = vec![0; 32];
    request[..4].copy_from_slice(&0x150000u32.to_le_bytes());
    request[16..20].copy_from_slice(&selector.to_le_bytes());
    request
}

fn decode_reply(response: Vec<u8>) -> Result<DeviceInfo> {
    status(&response, 0x150000, 16)?;
    let mut words = Vec::new();
    for pair in response[16..].as_chunks::<2>().0 {
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

/// State reported by the firmware-download information selector. `Writing`
/// describes camera work; it never means the host should block or retry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FirmwareDownloadState {
    Normal,
    Writing,
    Success,
    Failed { code: u8 },
    Unknown(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareDownloadReply {
    pub state: FirmwareDownloadState,
    pub text: String,
    pub raw: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(crate) enum DownloadStatusAction {
    Read,
    Clear,
}
impl DownloadStatusAction {
    pub(crate) fn encode(self) -> Vec<u8> {
        selector_request(match self {
            Self::Read => 0xf0001,
            Self::Clear => 0xf0002,
        })
    }
    pub(crate) fn decode(self, bytes: Vec<u8>) -> Result<FirmwareDownloadReply> {
        let info = decode_reply(bytes)?;
        let state = match info.text.as_str() {
            "NORMAL" => FirmwareDownloadState::Normal,
            "WRITING" => FirmwareDownloadState::Writing,
            "SUCCESS" => FirmwareDownloadState::Success,
            text => match text
                .strip_prefix("FAIL")
                .filter(|suffix| suffix.len() == 2 && suffix.bytes().all(|b| b.is_ascii_digit()))
            {
                Some(code) => FirmwareDownloadState::Failed {
                    code: code.parse().expect("two ASCII decimal digits"),
                },
                None => FirmwareDownloadState::Unknown(text.to_owned()),
            },
        };
        Ok(FirmwareDownloadReply {
            state,
            text: info.text,
            raw: info.raw,
        })
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

#[cfg(test)]
mod download_tests {
    use super::*;
    fn response(text: &str) -> Vec<u8> {
        let mut bytes = vec![0; 16];
        bytes[..4].copy_from_slice(&0x150000u32.to_le_bytes());
        for word in text.encode_utf16().chain([0]) {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn writing_and_failure_are_application_states_not_host_dispositions() {
        for (text, expected) in [
            ("NORMAL", FirmwareDownloadState::Normal),
            ("WRITING", FirmwareDownloadState::Writing),
            ("SUCCESS", FirmwareDownloadState::Success),
            ("FAIL09", FirmwareDownloadState::Failed { code: 9 }),
            ("FAIL99", FirmwareDownloadState::Failed { code: 99 }),
            ("FAIL1", FirmwareDownloadState::Unknown("FAIL1".into())),
            ("FUTURE", FirmwareDownloadState::Unknown("FUTURE".into())),
        ] {
            let bytes = response(text);
            let reply = DownloadStatusAction::Read.decode(bytes.clone()).unwrap();
            assert_eq!(reply.state, expected);
            assert_eq!(reply.text, text);
            assert_eq!(reply.raw, bytes);
        }
    }
    #[test]
    fn clearing_status_does_not_claim_a_normal_result_on_unknown_firmware() {
        let reply = DownloadStatusAction::Clear.decode(response("NEW")).unwrap();
        assert_eq!(reply.state, FirmwareDownloadState::Unknown("NEW".into()));
        assert_eq!(
            &DownloadStatusAction::Read.encode()[16..20],
            &0xf0001u32.to_le_bytes()
        );
        assert_eq!(
            &DownloadStatusAction::Clear.encode()[16..20],
            &0xf0002u32.to_le_bytes()
        );
    }
}
