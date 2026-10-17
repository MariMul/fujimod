//! Composed memory operations, separate from direct session commands.
use super::*;

#[pyfunction]
fn read_range<'py>(
    py: Python<'py>,
    session: &Session,
    address: u32,
    length: u32,
) -> PyResult<Bound<'py, PyBytes>> {
    let range =
        MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?;
    session
        .with(py, |s| {
            block_on(usb::operations::memory::read_range(
                s,
                range,
                session.limits.memory,
            ))
        })
        .map(|b| PyBytes::new(py, &b))
}
#[pyfunction]
fn write_range(py: Python<'_>, session: &Session, address: u32, data: Vec<u8>) -> PyResult<()> {
    session.with(py, |s| {
        block_on(usb::operations::memory::write_range(
            s,
            Address(address),
            &data,
            session.limits.memory,
        ))
    })
}

#[pyfunction]
fn read_chunks(
    py: Python<'_>,
    session: &Session,
    address: u32,
    length: u32,
) -> PyResult<range::Reader> {
    let range =
        MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?;
    range::Reader::new(session, range::Operation::Memory(range))
}

pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fujimod.usb.blocking.memory")?;
    module.add_function(wrap_pyfunction!(read_range, &module)?)?;
    module.add_function(wrap_pyfunction!(write_range, &module)?)?;
    module.add_function(wrap_pyfunction!(read_chunks, &module)?)?;
    Ok(module)
}
