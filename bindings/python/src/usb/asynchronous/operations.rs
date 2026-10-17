//! Compositions use the Rust operations, separate from direct session methods.
use super::*;

#[pyfunction]
async fn read_range(session: AsyncSession, address: u32, length: u32) -> PyResult<Py<PyBytes>> {
    let range = MemoryRange::new(Address(address), nz(length)?).map_err(error)?;
    let limits = session.limits.memory;
    session
        .with(move |s| Box::pin(usb::operations::memory::read_range(s, range, limits)))
        .await
        .map(bytes)
}
#[pyfunction]
async fn write_range(session: AsyncSession, address: u32, data: Vec<u8>) -> PyResult<()> {
    let limits = session.limits.memory;
    session
        .with(move |s| {
            Box::pin(async move {
                usb::operations::memory::write_range(s, Address(address), &data, limits).await
            })
        })
        .await
}
#[pyfunction]
fn read_chunks(session: &AsyncSession, address: u32, length: u32) -> PyResult<reader::Reader> {
    let range = MemoryRange::new(Address(address), nz(length)?).map_err(error)?;
    reader::Reader::new(session, range::Operation::Memory(range))
}
#[pyfunction(name = "read_chunks")]
fn data_chunks(
    session: &AsyncSession,
    window: &data::DataWindow,
    address: u32,
    length: u32,
) -> PyResult<reader::Reader> {
    let range = MemoryRange::new(Address(address), nz(length)?).map_err(error)?;
    window.inner.validate(range).map_err(error)?;
    reader::Reader::new(session, range::Operation::Data(window.inner, range))
}
#[pyfunction]
#[pyo3(signature=(session,data,*,piece_bytes,mode))]
async fn upload(
    session: AsyncSession,
    data: Vec<u8>,
    piece_bytes: u32,
    mode: DataWriteMode,
) -> PyResult<data::UploadReply> {
    let piece_bytes = nz(piece_bytes)?;
    session
        .with(move |s| {
            Box::pin(async move {
                usb::operations::data::upload(s, &data, piece_bytes, mode.into()).await
            })
        })
        .await
        .map(|inner| data::UploadReply { inner })
}
#[pyfunction]
async fn exchange(
    session: AsyncSession,
    mailbox: app80::Mailbox,
    request: app80::Request,
) -> PyResult<Py<PyBytes>> {
    if session.limits.configuration.get() < 33 {
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
    invocation: app80::catalogue::Invocation,
) -> PyResult<app80::catalogue::CommandReply> {
    app80::catalogue::check_limit(
        &session.limits,
        invocation.inner.command().request().encoded_len(),
    )?;
    session
        .with(move |s| Box::pin(async move { invocation.inner.execute(s).await }))
        .await
        .map(|inner| app80::catalogue::CommandReply { inner })
}
pub(super) fn register(
    py: Python<'_>,
    module: &Bound<'_, PyModule>,
    app80: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let memory = PyModule::new(py, "fujimod.usb.memory")?;
    memory.add_function(wrap_pyfunction!(read_range, &memory)?)?;
    memory.add_function(wrap_pyfunction!(write_range, &memory)?)?;
    memory.add_function(wrap_pyfunction!(read_chunks, &memory)?)?;
    let data = PyModule::new(py, "fujimod.usb.data")?;
    data.add_function(wrap_pyfunction!(data_chunks, &data)?)?;
    data.add_function(wrap_pyfunction!(upload, &data)?)?;
    data.add_class::<data::UploadReply>()?;
    app80.add_function(wrap_pyfunction!(exchange, app80)?)?;
    app80.add_function(wrap_pyfunction!(execute, app80)?)?;
    module.add_submodule(&memory)?;
    module.add_submodule(&data)?;
    let modules = py
        .import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;
    modules.set_item("fujimod.usb.memory", memory)?;
    modules.set_item("fujimod.usb.data", data)?;
    Ok(())
}
