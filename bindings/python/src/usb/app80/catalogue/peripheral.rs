//! Python variants for the peripheral command enums, with Rust validation.
use super::*;

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum RegisterAction {
    AudioValue { value: u8 },
    DisplayFirst { selector: u8, value: u16 },
    DisplayStub { selector: u8, value: u16 },
    DisplaySecond { selector: u8, value: u16 },
    PowerWrite { register: u16, value: u8 },
    PositionPair { first: u16, second: u16 },
    NoopFirst {},
    NoopSecond {},
}
impl From<RegisterAction> for core::RegisterAction {
    fn from(value: RegisterAction) -> Self {
        match value {
            RegisterAction::AudioValue { value } => Self::AudioValue { value },
            RegisterAction::DisplayFirst { selector, value } => {
                Self::DisplayFirst { selector, value }
            }
            RegisterAction::DisplayStub { selector, value } => {
                Self::DisplayStub { selector, value }
            }
            RegisterAction::DisplaySecond { selector, value } => {
                Self::DisplaySecond { selector, value }
            }
            RegisterAction::PowerWrite { register, value } => Self::PowerWrite { register, value },
            RegisterAction::PositionPair { first, second } => Self::PositionPair { first, second },
            RegisterAction::NoopFirst {} => Self::NoopFirst,
            RegisterAction::NoopSecond {} => Self::NoopSecond,
        }
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) enum I2cAction {
    Read {
        port: I2cPort,
        command: u8,
        length: I2cLength,
    },
    Write {
        port: I2cPort,
        command: u8,
        payload: I2cPayload,
    },
    WriteSingle {
        command: u8,
        value: u8,
    },
}
impl From<I2cAction> for core::I2cAction {
    fn from(value: I2cAction) -> Self {
        match value {
            I2cAction::Read {
                port,
                command,
                length,
            } => Self::Read {
                port: port.into(),
                command,
                length: length.inner,
            },
            I2cAction::Write {
                port,
                command,
                payload,
            } => Self::Write {
                port: port.into(),
                command,
                payload: payload.inner,
            },
            I2cAction::WriteSingle { command, value } => Self::WriteSingle { command, value },
        }
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RegisterAction>()?;

    module.add_class::<I2cAction>()?;

    Ok(())
}
