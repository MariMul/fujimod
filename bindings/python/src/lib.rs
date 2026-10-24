//! Shared FujiMod Python package initialization.
use pyo3::prelude::*;
mod catalog;
mod corpus;
#[cfg(feature = "emulator")]
pub mod emulator;
mod firmware;
mod usb;
mod recording;
mod remote;

/// Register every submodule on `fujimod._native` (built by
/// `fujimod-python-module`).
pub fn register(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    firmware::register(py, module, "fujimod.firmware")?;
    recording::register(py, module)?;
    remote::register(py, module)?;
    corpus::register(py, module)?;
    catalog::register(py, module)?;
    usb::register(py, module)?;
    #[cfg(feature = "emulator")]
    emulator::register(py, module)?;
    Ok(())
}
