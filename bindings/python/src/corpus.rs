//! JSON conversion only; matching and queries live in fujimod-corpus.
use fujimod_corpus as core;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyModule};
use serde_json::{Value, json};
fn invalid(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}
#[pyclass(module = "fujimod.corpus")]
struct CorpusIndex {
    inner: core::Index,
}
#[pymethods]
impl CorpusIndex {
    #[new]
    fn new() -> Self {
        Self {
            inner: core::Index::new(),
        }
    }
    #[staticmethod]
    fn open(py: Python<'_>, path: String) -> PyResult<Self> {
        py.detach(|| core::Index::open(path))
            .map(|inner| Self { inner })
            .map_err(invalid)
    }
    fn add_unit(
        &mut self,
        py: Python<'_>,
        metadata: &str,
        regions: Vec<(u64, Vec<u8>)>,
    ) -> PyResult<()> {
        let input = serde_json::from_str(metadata).map_err(invalid)?;
        let regions = regions
            .into_iter()
            .map(|(address, bytes)| core::Region { address, bytes })
            .collect();
        py.detach(|| self.inner.add_unit(input, regions))
            .map_err(invalid)
    }
    fn build(&mut self, py: Python<'_>) {
        py.detach(|| self.inner.build());
    }
    fn save(&self, py: Python<'_>, path: String) -> PyResult<()> {
        py.detach(|| self.inner.save(path)).map_err(invalid)
    }
    fn merge(&mut self, py: Python<'_>, other: &Self) -> PyResult<()> {
        py.detach(|| self.inner.merge(other.inner.clone()))
            .map_err(invalid)
    }
    fn subset(&self, py: Python<'_>, firmwares: Vec<String>) -> Self {
        let firmwares = firmwares.into_iter().collect();
        Self {
            inner: py.detach(|| self.inner.subset(&firmwares)),
        }
    }
    fn query(&self, py: Python<'_>, operation: &str, arguments: &str) -> PyResult<String> {
        let a: Value = serde_json::from_str(arguments).map_err(invalid)?;
        py.detach(||->PyResult<String> {
            let s=|key:&str|a.get(key).and_then(Value::as_str);
            let id=||a.get("id").and_then(Value::as_u64).map(|v|v as usize).ok_or_else(||invalid("id is required"));
            let v=match operation {
                "stats"=> {
                    let mut firmwares=std::collections::BTreeMap::<&str,usize>::new();
                    for f in self.inner.functions() {*firmwares.entry(&f.key.firmware).or_default()+=1;}
                    Ok(json!({"functions":self.inner.functions().len(),"families":self.inner.families().len(),"firmwares":firmwares}))
                },
                "find"=>serde_json::to_value(self.inner.find(s("name"),a.get("address").and_then(Value::as_u64),s("firmware"),s("unit"))),
                "members"=>serde_json::to_value(self.inner.members(id()?,s("firmware"))),
                "family"=>serde_json::to_value(self.inner.families().get(id()?)),
                "normalized_tokens"=>serde_json::to_value(self.inner.normalized_tokens(id()?)),
                "calls"=>serde_json::to_value(self.inner.calls(id()?)),
                "data_references"=>serde_json::to_value(self.inner.data_references(id()?)),
                "references_to"=>serde_json::to_value(self.inner.references_to(s("firmware").ok_or_else(||invalid("firmware required"))?,s("unit"),a["address"].as_u64().ok_or_else(||invalid("address required"))?)),
                "pairs"=>serde_json::to_value(self.inner.pairs(s("reference").ok_or_else(||invalid("reference required"))?,s("other").ok_or_else(||invalid("other required"))?)),
                "names"=> {
                    let donors=a.get("donors").filter(|v|!v.is_null()).map(|v|serde_json::from_value(v.clone())).transpose().map_err(invalid)?;
                    serde_json::to_value(self.inner.inherited_names(id()?,donors.as_ref(),a.get("normalized_only").and_then(Value::as_bool).unwrap_or(false)))
                },
                "search"=> {
                    let pattern:Vec<u8>=serde_json::from_value(a["pattern"].clone()).map_err(invalid)?;
                    let mask:Option<Vec<u8>>=a.get("mask").map(|v|serde_json::from_value(v.clone())).transpose().map_err(invalid)?.flatten();
                    serde_json::to_value(self.inner.search(s("firmware").ok_or_else(||invalid("firmware required"))?,s("unit"),&pattern,mask.as_deref()).map_err(invalid)?)
                },
                _=>return Err(invalid("unknown corpus query")),
            }.map_err(invalid)?;
            serde_json::to_string(&v).map_err(invalid)
        })
    }
    fn annotate(&mut self, family: usize, annotation: &str) -> PyResult<()> {
        self.inner
            .annotate(family, serde_json::from_str(annotation).map_err(invalid)?)
            .map_err(invalid)
    }
}
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "corpus")?;
    module.add_class::<CorpusIndex>()?;
    parent.add_submodule(&module)?;
    Ok(())
}
