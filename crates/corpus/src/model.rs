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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    /// (alias start, exclusive end, mapped start), explicit one-hop mappings.
    #[serde(default)]
    pub address_aliases: Vec<(u64, u64, u64)>,
    pub functions: Vec<Boundary>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub input: UnitInput,
    pub regions: Vec<Region>,
}
impl Unit {
    pub fn canonical_address(&self, address: u64) -> u64 {
        self.input
            .address_aliases
            .iter()
            .find_map(|&(lo, hi, target)| {
                (lo <= address && address < hi).then(|| target + (address - lo))
            })
            .unwrap_or(address)
    }
    pub fn read(&self, address: u64, size: usize) -> Option<&[u8]> {
        if self.input.address_aliases.iter().any(|&(lo, hi, _)| {
            lo <= address
                && address < hi
                && address.checked_add(size as u64).is_none_or(|end| end > hi)
        }) {
            return None;
        }
        let address = self.canonical_address(address);
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
pub struct NameDonor {
    #[serde(flatten)]
    pub key: FunctionKey,
    pub grade: Grade,
    pub sources: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name_donors: Vec<NameDonor>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSymbol {
    pub firmware: String,
    pub unit: String,
    pub address: u64,
    pub length: usize,
    pub name: String,
    pub kind: String,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    #[serde(skip)]
    pub(crate) inspection: std::sync::Arc<crate::quality::InspectionCache>,
    pub(crate) units: Vec<Unit>,
    pub(crate) data_symbols: Vec<DataSymbol>,
    pub(crate) functions: Vec<Function>,
    pub(crate) tokens: Vec<String>,
    pub(crate) families: Vec<Family>,
    pub(crate) membership: Vec<usize>,
    pub(crate) weak: Vec<(usize, usize)>,
    #[serde(skip)]
    pub(crate) weak_families: HashMap<usize, BTreeSet<usize>>,
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
        self.inspection =
            std::sync::Arc::new(crate::quality::InspectionCache::new(&self.functions));
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
        self.reindex_weak();
        self.targets.clear();
        for (i, f) in self.functions.iter().enumerate() {
            self.targets
                .entry((f.key.firmware.clone(), f.key.address))
                .or_default()
                .push(i);
        }
    }
    pub(crate) fn reindex_weak(&mut self) {
        self.weak_families.clear();
        for &(a, b) in &self.weak {
            if let (Some(&x), Some(&y)) = (self.membership.get(a), self.membership.get(b))
                && x != y
            {
                self.weak_families.entry(x).or_default().insert(y);
                self.weak_families.entry(y).or_default().insert(x);
            }
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
        let mut alias_ranges = unit.input.address_aliases.clone();
        alias_ranges.sort_unstable();
        if alias_ranges.windows(2).any(|w| w[0].1 > w[1].0)
            || alias_ranges.iter().any(|&(lo, hi, target)| {
                hi <= lo
                    || lo == target
                    || target.checked_add(hi - lo).is_none()
                    || !unit.regions.iter().any(|r| {
                        r.address <= target
                            && target + (hi - lo) <= r.address + r.bytes.len() as u64
                    })
            })
        {
            return Err(Error::Invalid(
                "invalid or overlapping address aliases".into(),
            ));
        }
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
        self.data_symbols.extend(other.data_symbols);
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
            data_symbols: self
                .data_symbols
                .iter()
                .filter(|s| firmwares.contains(&s.firmware))
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
            name_donors: vec![],
        }
    }
    pub fn find(
        &self,
        name: Option<&str>,
        address: Option<u64>,
        firmware: Option<&str>,
        unit: Option<&str>,
    ) -> Vec<FunctionView> {
        let candidates =
            name.map(|name| {
                let mut found = BTreeSet::new();
                for (donor, f) in self
                    .functions
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.names.iter().any(|n| n == name))
                {
                    found.insert(donor);
                    let Some(&family) = self.membership.get(donor) else {
                        continue;
                    };
                    found.extend(&self.families[family].members);
                    if let Some(adjacent) = self.weak_families.get(&family) {
                        for &other in adjacent {
                            found.extend(
                                self.families[other].members.iter().copied().filter(|&id| {
                                    self.functions[id].key.firmware != f.key.firmware
                                }),
                            );
                        }
                    }
                }
                found
            });
        self.functions
            .iter()
            .enumerate()
            .filter(|(id, f)| {
                firmware.is_none_or(|x| f.key.firmware == x)
                    && unit.is_none_or(|x| f.key.unit == x)
                    && address.is_none_or(|x| f.key.address == x)
                    && candidates.as_ref().is_none_or(|c| c.contains(id))
            })
            .map(|(i, _)| {
                let mut view = self.view(i);
                if let Some(name) = name {
                    view.name_donors = self
                        .inherited_names(i, None, false)
                        .remove(name)
                        .unwrap_or_default();
                }
                view
            })
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
    /// Analysis names carry their donor and grade. A weak edge exposes names
    /// from the adjacent family; weak edges never merge families or chain.
    pub fn inherited_names(
        &self,
        id: usize,
        donors: Option<&BTreeSet<String>>,
        normalized_only: bool,
    ) -> BTreeMap<String, Vec<NameDonor>> {
        let mut names = BTreeMap::<String, Vec<NameDonor>>::new();
        let Some(&family) = self.membership.get(id) else {
            return names;
        };
        let mut candidates: Vec<_> = self.families[family]
            .members
            .iter()
            .map(|&i| {
                (
                    i,
                    if i == id {
                        Grade::Identical
                    } else {
                        self.grade(i, id)
                    },
                )
            })
            .collect();
        if !normalized_only && let Some(adjacent) = self.weak_families.get(&family) {
            for &other in adjacent {
                candidates.extend(self.families[other].members.iter().filter_map(|&i| {
                    (self.functions[i].key.firmware != self.functions[id].key.firmware)
                        .then_some((i, Grade::Weak))
                }));
            }
        }
        candidates.sort_by_key(|&(i, _)| i);
        candidates.dedup_by_key(|entry| entry.0);
        for (i, grade) in candidates {
            let f = &self.functions[i];
            if (normalized_only && !grade.normalized())
                || !donors.is_none_or(|d| d.contains(&f.key.firmware))
            {
                continue;
            }
            for name in &f.names {
                names.entry(name.clone()).or_default().push(NameDonor {
                    key: f.key.clone(),
                    grade,
                    sources: f
                        .annotations
                        .iter()
                        .filter(|a| a.kind == "name-source" && a.text == *name)
                        .map(|a| a.source.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                });
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
    pub fn add_symbol(&mut self, symbol: DataSymbol) -> Result<()> {
        if symbol.name.is_empty()
            || symbol.length == 0
            || self
                .read(
                    &symbol.firmware,
                    &symbol.unit,
                    symbol.address,
                    symbol.length,
                )
                .is_none()
        {
            return Err(Error::Invalid("invalid or unmapped data symbol".into()));
        }
        if !self.data_symbols.contains(&symbol) {
            self.data_symbols.push(symbol);
        }
        Ok(())
    }
    pub fn symbols(
        &self,
        name: Option<&str>,
        firmware: Option<&str>,
        unit: Option<&str>,
    ) -> Vec<&DataSymbol> {
        self.data_symbols
            .iter()
            .filter(|s| {
                name.is_none_or(|n| n == s.name)
                    && firmware.is_none_or(|f| f == s.firmware)
                    && unit.is_none_or(|u| u == s.unit)
            })
            .collect()
    }
    pub fn read(&self, firmware: &str, unit: &str, address: u64, size: usize) -> Option<Vec<u8>> {
        self.units
            .iter()
            .find(|u| u.input.firmware == firmware && u.input.unit == unit)?
            .read(address, size)
            .map(|b| b.to_vec())
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
        file.write_all(b"FMCORP03")?;
        bincode::serialize_into(&mut file, self)?;
        file.flush()?;
        Ok(())
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        use std::io::Read;
        let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != b"FMCORP03" {
            return Err(Error::Invalid("unsupported index format".into()));
        }
        let mut value: Self = bincode::deserialize_from(file)?;
        value.reindex();
        Ok(value)
    }
}
