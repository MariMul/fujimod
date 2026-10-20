//! Shared FujiMod Python package initialization.
use pyo3::prelude::*;
mod corpus;
mod defs;
mod firmware;
mod usb;

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    firmware::register(py, module, "fujimod.firmware")?;
    corpus::register(py, module)?;
    defs::register(py, module)?;
    usb::register(py, module)
}
