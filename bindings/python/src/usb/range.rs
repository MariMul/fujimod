//! Shared ownership for composed memory and diagnostic-data range readers.
use super::*;

pub(super) enum Operation {
    Memory(MemoryRange),
    Data(usb::operations::data::DataWindow, MemoryRange),
}
enum NativeReader {
    Memory(usb::operations::memory::MemoryReader<ff80::Session>),
    Data(usb::operations::data::DataReader<ff80::Session>),
}
impl NativeReader {
    fn bytes_completed(&self) -> u32 {
        match self {
            Self::Memory(r) => r.bytes_completed(),
            Self::Data(r) => r.bytes_completed(),
        }
    }
    fn into_inner(self) -> ff80::Session {
        match self {
            Self::Memory(r) => r.into_inner(),
            Self::Data(r) => r.into_inner(),
        }
    }
    async fn next_chunk(&mut self) -> usb::Result<Option<Vec<u8>>> {
        match self {
            Self::Memory(r) => r.next_chunk().await,
            Self::Data(r) => r.next_chunk().await,
        }
    }
}

/// Owns the Rust session until iteration completes, closes, or is dropped.
#[pyclass(module = "fujimod.usb.blocking")]
pub(super) struct Reader {
    reader: Option<NativeReader>,
    owner: Arc<Mutex<SessionSlot>>,
    completed: u32,
}
impl Reader {
    pub(super) fn new(session: &Session, operation: Operation) -> PyResult<Self> {
        let mut slot = session
            .inner
            .lock()
            .map_err(|_| CompletionUnknownError::new_err("session mutex was poisoned"))?;
        match &*slot {
            SessionSlot::Closed | SessionSlot::Abandoned => {
                return Err(ClosedSessionError::new_err(
                    "session is no longer available",
                ));
            }
            SessionSlot::CompletionUnknown => {
                return Err(CompletionUnknownError::new_err(
                    "session close did not complete; further I/O is unavailable",
                ));
            }
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
        let SessionSlot::Open(native) = std::mem::replace(&mut *slot, SessionSlot::ReadingRange)
        else {
            unreachable!("validated slot under lock")
        };
        let reader = match operation {
            Operation::Memory(range) => NativeReader::Memory(
                usb::operations::memory::MemoryReader::new(native, range, session.limits.memory),
            ),
            Operation::Data(window, range) => NativeReader::Data(
                usb::operations::data::DataReader::new(native, window, range)
                    .expect("window validated before leasing the session"),
            ),
        };
        Ok(Self {
            reader: Some(reader),
            owner: session.inner.clone(),
            completed: 0,
        })
    }
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
