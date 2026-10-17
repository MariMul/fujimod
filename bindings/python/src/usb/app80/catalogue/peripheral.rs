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
pub(super) struct I2cLength {
    inner: core::I2cLength,
}
#[pymethods]
impl I2cLength {
    #[new]
    fn new(py: Python<'_>, value: u8) -> PyResult<Self> {
        core::I2cLength::new(value)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn value(&self) -> u8 {
        self.inner.value()
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) struct I2cPayload {
    inner: core::I2cPayload,
}
#[pymethods]
impl I2cPayload {
    #[new]
    fn new(py: Python<'_>, data: Vec<u8>) -> PyResult<Self> {
        core::I2cPayload::new(&data)
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.data())
    }
}
#[pyclass(eq, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum I2cPort {
    Port0,
    Port1,
    Port2,
    Port4,
    Port5,
}
impl From<I2cPort> for core::I2cPort {
    fn from(value: I2cPort) -> Self {
        match value {
            I2cPort::Port0 => Self::Port0,
            I2cPort::Port1 => Self::Port1,
            I2cPort::Port2 => Self::Port2,
            I2cPort::Port4 => Self::Port4,
            I2cPort::Port5 => Self::Port5,
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
#[pyclass(eq, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AccumulatorAction {
    Start,
    StopAndRead,
}
impl From<AccumulatorAction> for core::AccumulatorAction {
    fn from(value: AccumulatorAction) -> Self {
        match value {
            AccumulatorAction::Start => Self::Start,
            AccumulatorAction::StopAndRead => Self::StopAndRead,
        }
    }
}
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RegisterAction>()?;
    module.add_class::<I2cLength>()?;
    module.add_class::<I2cPayload>()?;
    module.add_class::<I2cPort>()?;
    module.add_class::<I2cAction>()?;
    module.add_class::<AccumulatorAction>()?;
    Ok(())
}
