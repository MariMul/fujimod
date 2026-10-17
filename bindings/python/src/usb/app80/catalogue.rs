//! Enum conversion and session adapters for the Rust service catalogue.
use super::*;
use crate::defs::{Codec, Definition, ServiceModes};
use usb::app80::catalogue as core;
mod values {
    include!(concat!(env!("OUT_DIR"), "/service_values_python.rs"));
}
use values::*;
pyo3::create_exception!(fujimod.usb.app80, CommandError, PyValueError);

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
        self.inner
            .command()
            .decode_bytes(&raw)
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
    fn prepare_code(
        &self,
        py: Python<'_>,
        code: &CommandCode,
        modes: &ServiceModes,
        command: Command,
    ) -> PyResult<Invocation> {
        self.inner
            .prepare_code(code.inner, modes.inner, command.into())
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
    check_limit(
        &session.limits,
        invocation.inner.command().request().encoded_len(),
    )?;
    session
        .with(py, |s| block_on(invocation.inner.execute(s)))
        .map(|inner| CommandReply { inner })
}
pub(in crate::usb) fn check_limit(limits: &FirmwareLimits, length: usize) -> PyResult<()> {
    if (limits.configuration.get() as usize) < length {
        return Err(PyValueError::new_err(
            "session configuration limit cannot carry this service mailbox",
        ));
    }
    Ok(())
}
pub(super) fn register(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    values::register(module)?;
    module.add("CommandError", py.get_type::<CommandError>())?;

    module.add_class::<CommandFailure>()?;

    module.add_class::<CommandReply>()?;
    module.add_class::<Invocation>()?;
    module.add_class::<Service>()?;
    Ok(())
}
pub(super) fn register_blocking(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execute, module)?)
}
