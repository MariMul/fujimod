//! Python packaging and language conversion only; all USB framing lives in Rust.
use fujimod_usb::{
    self as usb,
    connection::{Connection, TransferOptions},
    ff80::{
        self,
        commands::{
            configuration::{self, ConfigurationLocation, ReadConfiguration, WriteConfiguration},
            debug, diagnostic, info,
            memory::{Address, DebugMemoryLimits, MemoryRange, ReadMemory, WriteMemory},
        },
    },
};
use futures_lite::future::block_on;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyList, PyModule},
};
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};

pyo3::create_exception!(fujimod.usb, UsbError, PyRuntimeError);
pyo3::create_exception!(fujimod.usb, TransferError, UsbError);
pyo3::create_exception!(fujimod.usb, ProtocolError, UsbError);
pyo3::create_exception!(fujimod.usb, CompletionUnknownError, UsbError);
pyo3::create_exception!(fujimod.usb, DeviceCommandError, UsbError);
pyo3::create_exception!(fujimod.usb, ClosedSessionError, UsbError);
pyo3::create_exception!(fujimod.usb, SessionBusyError, UsbError);

fn nz(n: u32) -> PyResult<NonZeroU32> {
    NonZeroU32::new(n).ok_or_else(|| PyValueError::new_err("length/limit must be nonzero"))
}
fn convert_error(py: Python<'_>, error: usb::Error) -> PyErr {
    let message = error.to_string();
    let exception = match &error {
        usb::Error::CompletionUnknown => CompletionUnknownError::new_err(message),
        usb::Error::DeviceStatus { .. } => DeviceCommandError::new_err(message),
        usb::Error::Protocol(_) => ProtocolError::new_err(message),
        usb::Error::Transfer { .. }
        | usb::Error::Timeout(_)
        | usb::Error::ShortTransfer { .. }
        | usb::Error::PartialRead { .. } => TransferError::new_err(message),
        usb::Error::LengthLimit { .. }
        | usb::Error::AddressOverflow
        | usb::Error::OutsideDataWindow
        | usb::Error::InvalidLength
        | usb::Error::InvalidToken
        | usb::Error::InvalidTimeout => PyValueError::new_err(message),
        _ => UsbError::new_err(message),
    };
    let object = exception.value(py);
    // Transfers that fail before a data completion still expose the same fields.
    if matches!(
        &error,
        usb::Error::Transfer { .. }
            | usb::Error::Timeout(_)
            | usb::Error::ShortTransfer { .. }
            | usb::Error::PartialRead { .. }
    ) {
        let _ = object.setattr("partial", PyBytes::new(py, &[]));
        let _ = object.setattr("transferred", py.None());
    }
    match error {
        usb::Error::DeviceStatus {
            command,
            primary,
            secondary,
        } => {
            let _ = object.setattr("command", command);
            let _ = object.setattr("primary", primary);
            let _ = object.setattr("secondary", secondary);
        }
        usb::Error::Transfer {
            transferred,
            partial,
            ..
        } => {
            let _ = object.setattr("transferred", transferred);
            let _ = object.setattr("partial", PyBytes::new(py, &partial));
        }
        usb::Error::ShortTransfer {
            expected, actual, ..
        } => {
            let _ = object.setattr("expected", expected);
            let _ = object.setattr("transferred", actual);
        }
        usb::Error::PartialRead { data, source, .. } => {
            let _ = object.setattr("partial", PyBytes::new(py, &data));
            let _ = object.setattr("transfer_error", convert_error(py, *source));
        }
        _ => (),
    }
    exception
}

macro_rules! enum_binding {
    ($name:ident,$python:literal,$target:ty,{$($variant:ident),+ $(,)?}) => {
        #[pyclass(name=$python, module="fujimod.usb", eq, from_py_object)]
        #[derive(Clone,Copy,Debug,PartialEq)]
        enum $name { $($variant),+ }
        impl From<$name> for $target { fn from(value:$name)->Self { match value { $($name::$variant => <$target>::$variant),+ } } }
    };
}
enum_binding!(InfoSelector,"InfoSelector",info::InfoSelector,{Vendor,Manufacturer,Product,InternalVersion,DeviceType,Serial,Framework});
enum_binding!(ConfigurationView,"ConfigurationView",configuration::ConfigurationView,{Storage,Ram});
enum_binding!(KeyState,"KeyState",debug::KeyState,{On,Off});
enum_binding!(CameraMode,"CameraMode",debug::CameraMode,{Record,Playback});
enum_binding!(DataReadMode,"DataReadMode",diagnostic::DataReadMode,{Pattern,Echo});
enum_binding!(DataWriteMode,"DataWriteMode",diagnostic::DataWriteMode,{Preserve,FillOnes,Echo});
enum_binding!(OffsetMode,"OffsetMode",usb::operations::data::OffsetMode,{Checked,Wrapping});
enum_binding!(DeviceFilter,"DeviceFilter",usb::connection::DeviceFilter,{Fujifilm,Diagnostic});

#[pyclass(module = "fujimod.usb", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum SessionState {
    Ready,
    CompletionUnknown,
    Closed,
    ReadingRange,
}
#[pyclass(module = "fujimod.usb", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum CommandStatus {
    Success,
    DeviceError,
}
#[pyclass(module = "fujimod.usb", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum ConsoleProgress {
    Complete,
    Pending,
    Unknown,
}

#[pyclass(frozen, module = "fujimod.usb", from_py_object)]
#[derive(Clone)]
struct FirmwareLimits {
    memory: DebugMemoryLimits,
    configuration: NonZeroU32,
    console: NonZeroU32,
}
#[pymethods]
impl FirmwareLimits {
    #[new]
    #[pyo3(signature=(*,memory_read,memory_write,configuration,console))]
    fn new(
        memory_read: u32,
        memory_write: u32,
        configuration: u32,
        console: u32,
    ) -> PyResult<Self> {
        Ok(Self {
            memory: DebugMemoryLimits {
                read: nz(memory_read)?,
                write: nz(memory_write)?,
            },
            configuration: nz(configuration)?,
            console: nz(console)?,
        })
    }
    #[getter]
    fn memory_read(&self) -> u32 {
        self.memory.read.get()
    }
    #[getter]
    fn memory_write(&self) -> u32 {
        self.memory.write.get()
    }
    #[getter]
    fn configuration(&self) -> u32 {
        self.configuration.get()
    }
    #[getter]
    fn console(&self) -> u32 {
        self.console.get()
    }
}
#[pyclass(frozen, module = "fujimod.usb")]
#[derive(Clone)]
struct InterfaceSelection {
    inner: usb::connection::InterfaceSelection,
}
#[pymethods]
impl InterfaceSelection {
    #[new]
    #[pyo3(signature=(*,configuration,interface,alternate))]
    fn new(configuration: u8, interface: u8, alternate: u8) -> Self {
        Self {
            inner: usb::connection::InterfaceSelection {
                configuration,
                interface,
                alternate,
            },
        }
    }
    #[getter]
    fn configuration(&self) -> u8 {
        self.inner.configuration
    }
    #[getter]
    fn interface(&self) -> u8 {
        self.inner.interface
    }
    #[getter]
    fn alternate(&self) -> u8 {
        self.inner.alternate
    }
}
#[pyclass(frozen, module = "fujimod.usb")]
struct Device {
    inner: nusb::DeviceInfo,
}
#[pymethods]
impl Device {
    #[getter]
    fn vendor_id(&self) -> u16 {
        self.inner.vendor_id()
    }
    #[getter]
    fn product_id(&self) -> u16 {
        self.inner.product_id()
    }
    #[getter]
    fn bus_id(&self) -> &str {
        self.inner.bus_id()
    }
    #[getter]
    fn address(&self) -> u8 {
        self.inner.device_address()
    }
    #[getter]
    fn ports(&self) -> Vec<u8> {
        self.inner.port_chain().to_vec()
    }
    #[getter]
    fn manufacturer(&self) -> Option<&str> {
        self.inner.manufacturer_string()
    }
    #[getter]
    fn product(&self) -> Option<&str> {
        self.inner.product_string()
    }
    #[getter]
    fn serial(&self) -> Option<&str> {
        self.inner.serial_number()
    }
    fn interfaces(&self, py: Python<'_>) -> PyResult<Vec<InterfaceSelection>> {
        py.detach(|| block_on(usb::connection::interfaces(&self.inner)))
            .map(|items| {
                items
                    .into_iter()
                    .map(|inner| InterfaceSelection { inner })
                    .collect()
            })
            .map_err(|e| convert_error(py, e))
    }
}

#[pyclass(frozen, module = "fujimod.usb")]
struct DiagnosticTag {
    inner: diagnostic::DiagnosticTag,
}
#[pymethods]
impl DiagnosticTag {
    #[staticmethod]
    fn bulk_source() -> Self {
        Self {
            inner: diagnostic::DiagnosticTag::BulkSource,
        }
    }
    #[staticmethod]
    fn research(value: u32) -> Self {
        Self {
            inner: diagnostic::DiagnosticTag::Research(value),
        }
    }
    #[getter]
    fn value(&self) -> u32 {
        self.inner.value()
    }
}
#[pyclass(frozen, module = "fujimod.usb")]
struct LogReply {
    inner: debug::LogReply,
}
#[pymethods]
impl LogReply {
    #[getter]
    fn status(&self) -> u32 {
        self.inner.status
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.data)
    }
}

#[pyclass(frozen, module = "fujimod.usb")]
struct DiagnosticReply {
    inner: diagnostic::DiagnosticReply,
}
#[pymethods]
impl DiagnosticReply {
    #[getter]
    fn status(&self) -> CommandStatus {
        match self.inner.status {
            diagnostic::CommandStatus::Success => CommandStatus::Success,
            _ => CommandStatus::DeviceError,
        }
    }
    #[getter]
    fn primary(&self) -> u32 {
        match self.inner.status {
            diagnostic::CommandStatus::Success => 0,
            diagnostic::CommandStatus::DeviceError { primary, .. } => primary,
        }
    }
    #[getter]
    fn secondary(&self) -> u32 {
        match self.inner.status {
            diagnostic::CommandStatus::Success => 0,
            diagnostic::CommandStatus::DeviceError { secondary, .. } => secondary,
        }
    }
    #[getter]
    fn fields(&self) -> Vec<u32> {
        self.inner.fields.to_vec()
    }
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.raw)
    }
}
#[pyclass(frozen, module = "fujimod.usb")]
struct ConsoleReply {
    inner: debug::ConsoleReply,
}
#[pymethods]
impl ConsoleReply {
    #[getter]
    fn progress(&self) -> ConsoleProgress {
        match self.inner.progress {
            debug::ConsoleProgress::Complete => ConsoleProgress::Complete,
            debug::ConsoleProgress::Pending => ConsoleProgress::Pending,
            _ => ConsoleProgress::Unknown,
        }
    }
    #[getter]
    fn unknown_progress(&self) -> Option<u32> {
        match self.inner.progress {
            debug::ConsoleProgress::Unknown(n) => Some(n),
            _ => None,
        }
    }
    #[getter]
    fn status(&self) -> u32 {
        self.inner.status
    }
    #[getter]
    fn output<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.output)
    }
}
#[pyclass(frozen, module = "fujimod.usb")]
struct ConfigurationGeometry {
    inner: configuration::ConfigurationGeometry,
}
#[pymethods]
impl ConfigurationGeometry {
    #[getter]
    fn capacity(&self) -> u32 {
        self.inner.capacity
    }
    #[getter]
    fn page_bytes(&self) -> u32 {
        self.inner.page_bytes.get()
    }
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.raw)
    }
}

#[pyclass(module = "fujimod.usb.blocking")]
struct Session {
    inner: Arc<Mutex<SessionSlot>>,
    limits: FirmwareLimits,
}
enum SessionSlot {
    Open(ff80::Session),
    ReadingRange,
    Closed,
}

enum BindingError {
    Busy,
    Closed,
    Poisoned,
    Protocol(usb::Error),
}
impl Session {
    fn with<T: Send>(
        &self,
        py: Python<'_>,
        operation: impl FnOnce(&mut ff80::Session) -> usb::Result<T> + Send,
    ) -> PyResult<T> {
        let result = py.detach(|| {
            let mut guard = self.inner.lock().map_err(|_| BindingError::Poisoned)?;
            let session = match &mut *guard {
                SessionSlot::Open(session) => session,
                SessionSlot::Closed => return Err(BindingError::Closed),
                SessionSlot::ReadingRange => return Err(BindingError::Busy),
            };
            operation(session).map_err(BindingError::Protocol)
        });
        result.map_err(|error| match error {
            BindingError::Busy => {
                SessionBusyError::new_err("session is owned by an active range reader")
            }
            BindingError::Closed => ClosedSessionError::new_err("session is closed"),
            BindingError::Poisoned => {
                CompletionUnknownError::new_err("session mutex was poisoned; completion unknown")
            }
            BindingError::Protocol(error) => convert_error(py, error),
        })
    }
}
#[pymethods]
impl Session {
    #[staticmethod]
    #[pyo3(signature=(device,interface,limits,*,timeout_ms=5000,max_reply_bytes=8388608))]
    fn open(
        py: Python<'_>,
        device: &Device,
        interface: &InterfaceSelection,
        limits: FirmwareLimits,
        timeout_ms: u64,
        max_reply_bytes: u32,
    ) -> PyResult<Self> {
        let max_reply_bytes = nz(max_reply_bytes)?;
        let session = py
            .detach(|| {
                block_on(async {
                    Connection::open(
                        &device.inner,
                        interface.inner,
                        TransferOptions {
                            timeout: Duration::from_millis(timeout_ms),
                            max_reply_bytes,
                        },
                    )
                    .await?
                    .begin_session()
                    .await
                })
            })
            .map_err(|error| convert_error(py, error))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(SessionSlot::Open(session))),
            limits,
        })
    }
    #[getter]
    fn state(&self) -> PyResult<SessionState> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| CompletionUnknownError::new_err("session mutex was poisoned"))?;
        Ok(match &*guard {
            SessionSlot::Closed => SessionState::Closed,
            SessionSlot::ReadingRange => SessionState::ReadingRange,
            SessionSlot::Open(session) => match session.state() {
                ff80::SessionState::Ready => SessionState::Ready,
                ff80::SessionState::CompletionUnknown => SessionState::CompletionUnknown,
            },
        })
    }
    #[getter]
    fn packet_bytes(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |s| Ok(s.info().advertised_packet_bytes.get()))
    }
    #[getter]
    fn token(&self, py: Python<'_>) -> PyResult<u16> {
        self.with(py, |s| Ok(s.info().id))
    }
    fn ping<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |s| block_on(s.ping()))
            .map(|b| PyBytes::new(py, &b))
    }
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let session = {
            let mut slot = self
                .inner
                .lock()
                .map_err(|_| CompletionUnknownError::new_err("session mutex was poisoned"))?;
            match &*slot {
                SessionSlot::Closed => {
                    return Err(ClosedSessionError::new_err("session is closed"));
                }
                SessionSlot::ReadingRange => {
                    return Err(SessionBusyError::new_err(
                        "session is owned by an active range reader",
                    ));
                }
                SessionSlot::Open(_) => (),
            }
            match std::mem::replace(&mut *slot, SessionSlot::Closed) {
                SessionSlot::Open(session) => session,
                _ => unreachable!("validated slot under lock"),
            }
        };
        py.detach(|| block_on(session.close()))
            .map(|bytes| PyBytes::new(py, &bytes))
            .map_err(|e| convert_error(py, e))
    }
    fn device_info(&self, py: Python<'_>, selector: InfoSelector) -> PyResult<String> {
        self.with(py, |s| block_on(s.device_info(selector.into())))
            .map(|r| r.text)
    }
    fn read_memory<'py>(
        &self,
        py: Python<'py>,
        address: u32,
        length: u32,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let command = ReadMemory::new(
            MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?,
            self.limits.memory,
        )
        .map_err(|e| convert_error(py, e))?;
        self.with(py, |s| block_on(s.read_memory(command)))
            .map(|b| PyBytes::new(py, &b))
    }
    fn write_memory(&self, py: Python<'_>, address: u32, data: Vec<u8>) -> PyResult<()> {
        let command = WriteMemory::new(Address(address), data, self.limits.memory)
            .map_err(|e| convert_error(py, e))?;
        self.with(py, |s| block_on(s.write_memory(command)))
    }
    fn configuration_geometry(
        &self,
        py: Python<'_>,
        view: ConfigurationView,
    ) -> PyResult<ConfigurationGeometry> {
        self.with(py, |s| block_on(s.configuration_geometry(view.into())))
            .map(|inner| ConfigurationGeometry { inner })
    }
    #[pyo3(signature=(view,offset,length,*,page=0))]
    fn read_configuration<'py>(
        &self,
        py: Python<'py>,
        view: ConfigurationView,
        offset: u32,
        length: u32,
        page: u32,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let command = ReadConfiguration::new(
            ConfigurationLocation {
                view: view.into(),
                page,
                offset,
            },
            nz(length)?,
            self.limits.configuration,
        )
        .map_err(|e| convert_error(py, e))?;
        self.with(py, |s| block_on(s.read_configuration(command)))
            .map(|b| PyBytes::new(py, &b))
    }
    #[pyo3(signature=(view,offset,data,*,page=0))]
    fn write_configuration(
        &self,
        py: Python<'_>,
        view: ConfigurationView,
        offset: u32,
        data: Vec<u8>,
        page: u32,
    ) -> PyResult<()> {
        let command = WriteConfiguration::new(
            ConfigurationLocation {
                view: view.into(),
                page,
                offset,
            },
            data,
            self.limits.configuration,
        )
        .map_err(|e| convert_error(py, e))?;
        self.with(py, |s| block_on(s.write_configuration(command)))
    }
    /// An explicit selector value is a diagnostic tag, not an operation disposition.
    fn diagnostic_query(&self, py: Python<'_>, tag: &DiagnosticTag) -> PyResult<DiagnosticReply> {
        self.with(py, |s| block_on(s.diagnostic_query(tag.inner)))
            .map(|inner| DiagnosticReply { inner })
    }
    fn prepare_data_read(&self, py: Python<'_>, mode: DataReadMode) -> PyResult<DiagnosticReply> {
        self.with(py, |s| block_on(s.prepare_data_read(mode.into())))
            .map(|inner| DiagnosticReply { inner })
    }
    fn prepare_data_write(
        &self,
        py: Python<'_>,
        length: u32,
        mode: DataWriteMode,
    ) -> PyResult<DiagnosticReply> {
        let length = nz(length)?;
        self.with(py, |s| block_on(s.prepare_data_write(length, mode.into())))
            .map(|inner| DiagnosticReply { inner })
    }
    fn read_data<'py>(
        &self,
        py: Python<'py>,
        offset: u32,
        length: u32,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let length = nz(length)?;
        self.with(py, |s| block_on(s.read_data(offset, length)))
            .map(|b| PyBytes::new(py, &b))
    }
    fn write_data(&self, py: Python<'_>, data: Vec<u8>) -> PyResult<()> {
        self.with(py, |s| block_on(s.write_data(&data)))
    }
    fn empty_response<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |s| block_on(s.empty_response()))
            .map(|b| PyBytes::new(py, &b))
    }
    fn send_key<'py>(
        &self,
        py: Python<'py>,
        name: String,
        state: KeyState,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let name = debug::EventName::new(name).map_err(|e| convert_error(py, e))?;
        self.with(py, |s| {
            block_on(s.send_event(
                debug::DebugEvent::Key {
                    name,
                    state: state.into(),
                },
                self.limits.console,
            ))
        })
        .map(|b| PyBytes::new(py, &b))
    }
    fn request_mode<'py>(
        &self,
        py: Python<'py>,
        mode: CameraMode,
    ) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |s| {
            block_on(s.send_event(debug::DebugEvent::Mode(mode.into()), self.limits.console))
        })
        .map(|b| PyBytes::new(py, &b))
    }
    fn command_event<'py>(
        &self,
        py: Python<'py>,
        argument: String,
    ) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |s| {
            block_on(s.send_event(debug::DebugEvent::Command(argument), self.limits.console))
        })
        .map(|b| PyBytes::new(py, &b))
    }
    fn submit_console(&self, py: Python<'_>, data: Vec<u8>) -> PyResult<()> {
        let command = debug::ConsoleCommand::new(data, self.limits.console)
            .map_err(|e| convert_error(py, e))?;
        self.with(py, |s| block_on(s.submit_console(command)))
            .map(|_| ())
    }
    fn read_log(&self, py: Python<'_>, capacity: u32) -> PyResult<LogReply> {
        let capacity = nz(capacity)?;
        self.with(py, |s| block_on(s.read_log(capacity)))
            .map(|inner| LogReply { inner })
    }
    fn poll_console(&self, py: Python<'_>, capacity: u32) -> PyResult<ConsoleReply> {
        let capacity = nz(capacity)?;
        self.with(py, |s| block_on(s.poll_console(capacity)))
            .map(|inner| ConsoleReply { inner })
    }
}
#[pyfunction]
fn discover(py: Python<'_>, filter: DeviceFilter) -> PyResult<Vec<Device>> {
    py.detach(|| block_on(usb::connection::discover(filter.into())))
        .map(|items| items.into_iter().map(|inner| Device { inner }).collect())
        .map_err(|e| convert_error(py, e))
}

pub(crate) fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let usb = PyModule::new(py, "fujimod.usb")?;
    let blocking = PyModule::new(py, "fujimod.usb.blocking")?;
    m.add("__path__", PyList::empty(py))?;
    usb.add("__path__", PyList::empty(py))?;
    usb.add_class::<InfoSelector>()?;
    usb.add_class::<ConfigurationView>()?;
    usb.add_class::<KeyState>()?;
    usb.add_class::<CameraMode>()?;
    usb.add_class::<DataReadMode>()?;
    usb.add_class::<DataWriteMode>()?;
    usb.add_class::<DeviceFilter>()?;
    usb.add_class::<SessionState>()?;
    usb.add_class::<CommandStatus>()?;
    usb.add_class::<ConsoleProgress>()?;
    usb.add_class::<FirmwareLimits>()?;
    usb.add_class::<InterfaceSelection>()?;
    usb.add_class::<Device>()?;
    usb.add_class::<DiagnosticReply>()?;
    usb.add_class::<DiagnosticTag>()?;
    usb.add_class::<LogReply>()?;
    usb.add_class::<ConsoleReply>()?;
    usb.add_class::<ConfigurationGeometry>()?;
    usb.add("UsbError", py.get_type::<UsbError>())?;
    usb.add("TransferError", py.get_type::<TransferError>())?;
    usb.add("ProtocolError", py.get_type::<ProtocolError>())?;
    usb.add(
        "CompletionUnknownError",
        py.get_type::<CompletionUnknownError>(),
    )?;
    usb.add("DeviceCommandError", py.get_type::<DeviceCommandError>())?;
    usb.add("ClosedSessionError", py.get_type::<ClosedSessionError>())?;
    usb.add("SessionBusyError", py.get_type::<SessionBusyError>())?;
    blocking.add("__path__", PyList::empty(py))?;
    let memory = memory::module(py)?;
    let data = data::module(py)?;
    usb.add_class::<OffsetMode>()?;
    usb.add_class::<data::DataWindow>()?;
    blocking.add_class::<range::Reader>()?;
    blocking.add_submodule(&data)?;
    blocking.add_submodule(&memory)?;
    blocking.add_class::<Session>()?;
    blocking.add_function(wrap_pyfunction!(discover, &blocking)?)?;
    usb.add_submodule(&blocking)?;
    m.add_submodule(&usb)?;
    let modules = py
        .import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;
    modules.set_item("fujimod.usb", &usb)?;
    modules.set_item("fujimod.usb.blocking", &blocking)?;
    modules.set_item("fujimod.usb.blocking.memory", &memory)?;
    modules.set_item("fujimod.usb.blocking.data", &data)?;
    #[cfg(feature = "testing")]
    {
        let testing = testing::module(py)?;
        usb.add_submodule(&testing)?;
        modules.set_item("fujimod.usb.testing", &testing)?;
    }
    Ok(())
}

mod data;
mod memory;
mod range;

#[cfg(feature = "testing")]
mod testing;
