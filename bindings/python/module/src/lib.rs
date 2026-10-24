//! The `fujimod._native` extension module. The bindings live in the
//! `fujimod-python` library; this crate links them with the EXR II board
//! builder, so a device model edit rebuilds the model, the board crate and this
//! crate only.
use pyo3::prelude::*;

#[pymodule]
fn _native(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    #[cfg(feature = "emulator")]
    fujimod_emulator_exr2_board::register_board_builder();
    fujimod_python::register(py, module)?;
    #[cfg(feature = "emulator")]
    fujimod_python_graphics::register(&module.getattr("emulator")?.cast_into::<PyModule>()?)?;
    Ok(())
}
