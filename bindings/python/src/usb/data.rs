//! Composed E1 range operations with an explicit firmware mapping.
use super::*;

#[pyclass(module = "fujimod.usb", frozen)]
#[derive(Clone)]
pub(super) struct DataWindow {
    inner: usb::operations::data::DataWindow,
}
#[pymethods]
impl DataWindow {
    #[new]
    fn new(
        py: Python<'_>,
        base: u32,
        address: u32,
        length: u32,
        capacity: u32,
        offset_mode: OffsetMode,
    ) -> PyResult<Self> {
        let aperture =
            MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?;
        let inner = usb::operations::data::DataWindow::new(
            Address(base),
            aperture,
            nz(capacity)?,
            offset_mode.into(),
        )
        .map_err(|e| convert_error(py, e))?;
        Ok(Self { inner })
    }
    #[getter]
    fn capacity(&self) -> u32 {
        self.inner.capacity().get()
    }
}

#[pyfunction]
fn read_chunks(
    py: Python<'_>,
    session: &Session,
    window: &DataWindow,
    address: u32,
    length: u32,
) -> PyResult<range::Reader> {
    let range =
        MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?;
    window
        .inner
        .validate(range)
        .map_err(|e| convert_error(py, e))?;
    range::Reader::new(session, range::Operation::Data(window.inner, range))
}

#[pyclass(frozen, module = "fujimod.usb.blocking.data")]
struct UploadReply {
    inner: usb::operations::data::UploadReply,
}
#[pymethods]
impl UploadReply {
    #[getter]
    fn setup(&self) -> DiagnosticReply {
        DiagnosticReply {
            inner: self.inner.setup.clone(),
        }
    }
    #[getter]
    fn acknowledgement<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.acknowledgement)
    }
    #[getter]
    fn bytes_written(&self) -> u32 {
        self.inner.bytes_written
    }
}

#[pyfunction]
#[pyo3(signature=(session,data,*,piece_bytes,mode))]
fn upload(
    py: Python<'_>,
    session: &Session,
    data: Vec<u8>,
    piece_bytes: u32,
    mode: DataWriteMode,
) -> PyResult<UploadReply> {
    let piece_bytes = nz(piece_bytes)?;
    session
        .with(py, |s| {
            block_on(usb::operations::data::upload(
                s,
                &data,
                piece_bytes,
                mode.into(),
            ))
        })
        .map(|inner| UploadReply { inner })
}

pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fujimod.usb.blocking.data")?;
    module.add_function(wrap_pyfunction!(read_chunks, &module)?)?;
    module.add_function(wrap_pyfunction!(upload, &module)?)?;
    module.add_class::<UploadReply>()?;
    Ok(module)
}
