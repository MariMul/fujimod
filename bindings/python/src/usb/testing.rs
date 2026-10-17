//! Optional synthetic exchanges; never exposes a production connection path.
use super::*;

#[pyclass(frozen, from_py_object, module = "fujimod.usb.testing")]
#[derive(Clone)]
struct Exchange {
    inner: usb::testing::Exchange,
}
#[pymethods]
impl Exchange {
    #[staticmethod]
    fn control(bytes: Vec<u8>) -> PyResult<Self> {
        let bytes = bytes
            .try_into()
            .map_err(|_| PyValueError::new_err("control envelope must have 12 bytes"))?;
        Ok(Self {
            inner: usb::testing::Exchange::Control(bytes),
        })
    }
    #[staticmethod]
    fn write(bytes: Vec<u8>) -> Self {
        Self {
            inner: usb::testing::Exchange::Write(bytes),
        }
    }
    #[staticmethod]
    fn read(maximum: u32, data: Vec<u8>) -> Self {
        Self {
            inner: usb::testing::Exchange::Read {
                maximum: maximum as usize,
                data,
            },
        }
    }
    #[staticmethod]
    fn disconnected(maximum: u32, partial: Vec<u8>) -> Self {
        Self {
            inner: usb::testing::Exchange::ReadFailure {
                maximum: maximum as usize,
                error: nusb::transfer::TransferError::Disconnected,
                partial,
            },
        }
    }
}
#[pyclass(frozen, module = "fujimod.usb.testing")]
struct Script {
    inner: usb::testing::Script,
}
#[pymethods]
impl Script {
    #[getter]
    fn remaining(&self) -> usize {
        self.inner.remaining()
    }
}
#[pyfunction]
#[pyo3(signature=(exchanges,limits,*,max_reply_bytes=8388608))]
fn session(
    py: Python<'_>,
    exchanges: Vec<Exchange>,
    limits: FirmwareLimits,
    max_reply_bytes: u32,
) -> PyResult<(Session, Script)> {
    let script = usb::testing::Script::new(exchanges.into_iter().map(|e| e.inner));
    let native = py
        .detach(|| block_on(script.begin_session(max_reply_bytes)))
        .map_err(|e| convert_error(py, e))?;
    Ok((
        Session {
            inner: Arc::new(Mutex::new(SessionSlot::Open(native))),
            limits,
        },
        Script { inner: script },
    ))
}
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fujimod.usb.testing")?;
    module.add_class::<Exchange>()?;
    module.add_class::<Script>()?;
    module.add_function(wrap_pyfunction!(session, &module)?)?;
    Ok(module)
}
