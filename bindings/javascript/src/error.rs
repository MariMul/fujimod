use super::*;
use js_sys::{Error as JsError, Reflect, Uint8Array};

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Usb,
    Transfer,
    Protocol,
    DeviceStatus,
    CompletionUnknown,
    InvalidArgument,
    Busy,
    Closed,
    Abandoned,
}

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolFailureKind {
    ReplyLength,
    CommandEcho,
    FunctionEcho,
    ParameterEcho,
    ReservedField,
    ZeroPacketLimit,
    InvalidUtf16,
    UnterminatedString,
    DataLength,
}
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferFailureKind {
    Cancelled,
    Stall,
    Disconnected,
    Fault,
    InvalidArgument,
    Unknown,
}
fn stage(value: &JsValue, stage: usb::error::TransferStage) {
    use usb::error::TransferStage;
    let tag = match stage {
        TransferStage::Control(kind) => {
            property(
                value,
                "requestKind",
                tracing::TraceRequestKind::from(kind) as u32,
            );
            tracing::TraceStage::Control
        }
        TransferStage::BulkIn => tracing::TraceStage::BulkIn,
        TransferStage::BulkOut => tracing::TraceStage::BulkOut,
    };
    property(value, "stage", tag as u32);
}
pub(crate) fn transfer_failure(source: nusb::transfer::TransferError) -> TransferFailureKind {
    use nusb::transfer::TransferError;
    match source {
        TransferError::Cancelled => TransferFailureKind::Cancelled,
        TransferError::Stall => TransferFailureKind::Stall,
        TransferError::Disconnected => TransferFailureKind::Disconnected,
        TransferError::Fault => TransferFailureKind::Fault,
        TransferError::InvalidArgument => TransferFailureKind::InvalidArgument,
        TransferError::Unknown(_) => TransferFailureKind::Unknown,
    }
}
fn protocol(value: &JsValue, source: usb::error::ProtocolError) {
    use usb::error::ProtocolError as P;
    let kind = match source {
        P::ReplyLength { expected, actual } => {
            property(value, "expected", expected as f64);
            property(value, "actual", actual as f64);
            ProtocolFailureKind::ReplyLength
        }
        P::CommandEcho { expected, actual } => {
            property(value, "expected", expected);
            property(value, "actual", actual);
            ProtocolFailureKind::CommandEcho
        }
        P::FunctionEcho { expected, actual } => {
            property(value, "expected", expected);
            property(value, "actual", actual);
            ProtocolFailureKind::FunctionEcho
        }
        P::ParameterEcho => ProtocolFailureKind::ParameterEcho,
        P::ReservedField(raw) => {
            property(value, "reserved", raw);
            ProtocolFailureKind::ReservedField
        }
        P::ZeroPacketLimit => ProtocolFailureKind::ZeroPacketLimit,
        P::InvalidUtf16 => ProtocolFailureKind::InvalidUtf16,
        P::UnterminatedString => ProtocolFailureKind::UnterminatedString,
        P::DataLength {
            available,
            declared,
        } => {
            property(value, "available", available as f64);
            property(value, "declared", declared as f64);
            ProtocolFailureKind::DataLength
        }
    };
    property(value, "protocolKind", kind as u32);
}

pub(crate) fn property(object: &JsValue, name: &str, value: impl Into<JsValue>) {
    Reflect::set(object, &JsValue::from_str(name), &value.into()).expect("setting an error field");
}
pub(crate) fn fail(kind: ErrorKind, message: &str) -> JsValue {
    let value: JsValue = JsError::new(message).into();
    property(&value, "name", "FujiModUsbError");
    property(&value, "kind", kind as u32);
    value
}
pub(crate) fn error(source: usb::Error) -> JsValue {
    let kind = match &source {
        usb::Error::CompletionUnknown => ErrorKind::CompletionUnknown,
        usb::Error::DeviceStatus { .. } => ErrorKind::DeviceStatus,
        usb::Error::Protocol(_) => ErrorKind::Protocol,
        usb::Error::Transfer { .. }
        | usb::Error::Timeout(_)
        | usb::Error::ShortTransfer { .. }
        | usb::Error::PartialRead { .. } => ErrorKind::Transfer,
        usb::Error::InvalidLength
        | usb::Error::InvalidToken
        | usb::Error::LengthLimit { .. }
        | usb::Error::AddressOverflow
        | usb::Error::OutsideDataWindow
        | usb::Error::InvalidTimeout
        | usb::Error::InvalidInterface
        | usb::Error::AmbiguousEndpoints => ErrorKind::InvalidArgument,
        _ => ErrorKind::Usb,
    };
    let value = fail(kind, &source.to_string());
    match source {
        usb::Error::DeviceStatus {
            command,
            primary,
            secondary,
        } => {
            property(&value, "command", command);
            property(&value, "primary", primary);
            property(&value, "secondary", secondary);
        }
        usb::Error::Transfer {
            stage: where_,
            source,
            transferred,
            partial,
        } => {
            stage(&value, where_);
            property(&value, "transferKind", transfer_failure(source) as u32);
            if let nusb::transfer::TransferError::Unknown(code) = source {
                property(&value, "backendCode", code);
            }
            property(
                &value,
                "transferred",
                transferred
                    .map(|n| JsValue::from_f64(n as f64))
                    .unwrap_or(JsValue::UNDEFINED),
            );
            property(&value, "partial", Uint8Array::from(partial.as_slice()));
        }
        usb::Error::PartialRead { data, source, .. } => {
            property(&value, "stage", tracing::TraceStage::BulkIn as u32);
            property(&value, "partial", Uint8Array::from(data.as_slice()));
            property(&value, "cause", error(*source));
        }
        usb::Error::ShortTransfer {
            expected,
            actual,
            stage: where_,
        } => {
            stage(&value, where_);
            property(&value, "expected", expected as f64);
            property(&value, "transferred", actual as f64);
            property(&value, "partial", Uint8Array::new_with_length(0));
        }
        usb::Error::Timeout(where_) => {
            stage(&value, where_);
            property(&value, "partial", Uint8Array::new_with_length(0));
        }
        usb::Error::Protocol(source) => protocol(&value, source),
        usb::Error::LengthLimit { requested, limit } => {
            property(&value, "requested", requested);
            property(&value, "limit", limit);
        }
        _ => (),
    }
    value
}
pub(crate) fn nz(n: u32) -> Result<std::num::NonZeroU32, JsValue> {
    std::num::NonZeroU32::new(n).ok_or_else(|| {
        fail(
            ErrorKind::InvalidArgument,
            "length or limit must be nonzero",
        )
    })
}
pub(crate) fn bytes(data: impl AsRef<[u8]>) -> JsValue {
    Uint8Array::from(data.as_ref()).into()
}

pub(crate) fn integer(value: JsValue, maximum: u32) -> Result<u32, JsValue> {
    let value = value
        .as_f64()
        .ok_or_else(|| fail(ErrorKind::InvalidArgument, "expected a JavaScript number"))?;
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(maximum) {
        return Err(fail(
            ErrorKind::InvalidArgument,
            "expected an integer within the protocol field's range",
        ));
    }
    Ok(value as u32)
}

pub(crate) fn buffer(value: JsValue) -> Result<Vec<u8>, JsValue> {
    use wasm_bindgen::JsCast;
    value
        .dyn_into::<Uint8Array>()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| fail(ErrorKind::InvalidArgument, "expected a Uint8Array"))
}
