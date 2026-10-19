//! Versioned, camera-independent binary frames with opaque USB payloads.
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 3;
pub const MAX_TRANSFER: usize = 8 * 1024 * 1024;
pub const MAX_HEADER: usize = 65536;
pub const MAX_MESSAGE: usize = 8 + MAX_HEADER + MAX_TRANSFER;
const MAGIC: &[u8; 4] = b"FU\x03\0";
/// Connect and disconnect reasons are shown to the device owner; longer text is cut.
pub const MAX_REASON: usize = 512;

/// Cut a reason to MAX_REASON bytes on a character boundary.
pub fn bounded_reason(text: &str) -> String {
    let mut end = text.len().min(MAX_REASON);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    /// Relay route: exporter connection ID / browser-local device ID.
    pub id: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub interfaces: Vec<Interface>,
    /// Shared devices only: whether the camera is present, and its last battery report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DeviceStatus>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub online: bool,
    /// Why the device is offline: "battery", "unplugged", "not responding".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Last battery level a controller reported, in the firmware's own names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<String>,
    /// Milliseconds since that report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_age_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Interface {
    pub configuration: u8,
    pub interface: u8,
    pub alternate: u8,
    pub endpoints: Vec<Endpoint>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub address: u8,
    pub kind: String,
    pub packet_size: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setup {
    pub request_type: String,
    pub recipient: String,
    pub request: u8,
    pub value: u16,
    pub index: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Echo at the exporter without accessing or claiming USB.
    Probe,
    List,
    /// A neutral progress line for the device owner's log; never touches USB.
    OwnerStatus { message: String },
    /// Owner banner and spoken ten-second countdown; reply only after it ends.
    OwnerAttention { message: String },
    Open {
        configuration: u8,
        interface: u8,
        alternate: u8,
    },
    Close,
    ControlIn {
        setup: Setup,
        length: usize,
    },
    ControlOut {
        setup: Setup,
        #[serde(skip)]
        data: Vec<u8>,
    },
    TransferIn {
        endpoint: u8,
        length: usize,
    },
    TransferOut {
        endpoint: u8,
        #[serde(skip)]
        data: Vec<u8>,
    },
    ClearHalt {
        endpoint: u8,
    },
    /// USB steps run in order at the exporter, stopping after the first that
    /// fails: a command sequence in one round trip. OUT payloads travel in the
    /// frame's binary tail, `out_lengths` bytes each, in step order; the reply
    /// carries every IN payload concatenated, one `steps` entry per step run.
    Batch {
        steps: Vec<Operation>,
        out_lengths: Vec<usize>,
    },
}
/// Most steps one batch may carry.
pub const MAX_STEPS: usize = 4096;
impl Operation {
    /// A batch of transfer steps, with its payload lengths filled in.
    pub fn batch(steps: Vec<Operation>) -> Self {
        let out_lengths = steps
            .iter()
            .filter_map(|step| match step {
                Self::ControlOut { data, .. } | Self::TransferOut { data, .. } => Some(data.len()),
                _ => None,
            })
            .collect();
        Self::Batch { steps, out_lengths }
    }
}
/// One batch step's result; its IN bytes are `length` bytes of the reply data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepCompletion {
    pub transferred: usize,
    pub length: usize,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(skip)]
    pub data: Vec<u8>,
    #[serde(default)]
    pub transferred: usize,
    /// WebUSB status or DOMException name; partial bytes/counts remain available.
    pub error: Option<String>,
    /// Batch replies: one entry per step run, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepCompletion>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Hello {
        version: u32,
        role: String,
        token: String,
        /// Why this controller connected; native tools send their command line.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A controller's last message before it disconnects on purpose.
    Goodbye {
        reason: String,
    },
    /// Controller to owner, no reply: a battery reading the owner cannot take
    /// itself while the camera is shared.
    Status {
        battery: String,
    },
    /// Controller to owner, no reply: a problem only the controller can see
    /// (for example, the camera stopped answering); the owner alerts its user.
    Attention {
        message: String,
    },
    /// Controller to owner, no reply: what the tool is doing now, shown above
    /// the owner's log footer until replaced; an empty title clears it.
    Activity {
        title: String,
    },
    /// Controller to owner, no reply: a progress bar under the owner's log,
    /// `total` units long, until ProgressDone.
    ProgressStart {
        label: String,
        total: u64,
    },
    ProgressUpdate {
        done: u64,
    },
    ProgressDone,
    Ready {
        version: u32,
        id: String,
    },
    Inventory {
        devices: Vec<Device>,
    },
    Request {
        id: u64,
        device: String,
        operation: Operation,
        timeout_ms: u64,
    },
    Reply {
        id: u64,
        completion: Completion,
    },
}

impl Frame {
    /// Metadata is JSON; payload bytes never enter JSON, UTF-8 or base64 codecs.
    pub fn encode(&self) -> crate::Result<Vec<u8>> {
        let joined;
        let data = match self {
            Self::Request {
                operation: Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. },
                ..
            } => data.as_slice(),
            Self::Request { operation: Operation::Batch { steps, .. }, .. } => {
                joined = steps
                    .iter()
                    .flat_map(|step| match step {
                        Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } => {
                            data.as_slice()
                        }
                        _ => &[],
                    })
                    .copied()
                    .collect::<Vec<u8>>();
                joined.as_slice()
            }
            Self::Reply { completion, .. } => completion.data.as_slice(),
            _ => &[],
        };
        if data.len() > MAX_TRANSFER {
            return Err(crate::Error("transfer exceeds 8 MiB".into()));
        }
        let mut bytes = Vec::with_capacity(8 + 512 + data.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&[0; 4]);
        serde_json::to_writer(&mut bytes, self).map_err(|e| crate::Error(e.to_string()))?;
        let header = bytes.len() - 8;
        if header > MAX_HEADER {
            return Err(crate::Error("frame header exceeds limit".into()));
        }
        bytes[4..8].copy_from_slice(&(header as u32).to_le_bytes());
        bytes.extend_from_slice(data);
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> crate::Result<Self> {
        let invalid = || crate::Error("invalid binary proxy frame".into());
        if bytes.len() < 8 || bytes.len() > MAX_MESSAGE || &bytes[..4] != MAGIC {
            return Err(invalid());
        }
        let header = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if header > MAX_HEADER
            || header > bytes.len() - 8
            || bytes.len() - 8 - header > MAX_TRANSFER
        {
            return Err(invalid());
        }
        let mut frame: Self =
            serde_json::from_slice(&bytes[8..8 + header]).map_err(|_| invalid())?;
        let payload = &bytes[8 + header..];
        match &mut frame {
            Self::Request {
                operation: Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. },
                ..
            } => *data = payload.to_vec(),
            Self::Request { operation: Operation::Batch { steps, out_lengths }, .. } => {
                let mut rest = payload;
                let mut lengths = out_lengths.iter();
                for step in steps.iter_mut() {
                    if let Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } = step {
                        let length = *lengths.next().ok_or_else(invalid)?;
                        if length > rest.len() {
                            return Err(invalid());
                        }
                        *data = rest[..length].to_vec();
                        rest = &rest[length..];
                    }
                }
                if lengths.next().is_some() || !rest.is_empty() {
                    return Err(invalid());
                }
            }
            Self::Reply { completion, .. } => completion.data = payload.to_vec(),
            _ if !payload.is_empty() => return Err(invalid()),
            _ => (),
        }
        Ok(frame)
    }
}

impl Operation {
    /// Reject unsupported extents before a request can reach USB.
    pub fn validate(&self) -> crate::Result<()> {
        let invalid = |message: &str| crate::Error(message.into());
        match self {
            Self::OwnerStatus { message } | Self::OwnerAttention { message } => {
                if message.is_empty() || message.len() > MAX_REASON {
                    return Err(invalid("owner message must be 1–512 UTF-8 bytes"));
                }
            }
            Self::ControlIn { setup, length } => {
                setup.validate()?;
                if *length > MAX_TRANSFER {
                    return Err(invalid("transfer exceeds 8 MiB"));
                }
            }
            Self::ControlOut { setup, data } => {
                setup.validate()?;
                validate_data(data)?;
            }
            Self::TransferIn { endpoint, length } => {
                validate_endpoint(*endpoint)?;
                if endpoint & 128 == 0 || *length > MAX_TRANSFER {
                    return Err(invalid("invalid IN transfer"));
                }
            }
            Self::TransferOut { endpoint, data } => {
                validate_endpoint(*endpoint)?;
                if endpoint & 128 != 0 {
                    return Err(invalid("invalid OUT transfer"));
                }
                validate_data(data)?;
            }
            Self::ClearHalt { endpoint } => validate_endpoint(*endpoint)?,
            Self::Batch { steps, out_lengths } => {
                if steps.is_empty() || steps.len() > MAX_STEPS {
                    return Err(invalid("a batch carries 1–4096 steps"));
                }
                let (mut out, mut inward, mut outs) = (0usize, 0usize, Vec::new());
                for step in steps {
                    match step {
                        Self::ControlOut { data, .. } | Self::TransferOut { data, .. } => {
                            out += data.len();
                            outs.push(data.len());
                        }
                        Self::ControlIn { length, .. } | Self::TransferIn { length, .. } => inward += length,
                        Self::ClearHalt { .. } => (),
                        _ => return Err(invalid("batch steps are USB transfers only")),
                    }
                    step.validate()?;
                }
                if outs != *out_lengths || out > MAX_TRANSFER || inward > MAX_TRANSFER {
                    return Err(invalid("batch payloads exceed 8 MiB or disagree with their lengths"));
                }
            }
            _ => (),
        }
        fn validate_endpoint(endpoint: u8) -> crate::Result<()> {
            if endpoint & 0x70 != 0 || endpoint & 15 == 0 {
                return Err(crate::Error("invalid endpoint address".into()));
            }
            Ok(())
        }
        fn validate_data(data: &[u8]) -> crate::Result<()> {
            if data.len() > MAX_TRANSFER {
                return Err(crate::Error("transfer exceeds 8 MiB".into()));
            }
            Ok(())
        }
        Ok(())
    }
}
impl Setup {
    fn validate(&self) -> crate::Result<()> {
        if !["standard", "class", "vendor"].contains(&self.request_type.as_str())
            || !["device", "interface", "endpoint", "other"].contains(&self.recipient.as_str())
        {
            return Err(crate::Error("invalid control setup".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_requests_round_trip_and_bound_text() {
        for operation in [
            Operation::OwnerStatus { message: "stopped: camera in SET-UP mode".into() },
            Operation::OwnerAttention { message: "Film the strip".into() },
        ] {
            operation.validate().unwrap();
            let expected = serde_json::to_value(&operation).unwrap();
            let wire = Frame::Request { id: 8, device: "camera".into(), operation, timeout_ms: 30000 }.encode().unwrap();
            let Frame::Request { id, operation, .. } = Frame::decode(&wire).unwrap() else { panic!() };
            assert_eq!(id, 8);
            assert_eq!(serde_json::to_value(operation).unwrap(), expected);
        }
        for message in [String::new(), "é".repeat(257)] {
            assert!(Operation::OwnerStatus { message: message.clone() }.validate().is_err());
            assert!(Operation::OwnerAttention { message }.validate().is_err());
        }
    }
    #[test]
    fn reasons_are_optional_on_hello_and_bounded() {
        let without_reason: Frame =
            serde_json::from_str(r#"{"type":"hello","version":2,"role":"controller","token":"t"}"#)
                .unwrap();
        assert!(matches!(without_reason, Frame::Hello { reason: None, .. }));
        let goodbye = serde_json::to_string(&Frame::Goodbye {
            reason: "done".into(),
        })
        .unwrap();
        assert_eq!(goodbye, r#"{"type":"goodbye","reason":"done"}"#);
        let long = "é".repeat(MAX_REASON);
        let cut = bounded_reason(&long);
        assert!(cut.len() <= MAX_REASON && long.starts_with(&cut));
    }
    #[test]
    fn activity_and_progress_notifications_round_trip() {
        for frame in [
            Frame::Activity { title: "Uploading".into() },
            Frame::ProgressStart { label: "Uploading".into(), total: 93056 },
            Frame::ProgressUpdate { done: 16384 },
            Frame::ProgressDone,
        ] {
            let json = serde_json::to_value(&frame).unwrap();
            let decoded = Frame::decode(&frame.encode().unwrap()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), json);
        }
        let done = serde_json::to_value(Frame::ProgressDone).unwrap();
        assert_eq!(done, serde_json::json!({"type": "progress_done"}));
    }
    #[test]
    fn batch_carries_out_payloads_in_step_order_and_rejects_mismatch() {
        let setup = || Setup { request_type: "vendor".into(), recipient: "other".into(), request: 0, value: 0, index: 0 };
        let operation = Operation::batch(vec![
            Operation::ControlOut { setup: setup(), data: vec![1, 2, 3] },
            Operation::TransferOut { endpoint: 1, data: vec![] },
            Operation::TransferOut { endpoint: 1, data: vec![4, 5] },
            Operation::TransferIn { endpoint: 0x81, length: 512 },
        ]);
        operation.validate().unwrap();
        let wire = Frame::Request { id: 3, device: "camera".into(), operation, timeout_ms: 9000 }.encode().unwrap();
        let Frame::Request { operation: Operation::Batch { steps, out_lengths }, .. } = Frame::decode(&wire).unwrap() else { panic!() };
        assert_eq!(out_lengths, [3, 0, 2]);
        let datas: Vec<Vec<u8>> = steps.iter().filter_map(|s| match s {
            Operation::ControlOut { data, .. } | Operation::TransferOut { data, .. } => Some(data.clone()),
            _ => None,
        }).collect();
        assert_eq!(datas, [vec![1, 2, 3], vec![], vec![4, 5]]);
        let mut bad = Operation::batch(vec![Operation::TransferOut { endpoint: 1, data: vec![9] }]);
        if let Operation::Batch { out_lengths, .. } = &mut bad { out_lengths[0] = 2; }
        assert!(bad.validate().is_err());
        assert!(Operation::batch(vec![Operation::Close]).validate().is_err());
        assert!(Operation::batch(vec![]).validate().is_err());
    }
    #[test]
    fn binary_payload_round_trips_without_text_expansion() {
        for length in [0, 1, 256, 15999, 16000, 16001, MAX_TRANSFER] {
            let data: Vec<u8> = (0..length).map(|i| i as u8).collect();
            let frame = Frame::Request {
                id: 7,
                device: "camera".into(),
                operation: Operation::TransferOut {
                    endpoint: 1,
                    data: data.clone(),
                },
                timeout_ms: 5000,
            };
            let wire = frame.encode().unwrap();
            let header = u32::from_le_bytes(wire[4..8].try_into().unwrap()) as usize;
            assert_eq!(wire.len(), 8 + header + length);
            assert!(
                !std::str::from_utf8(&wire[8..8 + header])
                    .unwrap()
                    .contains("data")
            );
            let Frame::Request {
                operation: Operation::TransferOut { data: received, .. },
                ..
            } = Frame::decode(&wire).unwrap()
            else {
                panic!()
            };
            assert_eq!(received, data);
        }
        let frame = Frame::Reply {
            id: 9,
            completion: Completion {
                data: vec![0, 255, 128],
                transferred: 3,
                error: Some("stall".into()),
                ..Completion::default()
            },
        };
        let Frame::Reply { completion, .. } = Frame::decode(&frame.encode().unwrap()).unwrap()
        else {
            panic!()
        };
        assert_eq!(completion.data, [0, 255, 128]);
        assert_eq!(completion.transferred, 3);
        assert_eq!(completion.error.as_deref(), Some("stall"));
    }
    #[test]
    fn probe_bytes_match_javascript_codec() {
        let frame = Frame::Request {
            id: 7,
            device: "".into(),
            operation: Operation::Probe,
            timeout_ms: 5000,
        };
        let metadata = br#"{"type":"request","id":7,"device":"","operation":{"op":"probe"},"timeout_ms":5000}"#;
        let mut expected = MAGIC.to_vec();
        expected.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        expected.extend_from_slice(metadata);
        assert_eq!(frame.encode().unwrap(), expected);
    }
    #[test]
    fn malformed_or_oversized_frames_fail_before_execution() {
        let frame = Frame::Request {
            id: 1,
            device: "".into(),
            operation: Operation::Probe,
            timeout_ms: 10,
        };
        let mut bytes = frame.encode().unwrap();
        for length in 0..8 {
            assert!(Frame::decode(&bytes[..length]).is_err());
        }
        bytes[2] = 1;
        assert!(Frame::decode(&bytes).is_err());
        bytes[2] = VERSION as u8;
        bytes.extend_from_slice(&[255]);
        assert!(Frame::decode(&bytes).is_err());
        bytes[4..8].copy_from_slice(&((MAX_HEADER + 1) as u32).to_le_bytes());
        assert!(Frame::decode(&bytes).is_err());
        let huge = Frame::Reply {
            id: 1,
            completion: Completion {
                data: vec![0; MAX_TRANSFER + 1],
                ..Completion::default()
            },
        };
        assert!(huge.encode().is_err());
        let embedded = br#"{"type":"request","id":1,"device":"","operation":{"op":"transfer_out","endpoint":1,"data":"AQID"},"timeout_ms":10}"#;
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&(embedded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(embedded);
        assert!(Frame::decode(&bytes).is_err());
    }
}
