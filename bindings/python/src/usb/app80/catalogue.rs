//! Enum conversion and session adapters for the Rust service catalogue.
use super::*;
use crate::defs::{Codec, Definition, ServiceModes};
use usb::app80::catalogue as core;

pyo3::create_exception!(fujimod.usb.app80, CommandError, PyValueError);

#[pyclass(eq, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum ArgumentKind {
    PinBank,
    ProfileIndex,
    WordAlignment,
    WordAddress,
}
impl From<core::ArgumentKind> for ArgumentKind {
    fn from(value: core::ArgumentKind) -> Self {
        match value {
            core::ArgumentKind::PinBank => Self::PinBank,
            core::ArgumentKind::ProfileIndex => Self::ProfileIndex,
            core::ArgumentKind::WordAlignment => Self::WordAlignment,
            core::ArgumentKind::WordAddress => Self::WordAddress,
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum CommandFailure {
    Unsupported { codec: Codec },
    Ambiguous { codec: Codec },
    Unavailable { code: u8 },
    Argument { kind: ArgumentKind },
    OversizeArguments {},
}
fn command_error(py: Python<'_>, error: core::CommandError) -> PyErr {
    let exception = CommandError::new_err(error.to_string());
    let reason = match error {
        core::CommandError::Unsupported(codec) => CommandFailure::Unsupported {
            codec: codec.into(),
        },
        core::CommandError::Ambiguous(codec) => CommandFailure::Ambiguous {
            codec: codec.into(),
        },
        core::CommandError::Unavailable(code) => CommandFailure::Unavailable { code: code.0 },
        core::CommandError::Argument(kind) => CommandFailure::Argument { kind: kind.into() },
        core::CommandError::OversizeArguments => CommandFailure::OversizeArguments {},
    };
    match exception.value(py).setattr("reason", reason) {
        Ok(()) => exception,
        Err(error) => error,
    }
}

macro_rules! quantity {
    ($name:ident,$input:ty) => {
        #[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
        #[derive(Clone)]
        struct $name {
            inner: core::$name,
        }
        #[pymethods]
        impl $name {
            #[new]
            fn new(py: Python<'_>, value: $input) -> PyResult<Self> {
                core::$name::new(value)
                    .map(|inner| Self { inner })
                    .map_err(|e| command_error(py, e))
            }
            #[getter]
            fn value(&self) -> u8 {
                self.inner.value()
            }
        }
    };
}
quantity!(PinBank, u8);
quantity!(ProfileIndex, u8);

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
struct WordAddress {
    inner: core::WordAddress,
}
#[pymethods]
impl WordAddress {
    #[new]
    fn new(py: Python<'_>, value: u32) -> PyResult<Self> {
        core::WordAddress::new(Address(value))
            .map(|inner| Self { inner })
            .map_err(|e| command_error(py, e))
    }
    #[getter]
    fn value(&self) -> u32 {
        self.inner.address().0
    }
}

#[pyclass(eq, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProfileBank {
    First,
    Second,
}
impl From<ProfileBank> for core::ProfileBank {
    fn from(value: ProfileBank) -> Self {
        match value {
            ProfileBank::First => Self::First,
            ProfileBank::Second => Self::Second,
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum ProfileAction {
    Read {
        bank: ProfileBank,
        index: ProfileIndex,
    },
    Write {
        bank: ProfileBank,
        index: ProfileIndex,
        value: u8,
    },
    Reset {},
}
impl From<ProfileAction> for core::ProfileAction {
    fn from(value: ProfileAction) -> Self {
        match value {
            ProfileAction::Read { bank, index } => Self::Read {
                bank: bank.into(),
                index: index.inner,
            },
            ProfileAction::Write { bank, index, value } => Self::Write {
                bank: bank.into(),
                index: index.inner,
                value,
            },
            ProfileAction::Reset {} => Self::Reset,
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum Command {
    WritePins {
        bank: PinBank,
        levels: u16,
        exclude: u16,
    },
    ReadPins {
        bank: PinBank,
    },
    ReadChannel {
        channel: u8,
    },
    WriteChannel {
        channel: u8,
        value: u16,
    },
    ProfileByte {
        action: ProfileAction,
    },
    DownloadStatus {},
    CopyConfiguration {
        page: u8,
    },
    FlushConfiguration {},
    ReadWord {
        address: WordAddress,
    },
    WriteWord {
        address: WordAddress,
        value: u32,
    },
}
impl From<Command> for core::Command {
    fn from(value: Command) -> Self {
        match value {
            Command::WritePins {
                bank,
                levels,
                exclude,
            } => Self::WritePins {
                bank: bank.inner,
                levels,
                exclude,
            },
            Command::ReadPins { bank } => Self::ReadPins { bank: bank.inner },
            Command::ReadChannel { channel } => Self::ReadChannel { channel },
            Command::WriteChannel { channel, value } => Self::WriteChannel { channel, value },
            Command::ProfileByte { action } => Self::ProfileByte(action.into()),
            Command::DownloadStatus {} => Self::DownloadStatus,
            Command::CopyConfiguration { page } => Self::CopyConfiguration { page },
            Command::FlushConfiguration {} => Self::FlushConfiguration,
            Command::ReadWord { address } => Self::ReadWord {
                address: address.inner,
            },
            Command::WriteWord { address, value } => Self::WriteWord {
                address: address.inner,
                value,
            },
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum SaveStatus {
    Success {},
    Failed {},
    Unknown { raw_status: u8 },
}
impl From<core::SaveStatus> for SaveStatus {
    fn from(value: core::SaveStatus) -> Self {
        match value {
            core::SaveStatus::Success => Self::Success {},
            core::SaveStatus::Failed => Self::Failed {},
            core::SaveStatus::Unknown(raw_status) => Self::Unknown { raw_status },
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum WordReadResult {
    Value { value: u32 },
    Rejected {},
    Unknown { raw_status: u8 },
}
impl From<core::WordReadResult> for WordReadResult {
    fn from(value: core::WordReadResult) -> Self {
        match value {
            core::WordReadResult::Value(value) => Self::Value { value },
            core::WordReadResult::Rejected => Self::Rejected {},
            core::WordReadResult::Unknown(raw_status) => Self::Unknown { raw_status },
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum WordWriteResult {
    Confirmed {},
    Rejected {},
    Unknown { raw_status: u8 },
}
impl From<core::WordWriteResult> for WordWriteResult {
    fn from(value: core::WordWriteResult) -> Self {
        match value {
            core::WordWriteResult::Confirmed => Self::Confirmed {},
            core::WordWriteResult::Rejected => Self::Rejected {},
            core::WordWriteResult::Unknown(raw_status) => Self::Unknown { raw_status },
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
enum ReplyValue {
    NoPayload {},
    Pins { levels: u16 },
    Channel { value: u16 },
    ProfileByte { value: u8 },
    DownloadStatus { raw_status: u8 },
    ConfigurationSaved { status: SaveStatus },
    ReadWord { result: WordReadResult },
    WriteWord { result: WordWriteResult },
    Research {},
}
impl From<core::ReplyValue> for ReplyValue {
    fn from(value: core::ReplyValue) -> Self {
        match value {
            core::ReplyValue::NoPayload => Self::NoPayload {},
            core::ReplyValue::Pins(levels) => Self::Pins { levels },
            core::ReplyValue::Channel(value) => Self::Channel { value },
            core::ReplyValue::ProfileByte(value) => Self::ProfileByte { value },
            core::ReplyValue::DownloadStatus { raw_status } => Self::DownloadStatus { raw_status },
            core::ReplyValue::ConfigurationSaved(status) => Self::ConfigurationSaved {
                status: status.into(),
            },
            core::ReplyValue::ReadWord(result) => Self::ReadWord {
                result: result.into(),
            },
            core::ReplyValue::WriteWord(result) => Self::WriteWord {
                result: result.into(),
            },
            core::ReplyValue::Research => Self::Research {},
        }
    }
}
#[pyclass(frozen, module = "fujimod.usb.app80")]
pub(in crate::usb) struct CommandReply {
    pub(in crate::usb) inner: core::CommandReply,
}
#[pymethods]
impl CommandReply {
    #[getter]
    fn value(&self) -> ReplyValue {
        self.inner.value().clone().into()
    }
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.raw().raw())
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(in crate::usb) struct Invocation {
    pub(in crate::usb) inner: usb::operations::app80::Invocation,
}
#[pymethods]
impl Invocation {
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.command().request().encode())
    }
    #[getter]
    fn configuration_limit(&self) -> u32 {
        self.inner.configuration_limit().get()
    }
    fn decode(&self, py: Python<'_>, raw: Vec<u8>) -> PyResult<CommandReply> {
        usb::app80::Reply::decode(&raw)
            .and_then(|reply| self.inner.command().decode(reply))
            .map(|inner| CommandReply { inner })
            .map_err(|e| convert_error(py, e))
    }
}
#[pyclass(frozen, module = "fujimod.usb.app80")]
struct Service {
    inner: usb::operations::app80::Service,
}
#[pymethods]
impl Service {
    #[new]
    fn new(py: Python<'_>, definition: &Definition) -> PyResult<Self> {
        usb::operations::app80::Service::new(definition.inner.clone())
            .map(|inner| Self { inner })
            .map_err(|e| convert_error(py, e))
    }
    fn prepare(
        &self,
        py: Python<'_>,
        modes: &ServiceModes,
        command: Command,
    ) -> PyResult<Invocation> {
        self.inner
            .prepare(modes.inner, command.into())
            .map(|inner| Invocation { inner })
            .map_err(|e| command_error(py, e))
    }
    fn prepare_research(
        &self,
        py: Python<'_>,
        code: &CommandCode,
        modes: &ServiceModes,
        arguments: Vec<u8>,
    ) -> PyResult<Invocation> {
        self.inner
            .prepare_research(code.inner, modes.inner, &arguments)
            .map(|inner| Invocation { inner })
            .map_err(|e| command_error(py, e))
    }
}
#[pyfunction]
fn execute(py: Python<'_>, session: &Session, invocation: &Invocation) -> PyResult<CommandReply> {
    check_limit(&session.limits)?;
    session
        .with(py, |s| block_on(invocation.inner.execute(s)))
        .map(|inner| CommandReply { inner })
}
pub(in crate::usb) fn check_limit(limits: &FirmwareLimits) -> PyResult<()> {
    if limits.configuration.get() < 33 {
        return Err(PyValueError::new_err(
            "service mailbox requires a 33-byte configuration command",
        ));
    }
    Ok(())
}
pub(super) fn register(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("CommandError", py.get_type::<CommandError>())?;
    module.add_class::<ArgumentKind>()?;
    module.add_class::<CommandFailure>()?;
    module.add_class::<PinBank>()?;
    module.add_class::<ProfileIndex>()?;
    module.add_class::<WordAddress>()?;
    module.add_class::<ProfileBank>()?;
    module.add_class::<ProfileAction>()?;
    module.add_class::<Command>()?;
    module.add_class::<SaveStatus>()?;
    module.add_class::<WordReadResult>()?;
    module.add_class::<WordWriteResult>()?;
    module.add_class::<ReplyValue>()?;
    module.add_class::<CommandReply>()?;
    module.add_class::<Invocation>()?;
    module.add_class::<Service>()?;
    Ok(())
}
pub(super) fn register_blocking(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execute, module)?)
}
