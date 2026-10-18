//! App80 wire types for the awaitable ff80 mailbox operation.
use super::*;
pub(super) mod catalogue;

#[pyclass(frozen, eq, hash, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct CommandCode {
    inner: usb::app80::CommandCode,
}
#[pymethods]
impl CommandCode {
    #[new]
    fn new(value: u8) -> Self {
        Self {
            inner: usb::app80::CommandCode(value),
        }
    }
    #[getter]
    fn value(&self) -> u8 {
        self.inner.0
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.usb.app80")]
#[derive(Clone)]
pub(super) struct Request {
    pub(super) inner: usb::app80::Request,
}
#[pymethods]
impl Request {
    #[new]
    fn new(py: Python<'_>, code: &CommandCode, arguments: Vec<u8>) -> PyResult<Self> {
        usb::app80::Request::new(code.inner, &arguments)
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
pub(super) struct Mailbox {
    pub(super) inner: usb::operations::app80::Mailbox,
}
#[pymethods]
impl Mailbox {
    #[new]
    #[pyo3(signature=(*,page,offset,configuration_limit))]
    fn new(py: Python<'_>, page: u32, offset: u32, configuration_limit: u32) -> PyResult<Self> {
        usb::operations::app80::Mailbox::new(page, offset, nz(configuration_limit)?)
            .map(|inner| Self { inner })
            .map_err(|e| convert_error(py, e))
    }
}

pub(super) fn types_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fujimod.usb.app80")?;
    module.add_class::<CommandCode>()?;
    module.add_class::<Request>()?;
    module.add_class::<Mailbox>()?;
    catalogue::register(py, &module)?;
    Ok(module)
}
