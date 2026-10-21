//! Language conversion for the shared firmware-data loader; no USB operations.
use fujimod_catalog::service as core;
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyList, PyModule, PyTuple},
};

include!(concat!(env!("OUT_DIR"), "/definition_codec_python.rs"));
pyo3::create_exception!(fujimod.catalog, DefinitionError, PyValueError);

/// Resolve a checked body and firmware selection from the bundled catalog.
#[pyfunction]
fn resolve_reference(body_id: &str, firmware_id: &str) -> PyResult<String> {
    let selection = fujimod_catalog::ReferenceCatalog::bundled()
        .and_then(|catalog| catalog.resolve(body_id, firmware_id))
        .map_err(PyValueError::new_err)?;
    serde_json::to_string(&selection).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Resolve a checked firmware selection and its body from the bundled catalog.
#[pyfunction]
fn resolve_firmware(firmware_id: &str) -> PyResult<String> {
    let selection = fujimod_catalog::ReferenceCatalog::bundled()
        .and_then(|catalog| catalog.resolve_firmware(firmware_id))
        .map_err(PyValueError::new_err)?;
    serde_json::to_string(&selection).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Resolve and validate a named run configuration and all reference identities.
#[pyfunction]
fn resolve_configuration(configuration_id: &str) -> PyResult<String> {
    let resolved = fujimod_catalog::ResolvedConfiguration::bundled(configuration_id)
        .map_err(PyValueError::new_err)?;
    serde_json::to_string(&resolved).map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyclass(frozen, from_py_object, module = "fujimod.catalog")]
#[derive(Clone)]
enum DefinitionFailure {
    Json { line: usize, column: usize },
    Schema { schema: u32 },
    EmptyIdentity {},
    InvalidDigest {},
    FirmwareMismatch {},
    EmptyModes {},
    InvalidMailbox {},
    InvalidLayout { code: u8, codec: Codec },
    SourceCount {},
    EmptyLabel {},
}
fn invalid(error: core::DefinitionError) -> PyErr {
    let exception = DefinitionError::new_err(error.to_string());
    let reason = match error {
        core::DefinitionError::Json(error) => DefinitionFailure::Json {
            line: error.line(),
            column: error.column(),
        },
        core::DefinitionError::Schema(schema) => DefinitionFailure::Schema { schema },
        core::DefinitionError::EmptyIdentity => DefinitionFailure::EmptyIdentity {},
        core::DefinitionError::InvalidDigest => DefinitionFailure::InvalidDigest {},
        core::DefinitionError::FirmwareMismatch => DefinitionFailure::FirmwareMismatch {},
        core::DefinitionError::EmptyModes => DefinitionFailure::EmptyModes {},
        core::DefinitionError::InvalidMailbox => DefinitionFailure::InvalidMailbox {},
        core::DefinitionError::InvalidLayout { code, codec } => DefinitionFailure::InvalidLayout {
            code,
            codec: codec.into(),
        },
        core::DefinitionError::SourceCount => DefinitionFailure::SourceCount {},
        core::DefinitionError::EmptyLabel => DefinitionFailure::EmptyLabel {},
    };
    Python::attach(|py| match exception.value(py).setattr("reason", reason) {
        Ok(()) => exception,
        Err(error) => error,
    })
}

#[pyclass(frozen, from_py_object, module = "fujimod.catalog")]
#[derive(Clone)]
pub(crate) struct FirmwareIdentity {
    pub(crate) inner: core::FirmwareIdentity,
}
#[pymethods]
impl FirmwareIdentity {
    #[new]
    fn new(id: String, sha256: &str) -> PyResult<Self> {
        core::FirmwareIdentity::new(id, sha256)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[getter]
    fn id(&self) -> &str {
        self.inner.id()
    }
    #[getter]
    fn sha256(&self) -> String {
        self.inner.digest().hex()
    }
}

#[pyclass(frozen, eq, from_py_object, module = "fujimod.catalog.service")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServiceModes {
    pub(crate) inner: core::ServiceModes,
}
#[pymethods]
impl ServiceModes {
    #[new]
    fn new(mask: u32) -> PyResult<Self> {
        core::ServiceModes::new(mask)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[getter]
    fn mask(&self) -> u32 {
        self.inner.mask()
    }
}

#[pyclass(frozen, from_py_object, module = "fujimod.catalog.service")]
#[derive(Clone)]
enum ModeAvailability {
    Modes { modes: ServiceModes },
    Disabled {},
    Unreviewed {},
}
impl From<core::ModeAvailability> for ModeAvailability {
    fn from(value: core::ModeAvailability) -> Self {
        match value {
            core::ModeAvailability::Modes(inner) => Self::Modes {
                modes: ServiceModes { inner },
            },
            core::ModeAvailability::Disabled => Self::Disabled {},
            core::ModeAvailability::Unreviewed => Self::Unreviewed {},
        }
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.catalog.service")]
#[derive(Clone)]
enum ModeTransition {
    Keep {},
    Set { modes: ServiceModes },
}
impl From<core::ModeTransition> for ModeTransition {
    fn from(value: core::ModeTransition) -> Self {
        match value {
            core::ModeTransition::Keep => Self::Keep {},
            core::ModeTransition::Set(inner) => Self::Set {
                modes: ServiceModes { inner },
            },
        }
    }
}
#[pyclass(frozen, module = "fujimod.catalog.service")]
struct Entry {
    inner: core::Entry,
}
#[pymethods]
impl Entry {
    #[getter]
    fn code(&self) -> u8 {
        self.inner.code()
    }
    #[getter]
    fn label(&self) -> &str {
        self.inner.label()
    }
    #[getter]
    fn codec(&self) -> Codec {
        self.inner.codec().into()
    }
    #[getter]
    fn slots<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.inner.slots().iter().copied())
    }
    #[getter]
    fn enabled_modes(&self) -> ModeAvailability {
        self.inner.enabled_modes().into()
    }
    #[getter]
    fn transition(&self) -> ModeTransition {
        self.inner.transition().into()
    }
    #[getter]
    fn firmware_kind(&self) -> u32 {
        self.inner.firmware_kind()
    }
    #[getter]
    fn handler(&self) -> u64 {
        self.inner.handler()
    }
    #[getter]
    fn parameter_table(&self) -> u64 {
        self.inner.parameter_table()
    }
}
#[pyclass(frozen, module = "fujimod.catalog.service")]
struct Mailbox {
    inner: core::Mailbox,
}
#[pymethods]
impl Mailbox {
    #[getter]
    fn page(&self) -> u32 {
        self.inner.page()
    }
    #[getter]
    fn offset(&self) -> u32 {
        self.inner.offset()
    }
    #[getter]
    fn configuration_limit(&self) -> u32 {
        self.inner.configuration_limit().get()
    }
}
#[pyclass(frozen, from_py_object, module = "fujimod.catalog.service")]
#[derive(Clone)]
pub(crate) struct Definition {
    pub(crate) inner: core::Definition,
}
#[pymethods]
impl Definition {
    #[staticmethod]
    fn from_json(json: &str, expected: &FirmwareIdentity) -> PyResult<Self> {
        core::Definition::from_json(json, &expected.inner)
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    #[getter]
    fn identity(&self) -> FirmwareIdentity {
        FirmwareIdentity {
            inner: self.inner.identity().clone(),
        }
    }
    #[getter]
    fn model(&self) -> &str {
        self.inner.model()
    }
    #[getter]
    fn version(&self) -> &str {
        self.inner.version()
    }
    #[getter]
    fn mailbox(&self) -> Mailbox {
        Mailbox {
            inner: self.inner.mailbox(),
        }
    }
    #[getter]
    fn entries(&self) -> Vec<Entry> {
        self.inner
            .entries()
            .iter()
            .cloned()
            .map(|inner| Entry { inner })
            .collect()
    }
    fn select(&self, code: u8, modes: &ServiceModes) -> Option<Entry> {
        self.inner
            .select(code, modes.inner)
            .cloned()
            .map(|inner| Entry { inner })
    }
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let catalog = PyModule::new(py, "fujimod.catalog")?;
    catalog.add_function(wrap_pyfunction!(resolve_reference, &catalog)?)?;
    catalog.add_function(wrap_pyfunction!(resolve_firmware, &catalog)?)?;
    catalog.add_function(wrap_pyfunction!(resolve_configuration, &catalog)?)?;
    catalog.add("__path__", PyList::empty(py))?;
    catalog.add_class::<FirmwareIdentity>()?;
    catalog.add("DefinitionError", py.get_type::<DefinitionError>())?;
    catalog.add_class::<DefinitionFailure>()?;
    let service = PyModule::new(py, "fujimod.catalog.service")?;
    service.add_class::<ServiceModes>()?;
    service.add_class::<Codec>()?;
    service.add_class::<ModeAvailability>()?;
    service.add_class::<ModeTransition>()?;
    service.add_class::<Entry>()?;
    service.add_class::<Mailbox>()?;
    service.add_class::<Definition>()?;
    catalog.add_submodule(&service)?;
    parent.add_submodule(&catalog)?;
    let modules = py
        .import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;
    modules.set_item("fujimod.catalog", &catalog)?;
    modules.set_item("fujimod.catalog.service", &service)?;
    Ok(())
}
