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

/// Owns the Rust session until iteration completes, closes, or is dropped.
#[pyclass(module = "fujimod.usb.blocking.memory")]
struct Reader {
    reader: Option<usb::operations::memory::MemoryReader<ff80::Session>>,
    owner: Arc<Mutex<SessionSlot>>,
    completed: u32,
}
impl Reader {
    fn restore(&mut self) {
        if let Some(reader) = self.reader.take() {
            self.completed = reader.bytes_completed();
            if let Ok(mut slot) = self.owner.lock()
                && matches!(*slot, SessionSlot::ReadingRange)
            {
                *slot = SessionSlot::Open(reader.into_inner());
            }
        }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.restore();
    }
}
#[pymethods]
impl Reader {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        let Some(reader) = self.reader.as_mut() else {
            return Ok(None);
        };
        let result = py.detach(|| block_on(reader.next_chunk()));
        self.completed = reader.bytes_completed();
        match result {
            Ok(Some(bytes)) => Ok(Some(PyBytes::new(py, &bytes))),
            Ok(None) => {
                self.restore();
                Ok(None)
            }
            Err(error) => {
                self.restore();
                Err(convert_error(py, error))
            }
        }
    }
    #[getter]
    fn bytes_completed(&self) -> u32 {
        self.completed
    }
    fn close(&mut self) {
        self.restore();
    }
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __exit__(
        &mut self,
        _kind: &Bound<'_, PyAny>,
        _value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> bool {
        self.restore();
        false
    }
}
#[pyfunction]
fn read_chunks(py: Python<'_>, session: &Session, address: u32, length: u32) -> PyResult<Reader> {
    let range =
        MemoryRange::new(Address(address), nz(length)?).map_err(|e| convert_error(py, e))?;
    let mut slot = session
        .inner
        .lock()
        .map_err(|_| CompletionUnknownError::new_err("session mutex was poisoned"))?;
    match &*slot {
        SessionSlot::Closed => return Err(ClosedSessionError::new_err("session is closed")),
        SessionSlot::ReadingRange => {
            return Err(SessionBusyError::new_err(
                "session is owned by an active range reader",
            ));
        }
        SessionSlot::Open(native) => {
            if native.state() != ff80::SessionState::Ready {
                return Err(CompletionUnknownError::new_err(
                    "session completion is unknown",
                ));
            }
        }
    }
    let SessionSlot::Open(native) = std::mem::replace(&mut *slot, SessionSlot::ReadingRange) else {
        unreachable!("validated slot under lock")
    };
    Ok(Reader {
        reader: Some(usb::operations::memory::MemoryReader::new(
            native,
            range,
            session.limits.memory,
        )),
        owner: session.inner.clone(),
        completed: 0,
    })
}

pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fujimod.usb.blocking.memory")?;
    module.add_function(wrap_pyfunction!(read_range, &module)?)?;
    module.add_function(wrap_pyfunction!(write_range, &module)?)?;
    module.add_class::<Reader>()?;
    module.add_function(wrap_pyfunction!(read_chunks, &module)?)?;
    Ok(module)
}
