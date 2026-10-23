//! The `fujimod._native` extension module. The bindings live in the
//! `fujimod-python` library; this crate only links them, so an edit below it
//! relinks the module without recompiling much here.
use pyo3::prelude::*;

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    fujimod_python::register(py, module)
}
