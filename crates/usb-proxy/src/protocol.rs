//! Versioned, camera-independent wire messages. USB payloads are base64, never commands.
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_TRANSFER: usize = 8 * 1024 * 1024;
pub const MAX_MESSAGE: usize = MAX_TRANSFER * 4 / 3 + 65536;

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
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    List,
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
        data: String,
    },
    TransferIn {
        endpoint: u8,
        length: usize,
    },
    TransferOut {
        endpoint: u8,
        data: String,
    },
    ClearHalt {
        endpoint: u8,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Completion {
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub transferred: usize,
    /// WebUSB status or DOMException name; partial bytes/counts remain available.
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Hello {
        version: u32,
        role: String,
        token: String,
    },
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

impl Operation {
    /// Reject unsupported extents before a request can reach USB.
    pub fn validate(&self) -> crate::Result<()> {
        use base64::Engine;
        let invalid = |message: &str| crate::Error(message.into());
        match self {
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
            _ => (),
        }
        fn validate_endpoint(endpoint: u8) -> crate::Result<()> {
            if endpoint & 0x70 != 0 || endpoint & 15 == 0 {
                return Err(crate::Error("invalid endpoint address".into()));
            }
            Ok(())
        }
        fn validate_data(data: &str) -> crate::Result<()> {
            if data.len() > MAX_TRANSFER.div_ceil(3) * 4 {
                return Err(crate::Error("transfer exceeds 8 MiB".into()));
            }
            let length = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| crate::Error("invalid base64".into()))?
                .len();
            if length > MAX_TRANSFER {
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
