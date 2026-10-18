use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Binary(#[from] bincode::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    Arm32,
    Aarch64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FunctionKey {
    pub firmware: String,
    pub unit: String,
    pub address: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Region {
    pub address: u64,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    pub kind: String,
    pub text: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Boundary {
    pub address: u64,
    /// Half-open byte ranges, in execution/listing order.
    pub ranges: Vec<(u64, u64)>,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(default)]
    pub references: Vec<Reference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reference {
    pub site: u64,
    pub target: u64,
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitInput {
    pub firmware: String,
    pub unit: String,
    pub architecture: Architecture,
    /// Pointer windows, including aliases, supplied by the caller. No camera map is built in.
    pub address_ranges: Vec<(u64, u64)>,
    pub functions: Vec<Boundary>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub input: UnitInput,
    pub regions: Vec<Region>,
}
impl Unit {
    pub fn read(&self, address: u64, size: usize) -> Option<&[u8]> {
        self.regions.iter().find_map(|r| {
            let offset = usize::try_from(address.checked_sub(r.address)?).ok()?;
            r.bytes.get(offset..offset.checked_add(size)?)
        })
    }
    pub fn pointer(&self, value: u64) -> bool {
        self.input
            .address_ranges
            .iter()
            .any(|&(lo, hi)| lo <= value && value < hi)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Function {
    pub key: FunctionKey,
    pub architecture: Architecture,
    pub names: Vec<String>,
    pub annotations: Vec<Annotation>,
    pub ranges: Vec<(u64, u64)>,
    pub tokens: Vec<u32>,
    pub signature: [u8; 32],
    pub instructions: usize,
    /// Token offset and absolute target. Resolution always keeps the unit identity.
    pub calls: Vec<(usize, u64)>,
    pub references: Vec<Reference>,
    pub constants: BTreeSet<u64>,
    pub strings: BTreeSet<String>,
    pub ngrams: BTreeSet<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Grade {
    Identical,
    SameCode,
    CallSite,
    Weak,
}
impl Grade {
    pub fn normalized(self) -> bool {
        matches!(self, Self::Identical | Self::SameCode)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub reference: FunctionKey,
    pub other: FunctionKey,
    pub grade: Grade,
    pub similarity: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Family {
    pub id: usize,
    pub members: Vec<usize>,
    pub annotations: Vec<Annotation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionView {
    pub id: usize,
    pub family: usize,
    pub key: FunctionKey,
    pub names: Vec<String>,
    pub instructions: usize,
    pub signature: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub(crate) units: Vec<Unit>,
    pub(crate) functions: Vec<Function>,
    pub(crate) tokens: Vec<String>,
    pub(crate) families: Vec<Family>,
    pub(crate) membership: Vec<usize>,
    pub(crate) weak: Vec<(usize, usize)>,
    #[serde(skip)]
    pub(crate) intern: HashMap<String, u32>,
    #[serde(skip)]
    pub(crate) keys: HashMap<FunctionKey, usize>,
    #[serde(skip)]
    pub(crate) targets: HashMap<(String, u64), Vec<usize>>,
}
impl Index {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }
    pub fn families(&self) -> &[Family] {
        &self.families
    }
    pub(crate) fn reindex(&mut self) {
        self.intern = self
            .tokens
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i as u32))
            .collect();
        self.keys = self
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.key.clone(), i))
            .collect();
        self.targets.clear();
        for (i, f) in self.functions.iter().enumerate() {
            self.targets
                .entry((f.key.firmware.clone(), f.key.address))
                .or_default()
                .push(i);
        }
    }
    pub fn add_unit(&mut self, input: UnitInput, mut regions: Vec<Region>) -> Result<()> {
        if input.firmware.is_empty() || input.unit.is_empty() {
            return Err(Error::Invalid("empty firmware/unit".into()));
        }
        if self
            .units
            .iter()
            .any(|u| u.input.firmware == input.firmware && u.input.unit == input.unit)
        {
            return Err(Error::Invalid("duplicate firmware unit".into()));
        }
        regions.sort_by_key(|r| r.address);
        for r in &regions {
            r.address
                .checked_add(r.bytes.len() as u64)
                .ok_or_else(|| Error::Invalid("region overflow".into()))?;
        }
        if regions
            .windows(2)
            .any(|r| r[0].address + r[0].bytes.len() as u64 > r[1].address)
        {
            return Err(Error::Invalid("overlapping regions in one unit".into()));
        }
        let unit = Unit { input, regions };
        let mut seen = BTreeSet::new();
        for b in &unit.input.functions {
            if !seen.insert(b.address)
                || b.ranges.is_empty()
                || b.ranges
                    .iter()
                    .any(|&(lo, hi)| hi <= lo || unit.read(lo, (hi - lo) as usize).is_none())
            {
                return Err(Error::Invalid(format!(
                    "invalid/unmapped boundary {:#x}",
                    b.address
                )));
            }
        }
        let (decoded, local) = crate::decode::decode(&unit)?;
        let start = self.functions.len();
        self.functions.extend(decoded);
        let remap: Vec<u32> = local
            .into_iter()
            .map(|s| {
                let n = self.tokens.len() as u32;
                *self.intern.entry(s.clone()).or_insert_with(|| {
                    self.tokens.push(s);
                    n
                })
            })
            .collect();
        for f in &mut self.functions[start..] {
            for t in &mut f.tokens {
                *t = remap[*t as usize];
            }
        }
        self.units.push(unit);
        self.families.clear();
        self.membership.clear();
        self.weak.clear();
        self.reindex();
        Ok(())
    }
    /// Merge normalized indexes without decoding either corpus again.
    pub fn merge(&mut self, other: Self) -> Result<()> {
        if other.units.iter().any(|u| {
            self.units
                .iter()
                .any(|v| u.input.firmware == v.input.firmware && u.input.unit == v.input.unit)
        }) {
            return Err(Error::Invalid("duplicate firmware unit in merge".into()));
        }
        if other
            .functions
            .iter()
            .any(|f| self.keys.contains_key(&f.key))
        {
            return Err(Error::Invalid("duplicate function in merge".into()));
        }
        let remap: Vec<u32> = other
            .tokens
            .into_iter()
            .map(|s| {
                let n = self.tokens.len() as u32;
                *self.intern.entry(s.clone()).or_insert_with(|| {
                    self.tokens.push(s);
                    n
                })
            })
            .collect();
        for mut f in other.functions {
            for t in &mut f.tokens {
                *t = remap[*t as usize];
            }
            self.functions.push(f);
        }
        self.units.extend(other.units);
        self.build();
        Ok(())
    }
    /// Recompute families using only selected donor firmware, retaining original features.
    pub fn subset(&self, firmwares: &BTreeSet<String>) -> Self {
        let mut out = Self {
            units: self
                .units
                .iter()
                .filter(|u| firmwares.contains(&u.input.firmware))
                .cloned()
                .collect(),
            functions: self
                .functions
                .iter()
                .filter(|f| firmwares.contains(&f.key.firmware))
                .cloned()
                .collect(),
            tokens: self.tokens.clone(),
            ..Self::new()
        };
        out.build();
        out
    }
    pub(crate) fn resolve(&self, caller: usize, address: u64) -> Option<usize> {
        let k = &self.functions[caller].key;
        self.keys
            .get(&FunctionKey {
                firmware: k.firmware.clone(),
                unit: k.unit.clone(),
                address,
            })
            .copied()
            .or_else(|| {
                let v = self.targets.get(&(k.firmware.clone(), address))?;
                (v.len() == 1).then_some(v[0])
            })
    }
    pub fn view(&self, id: usize) -> FunctionView {
        let f = &self.functions[id];
        FunctionView {
            id,
            family: self.membership.get(id).copied().unwrap_or(id),
            key: f.key.clone(),
            names: f.names.clone(),
            instructions: f.instructions,
            signature: f.signature.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
    pub fn find(
        &self,
        name: Option<&str>,
        address: Option<u64>,
        firmware: Option<&str>,
        unit: Option<&str>,
    ) -> Vec<FunctionView> {
        self.functions
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                firmware.is_none_or(|x| f.key.firmware == x)
                    && unit.is_none_or(|x| f.key.unit == x)
                    && address.is_none_or(|x| f.key.address == x)
                    && name.is_none_or(|x| {
                        f.names.iter().any(|n| n == x)
                            || self
                                .membership
                                .get(self.keys[&f.key])
                                .is_some_and(|&family| {
                                    self.families[family]
                                        .members
                                        .iter()
                                        .any(|&i| self.functions[i].names.iter().any(|n| n == x))
                                })
                    })
            })
            .map(|(i, _)| self.view(i))
            .collect()
    }
    pub fn members(&self, family: usize, firmware: Option<&str>) -> Vec<FunctionView> {
        self.families
            .get(family)
            .into_iter()
            .flat_map(|f| &f.members)
            .filter(|&&i| firmware.is_none_or(|x| self.functions[i].key.firmware == x))
            .map(|&i| self.view(i))
            .collect()
    }
    pub fn inherited_names(
        &self,
        id: usize,
        donors: Option<&BTreeSet<String>>,
        normalized_only: bool,
    ) -> BTreeMap<String, Vec<FunctionKey>> {
        let mut names = BTreeMap::<String, Vec<FunctionKey>>::new();
        if id >= self.functions.len() || self.membership.is_empty() {
            return names;
        }
        // Strong family membership carries names; consumers may request equal-code evidence only.
        for &i in &self.families[self.membership[id]].members {
            let f = &self.functions[i];
            if (normalized_only
                && (f.signature != self.functions[id].signature
                    || f.architecture != self.functions[id].architecture))
                || !donors.is_none_or(|d| d.contains(&f.key.firmware))
            {
                continue;
            }
            for n in &f.names {
                names.entry(n.clone()).or_default().push(f.key.clone());
            }
        }
        names
    }
    pub fn normalized_tokens(&self, id: usize) -> Vec<&str> {
        self.functions
            .get(id)
            .into_iter()
            .flat_map(|f| f.tokens.iter())
            .map(|&t| self.tokens[t as usize].as_str())
            .collect()
    }
    pub fn calls(&self, id: usize) -> Vec<(u64, Option<FunctionView>)> {
        self.functions
            .get(id)
            .into_iter()
            .flat_map(|f| &f.calls)
            .map(|&(_, a)| (a, self.resolve(id, a).map(|i| self.view(i))))
            .collect()
    }
    pub fn data_references(&self, id: usize) -> Vec<Reference> {
        self.functions
            .get(id)
            .map(|f| f.references.clone())
            .unwrap_or_default()
    }
    pub fn references_to(
        &self,
        firmware: &str,
        unit: Option<&str>,
        address: u64,
    ) -> Vec<(FunctionView, Reference)> {
        self.functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.key.firmware == firmware && unit.is_none_or(|u| f.key.unit == u))
            .flat_map(|(i, f)| {
                f.references
                    .iter()
                    .filter(move |r| r.target == address)
                    .map(move |r| (self.view(i), r.clone()))
            })
            .collect()
    }
    pub fn search(
        &self,
        firmware: &str,
        unit: Option<&str>,
        pattern: &[u8],
        mask: Option<&[u8]>,
    ) -> Result<Vec<FunctionKey>> {
        if pattern.is_empty() || mask.is_some_and(|m| m.len() != pattern.len()) {
            return Err(Error::Invalid(
                "empty pattern or mask length mismatch".into(),
            ));
        }
        let mut hits = Vec::new();
        for u in &self.units {
            if u.input.firmware != firmware || !unit.is_none_or(|x| u.input.unit == x) {
                continue;
            }
            for r in &u.regions {
                for (i, w) in r.bytes.windows(pattern.len()).enumerate() {
                    if w.iter()
                        .zip(pattern)
                        .enumerate()
                        .all(|(j, (&a, &b))| (a ^ b) & mask.map_or(255, |m| m[j]) == 0)
                    {
                        hits.push(FunctionKey {
                            firmware: firmware.into(),
                            unit: u.input.unit.clone(),
                            address: r.address + i as u64,
                        });
                    }
                }
            }
        }
        Ok(hits)
    }
    pub fn annotate(&mut self, family: usize, annotation: Annotation) -> Result<()> {
        let f = self
            .families
            .get_mut(family)
            .ok_or_else(|| Error::Invalid("unknown family".into()))?;
        let representative = f.members[0];
        f.annotations.push(annotation.clone());
        self.functions[representative]
            .annotations
            .push(annotation.clone());
        let key = &self.functions[representative].key;
        if let Some(b) = self
            .units
            .iter_mut()
            .find(|u| u.input.firmware == key.firmware && u.input.unit == key.unit)
            .and_then(|u| {
                u.input
                    .functions
                    .iter_mut()
                    .find(|b| b.address == key.address)
            })
        {
            b.annotations.push(annotation);
        }
        Ok(())
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        use std::io::Write;
        file.write_all(b"FMCORP01")?;
        bincode::serialize_into(&mut file, self)?;
        file.flush()?;
        Ok(())
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        use std::io::Read;
        let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != b"FMCORP01" {
            return Err(Error::Invalid("unsupported index format".into()));
        }
        let mut value: Self = bincode::deserialize_from(file)?;
        value.reindex();
        Ok(value)
    }
}
