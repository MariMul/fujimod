//! Bounded aggregate RAM-byte preservation over CPU stores and bus writes.
use super::{
    Board, State,
    factory::MemoryMap,
    observation::{tcm_overlap, union},
    scalar,
};
use fujimod_emulator_cpu::{Cpu, Hook, HookId, HookRange, Model, Register};
use serde::Serialize;
use std::{
    cell::{RefCell, Weak},
    ffi::c_void,
    rc::Rc,
};

type Span = (u64, u64);

const MAX_CPU_STORE: u64 = 8;
/// A wider store starting this far before a protected byte can still overlap.
const HOOK_REACH: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreservationBytes {
    bytes: [u8; MAX_CPU_STORE as usize],
    len: u8,
}

impl PreservationBytes {
    fn new(bytes: &[u8]) -> Self {
        let mut out = [0; MAX_CPU_STORE as usize];
        out[..bytes.len()].copy_from_slice(bytes);
        Self {
            bytes: out,
            len: bytes.len() as u8,
        }
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl Serialize for PreservationBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = [0u8; 16];
        for (index, &byte) in self.as_slice().iter().enumerate() {
            text[index * 2] = HEX[usize::from(byte >> 4)];
            text[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        let len = usize::from(self.len) * 2;
        let text = std::str::from_utf8(&text[..len])
            .map_err(|_| serde::ser::Error::custom("hex digits"))?;
        serializer.serialize_str(text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreservationBusSource {
    HostWrite,
    ModelBusWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreservationFault {
    UnresolvedMapping,
    UnsupportedAccess,
    UnreadablePreimage,
    CounterOverflow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PreservationViolation {
    CpuChange {
        core: usize,
        pc: u64,
        instructions: u64,
        address: u64,
        physical: u64,
        size: u32,
        before: PreservationBytes,
        after: PreservationBytes,
    },
    UnverifiableWrite {
        source: PreservationBusSource,
        address: u64,
        physical: u64,
        size: u32,
    },
    IntegrityFault {
        reason: PreservationFault,
    },
}

/// Integrity faults override earlier effects, mark counters incomplete and remain stable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PreservationState {
    pub enabled: bool,
    pub ranges: Vec<(u64, u64)>,
    pub identity_cpu_writes: u64,
    pub changed_cpu_writes: u64,
    pub host_writes: u64,
    pub model_bus_writes: u64,
    pub violation: Option<PreservationViolation>,
}

impl PreservationState {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ranges: Vec::new(),
            identity_cpu_writes: 0,
            changed_cpu_writes: 0,
            host_writes: 0,
            model_bus_writes: 0,
            violation: None,
        }
    }
}

pub(super) struct PreservationRecords {
    ranges: Vec<Span>,
    merged: Vec<Span>,
    identity_cpu_writes: u64,
    changed_cpu_writes: u64,
    host_writes: u64,
    model_bus_writes: u64,
    violation: Option<PreservationViolation>,
}

impl PreservationRecords {
    pub(super) fn state(&self) -> PreservationState {
        PreservationState {
            enabled: true,
            ranges: self.ranges.clone(),
            identity_cpu_writes: self.identity_cpu_writes,
            changed_cpu_writes: self.changed_cpu_writes,
            host_writes: self.host_writes,
            model_bus_writes: self.model_bus_writes,
            violation: self.violation.clone(),
        }
    }
    pub(super) fn overlaps(&self, physical: u64, bytes: u64) -> bool {
        if bytes == 0 {
            return false;
        }
        self.merged
            .iter()
            .any(|&(lo, hi)| physical < hi && lo < physical.saturating_add(bytes))
    }
    fn fault(&mut self, reason: PreservationFault) {
        if matches!(
            self.violation,
            Some(PreservationViolation::IntegrityFault { .. })
        ) {
            return;
        }
        self.violation = Some(PreservationViolation::IntegrityFault { reason });
    }
    fn cpu_store(&mut self, changed: Option<PreservationViolation>) {
        if let Some(violation) = changed {
            let overflow = match self.changed_cpu_writes.checked_add(1) {
                Some(next) => {
                    self.changed_cpu_writes = next;
                    false
                }
                None => {
                    self.changed_cpu_writes = u64::MAX;
                    true
                }
            };
            if self.violation.is_none() {
                self.violation = Some(violation);
            }
            if overflow {
                self.fault(PreservationFault::CounterOverflow);
            }
        } else {
            match self.identity_cpu_writes.checked_add(1) {
                Some(next) => self.identity_cpu_writes = next,
                None => {
                    self.identity_cpu_writes = u64::MAX;
                    self.fault(PreservationFault::CounterOverflow);
                }
            }
        }
    }
    pub(super) fn bus_write(
        &mut self,
        source: PreservationBusSource,
        physical: u64,
        address: u64,
        len: usize,
    ) {
        if !self.overlaps(physical, len as u64) {
            return;
        }
        let counter = match source {
            PreservationBusSource::HostWrite => &mut self.host_writes,
            PreservationBusSource::ModelBusWrite => &mut self.model_bus_writes,
        };
        let overflow = match counter.checked_add(1) {
            Some(next) => {
                *counter = next;
                false
            }
            None => {
                *counter = u64::MAX;
                true
            }
        };
        match u32::try_from(len) {
            Ok(size) => {
                if self.violation.is_none() {
                    self.violation = Some(PreservationViolation::UnverifiableWrite {
                        source,
                        address,
                        physical,
                        size,
                    });
                }
            }
            Err(_) => self.fault(PreservationFault::UnsupportedAccess),
        }
        if overflow {
            self.fault(PreservationFault::CounterOverflow);
        }
    }
    pub(super) fn unresolved(&mut self) {
        self.fault(PreservationFault::UnresolvedMapping);
    }
}

pub struct Preservation {
    memory: Weak<super::memory::Memory>,
    records: Rc<RefCell<PreservationRecords>>,
    contexts: Vec<Box<PreservationContext>>,
    hooks: Vec<(Weak<dyn Cpu>, HookId)>,
    broken: bool,
    ended: bool,
}

impl Preservation {
    pub fn state(&self) -> PreservationState {
        if self.ended {
            PreservationState::disabled()
        } else {
            self.records.borrow().state()
        }
    }
    pub fn end(&mut self) -> Result<(), String> {
        if self.broken {
            return Err("preservation cleanup previously failed".into());
        }
        if self.ended {
            return Ok(());
        }
        let mut failure = None;
        for (cpu, hook) in std::mem::take(&mut self.hooks) {
            if let Some(cpu) = cpu.upgrade() {
                if let Err(error) = cpu.remove_hook(hook) {
                    failure.get_or_insert(error);
                }
            }
        }
        if failure.is_some() {
            self.broken = true;
            self.records
                .borrow_mut()
                .fault(PreservationFault::UnsupportedAccess);
            let contexts = std::mem::take(&mut self.contexts);
            std::mem::forget(contexts);
            return Err(failure.unwrap());
        }
        self.contexts.clear();
        if let Some(memory) = self.memory.upgrade() {
            memory.clear_preservation(&self.records);
        }
        self.ended = true;
        Ok(())
    }
}

impl Drop for Preservation {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

struct PreservationContext {
    state: Weak<RefCell<State>>,
    cpu: Weak<dyn Cpu>,
    records: Rc<RefCell<PreservationRecords>>,
    maps: Vec<MemoryMap>,
    spans: Vec<Span>,
    merged: Vec<Span>,
    core: usize,
}

impl PreservationContext {
    fn fault(&self, reason: PreservationFault) {
        let mut records = self.records.borrow_mut();
        records.fault(reason);
    }
    fn store(&self, address: u64, size: i32, value: i64) {
        let (Some(state), Some(cpu)) = (self.state.upgrade(), self.cpu.upgrade()) else {
            return;
        };
        let state = state.borrow();
        if size <= 0 {
            if self.overlaps_spans(address, 1) {
                self.fault(PreservationFault::UnsupportedAccess);
            }
            return;
        }
        let width = size as u64;
        if width > MAX_CPU_STORE {
            if self.overlaps_spans(address, width) {
                self.fault(PreservationFault::UnsupportedAccess);
            }
            return;
        }
        if !self.overlaps_spans(address, width) {
            return;
        }
        match tcm_overlap(&*cpu, address, width) {
            Ok(false) => {}
            Ok(true) => {
                self.fault(PreservationFault::UnsupportedAccess);
                return;
            }
            Err(_) => {
                self.fault(PreservationFault::UnsupportedAccess);
                return;
            }
        }
        let physical = state
            .decoders
            .observed_physical(self.core, address, width)
            .or_else(|| {
                self.maps.iter().find(|map| {
                    map.address <= address
                        && address.saturating_add(width) <= map.address + map.size as u64
                }).map(|map| map.physical + address - map.address)
            });
        let Some(physical) = physical else {
            self.fault(PreservationFault::UnresolvedMapping);
            return;
        };
        if !self
            .merged
            .iter()
            .any(|&(lo, hi)| physical < hi && lo < physical + width)
        {
            return;
        }
        let mut before = [0; MAX_CPU_STORE as usize];
        if cpu.read_memory(address, &mut before[..width as usize]).is_err() {
            self.fault(PreservationFault::UnreadablePreimage);
            return;
        }
        let byte_order = match scalar(&*cpu, Register::Pstate) {
            Ok(pstate) => pstate & (1 << 9) != 0,
            Err(_) => {
                self.fault(PreservationFault::UnsupportedAccess);
                return;
            }
        };
        let mask = if width == 8 {
            u64::MAX
        } else {
            (1u64 << (width * 8)) - 1
        };
        let stored = value as u64 & mask;
        let mut after = [0; MAX_CPU_STORE as usize];
        for index in 0..width as usize {
            let shift = if byte_order {
                (width as usize - 1 - index) * 8
            } else {
                index * 8
            };
            after[index] = (stored >> shift) as u8;
        }
        let mut changed = None;
        for &(lo, hi) in &self.merged {
            let start = lo.max(physical);
            let end = hi.min(physical + width);
            if start >= end {
                continue;
            }
            let offset = (start - physical) as usize;
            let len = (end - start) as usize;
            if before[offset..offset + len] == after[offset..offset + len] {
                continue;
            }
            let progress = cpu
                .instruction_progress()
                .unwrap_or(state.accounted[self.core]);
            let Some(extra) = progress.checked_sub(state.accounted[self.core]) else {
                self.fault(PreservationFault::CounterOverflow);
                return;
            };
            let Some(counted) = state
                .instructions
                .iter()
                .try_fold(0u64, |total, count| total.checked_add(*count))
            else {
                self.fault(PreservationFault::CounterOverflow);
                return;
            };
            let Some(instructions) = counted.checked_add(extra) else {
                self.fault(PreservationFault::CounterOverflow);
                return;
            };
            let pc = match scalar(&*cpu, Register::Pc) {
                Ok(pc) => pc,
                Err(_) => {
                    self.fault(PreservationFault::UnsupportedAccess);
                    return;
                }
            };
            changed = Some(PreservationViolation::CpuChange {
                core: self.core,
                pc,
                instructions,
                address: address + offset as u64,
                physical: start,
                size: len as u32,
                before: PreservationBytes::new(&before[offset..offset + len]),
                after: PreservationBytes::new(&after[offset..offset + len]),
            });
            break;
        }
        self.records.borrow_mut().cpu_store(changed);
    }
    fn overlaps_spans(&self, address: u64, width: u64) -> bool {
        self.spans
            .iter()
            .any(|&(lo, hi)| address < hi && lo < address.saturating_add(width))
    }
}

unsafe fn write(address: u64, size: i32, value: i64, data: *mut c_void) {
    let context = unsafe { &*data.cast::<PreservationContext>() };
    context.store(address, size, value);
}

impl Board {
    pub(super) fn preservation_active(&self) -> bool {
        self.physical_memory
            .preservation
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some()
    }
    pub(super) fn preservation_records(&self) -> Option<Rc<RefCell<PreservationRecords>>> {
        self.physical_memory
            .preservation
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
    }
    pub(super) fn record_host_cpu_write(&self, core: usize, address: u64, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let Some(records) = self.preservation_records() else {
            return;
        };
        let state = self.state.borrow();
        let physical = state
            .decoders
            .observed_physical(core, address, bytes as u64)
            .or_else(|| {
                self.mappings
                    .iter()
                    .find(|map| {
                        map.core == core
                            && map.address <= address
                            && address.saturating_add(bytes as u64) <= map.address + map.size as u64
                    })
                    .map(|map| map.physical + address - map.address)
            });
        match physical {
            Some(physical) => records
                .borrow_mut()
                .bus_write(PreservationBusSource::HostWrite, physical, address, bytes),
            None => records.borrow_mut().unresolved(),
        }
    }
    pub fn preservation_begin(&self, ranges: Vec<Span>) -> Result<Box<Preservation>, String> {
        if ranges.is_empty()
            || ranges.len() > 8
            || ranges
                .iter()
                .any(|&(lo, hi)| lo >= hi || hi > u32::MAX as u64)
            || self.cpus.iter().any(|cpu| cpu.model() != Model::CortexR5F)
        {
            return Err(
                "Preservation observation requires 1..8 bounded 32-bit ranges on R-profile processors"
                    .into(),
            );
        }
        self.state
            .try_borrow_mut()
            .map_err(|_| "Preservation observation requires a paused board")?;
        if self.preservation_active() {
            return Err("A preservation observation is already armed".into());
        }
        if self
            .physical_memory
            .observation
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some()
        {
            return Err("A memory watch conflicts with preservation observation".into());
        }
        let merged = union(ranges.clone());
        for &(lo, hi) in &merged {
            let mut cursor = lo;
            while cursor < hi {
                let (base, backing) = self
                    .physical_memory
                    .regions()
                    .iter()
                    .find(|(base, backing)| {
                        *base <= cursor && cursor < *base + backing.len() as u64
                    })
                    .ok_or("Preservation range has no complete physical RAM backing")?;
                cursor = hi.min(*base + backing.len() as u64);
            }
        }
        let mut views = self.mappings.clone();
        {
            let state = self.state.borrow();
            for core in 0..self.cpus.len() {
                views.extend(state.decoders.observation_maps(core));
            }
        }
        views.sort_by_key(|map| (map.core, map.address, map.physical, map.size));
        views.dedup();
        let mut spans: Vec<Vec<Span>> = vec![Vec::new(); self.cpus.len()];
        for map in &views {
            if map.core >= self.cpus.len() {
                continue;
            }
            for &(lo, hi) in &merged {
                let start = lo.max(map.physical);
                let end = hi.min(map.physical + map.size as u64);
                if start < end {
                    spans[map.core].push((
                        map.address + start - map.physical,
                        map.address + end - map.physical,
                    ));
                }
            }
        }
        for (core, spans) in spans.iter_mut().enumerate() {
            *spans = union(std::mem::take(spans));
            for &(lo, hi) in spans.iter() {
                if tcm_overlap(&*self.cpus[core], lo, hi - lo)
                    .map_err(|error| format!("Preservation TCM inspection: {error}"))?
                {
                    return Err(
                        "Preservation range intersects a CPU-private TCM overlay".into(),
                    );
                }
            }
        }
        let records = Rc::new(RefCell::new(PreservationRecords {
            ranges,
            merged,
            identity_cpu_writes: 0,
            changed_cpu_writes: 0,
            host_writes: 0,
            model_bus_writes: 0,
            violation: None,
        }));
        let mut preservation = Box::new(Preservation {
            memory: Rc::downgrade(&self.physical_memory),
            records: records.clone(),
            contexts: Vec::new(),
            hooks: Vec::new(),
            broken: false,
            ended: false,
        });
        for (core, spans) in spans.into_iter().enumerate() {
            if spans.is_empty() {
                continue;
            }
            let context_merged = records.borrow().merged.clone();
            preservation.contexts.push(Box::new(PreservationContext {
                state: Rc::downgrade(&self.state),
                cpu: Rc::downgrade(&self.cpus[core]),
                records: records.clone(),
                maps: self
                    .mappings
                    .iter()
                    .filter(|map| map.core == core)
                    .cloned()
                    .collect(),
                merged: context_merged,
                spans,
                core,
            }));
            let data = (&**preservation.contexts.last().unwrap() as *const PreservationContext)
                .cast_mut()
                .cast();
            for (begin, end) in union(
                preservation.contexts.last().unwrap().spans
                    .iter()
                    .map(|&(lo, hi)| (lo.saturating_sub(HOOK_REACH), hi))
                    .collect(),
            ) {
                match unsafe {
                    self.cpus[core].install_hook(Hook::Memory {
                        callback: write,
                        data,
                        read: false,
                        write: true,
                        range: HookRange::Inclusive {
                            begin,
                            end: end - 1,
                        },
                    })
                } {
                    Ok(id) => preservation
                        .hooks
                        .push((Rc::downgrade(&self.cpus[core]), id)),
                    Err(error) => {
                        self.preservation_end(&mut preservation)?;
                        return Err(error);
                    }
                }
            }
        }
        self.physical_memory
            .preservation
            .replace(Some(Rc::downgrade(&records)));
        Ok(preservation)
    }
    pub fn preservation_end(&self, preservation: &mut Preservation) -> Result<(), String> {
        preservation.end()
    }
}
