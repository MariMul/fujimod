//! Shared FujiMod Python package initialization.
use pyo3::prelude::*;
mod usb;

#[pymodule]
fn fujimod(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    usb::register(py, module)
}
