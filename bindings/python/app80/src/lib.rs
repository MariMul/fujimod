//! Python bindings for App80: mailbox wire types, the awaitable ff80 mailbox
//! exchange and firmware-qualified service commands, in `fujimod.usb.app80`.
use fujimod_python_usb::{
    asynchronous::{AsyncSession, bytes},
    convert_error, nz,
};
use fujimod_usb as usb;
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyBytes, PyDict, PyModule},
};

mod catalog;

#[pyclass(frozen, eq, hash, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct CommandCode {
    inner: fujimod_usb_app80::CommandCode,
}
#[pymethods]
impl CommandCode {
    #[new]
    fn new(value: u8) -> Self {
        Self {
            inner: fujimod_usb_app80::CommandCode(value),
        }
    }
    #[getter]
    fn value(&self) -> u8 {
        self.inner.0
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
struct Request {
    inner: fujimod_usb_app80::Request,
}
#[pymethods]
impl Request {
    #[new]
    fn new(py: Python<'_>, code: &CommandCode, arguments: Vec<u8>) -> PyResult<Self> {
        fujimod_usb_app80::Request::new(code.inner, &arguments)
            .map(|inner| Self { inner })
            .map_err(|e| convert_error(py, e))
    }
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.encode())
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
struct Mailbox {
    inner: fujimod_usb_app80::operations::Mailbox,
}
#[pymethods]
impl Mailbox {
    #[new]
    #[pyo3(signature=(*,page,offset,configuration_limit))]
    fn new(py: Python<'_>, page: u32, offset: u32, configuration_limit: u32) -> PyResult<Self> {
        fujimod_usb_app80::operations::Mailbox::new(page, offset, nz(configuration_limit)?)
            .map(|inner| Self { inner })
            .map_err(|e| convert_error(py, e))
    }
}

#[pyfunction]
async fn exchange(session: AsyncSession, mailbox: Mailbox, request: Request) -> PyResult<Py<PyBytes>> {
    if session.configuration_limit() < 33 {
        return Err(PyValueError::new_err(
            "App80 mailbox requires a 33-byte configuration command",
        ));
    }
    session
        .with(move |s| Box::pin(async move { mailbox.inner.exchange(s, &request.inner).await }))
        .await
        .map(|reply| bytes(reply.raw()))
}
#[pyfunction]
async fn execute(
    session: AsyncSession,
    invocation: catalog::Invocation,
) -> PyResult<catalog::CommandReply> {
    catalog::check_limit(
        session.configuration_limit(),
        invocation.inner.command().request().encoded_len(),
    )?;
    session
        .with(move |s| Box::pin(async move { invocation.inner.execute(s).await }))
        .await
        .map(|inner| catalog::CommandReply { inner })
}

/// Add `fujimod.usb.app80` to the `fujimod.usb` module the USB bindings registered on `parent`.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let usb = parent.getattr("usb")?.cast_into::<PyModule>()?;
    let module = PyModule::new(py, "fujimod.usb.app80")?;
    module.add_class::<CommandCode>()?;
    module.add_class::<Request>()?;
    module.add_class::<Mailbox>()?;
    catalog::register(py, &module)?;
    module.add_function(wrap_pyfunction!(exchange, &module)?)?;
    module.add_function(wrap_pyfunction!(execute, &module)?)?;
    usb.add_submodule(&module)?;
    py.import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?
        .set_item("fujimod.usb.app80", &module)?;
    Ok(())
}
