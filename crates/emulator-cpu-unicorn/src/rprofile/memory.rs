//! Shared backing identities, translated bytes and alias-coherent code writes.
//! Only the CPU backend uses this execution mechanism. It never supplies a
//! device with program inspection or private RAM predicates.
use super::core::Execution;
use fujimod_emulator_cpu::Cpu;
use fujimod_emulator_cpu::exclusive;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

#[derive(Clone, Copy)]
struct View {
    core: usize,
    address: u64,
    pointer: usize,
    bytes: usize,
    physical: Option<u64>,
}
struct Chunks<'a> {
    views: &'a [View],
    core: usize,
    cursor: u64,
    end: u64,
    require_backing: bool,
}
impl<'a> Chunks<'a> {
    fn new(
        views: &'a [View],
        core: usize,
        address: u64,
        bytes: usize,
        require_backing: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            views,
            core,
            cursor: address,
            end: address
                .checked_add(bytes as u64)
                .ok_or("CPU memory range overflow")?,
            require_backing,
        })
    }
}
impl Iterator for Chunks<'_> {
    type Item = Result<(usize, usize), String>;
    fn next(&mut self) -> Option<Self::Item> {
        while self.cursor < self.end {
            if let Some(v) = self.views.iter().find(|v| {
                v.core == self.core
                    && self.cursor >= v.address
                    && self.cursor < v.address + v.bytes as u64
            }) {
                let bytes = (self.end.min(v.address + v.bytes as u64) - self.cursor) as usize;
                let pointer = v.pointer + (self.cursor - v.address) as usize;
                self.cursor += bytes as u64;
                return Some(Ok((pointer, bytes)));
            }
            if self.require_backing {
                let address = self.cursor;
                self.cursor = self.end;
                return Some(Err(format!(
                    "CPU memory has no owned backing at {address:#x}"
                )));
            }
            self.cursor = self
                .views
                .iter()
                .filter(|v| v.core == self.core && v.address > self.cursor)
                .map(|v| v.address)
                .min()
                .unwrap_or(self.end)
                .min(self.end);
        }
        None
    }
}
struct Page {
    bits: Box<[u64; 64]>,
    invalid: bool,
}
#[derive(Default)]
struct State {
    cores: Vec<Weak<Execution>>,
    views: Vec<View>,
    bindings: Vec<View>,
    pages: HashMap<usize, Page>,
    watches: Vec<(usize, u64)>,
    invalidations: Vec<(usize, u64)>,
    dirty_pages: Vec<usize>,
}
#[derive(Clone, Default)]
pub struct Memory {
    state: Rc<RefCell<State>>,
    active: Rc<Cell<Option<usize>>>,
    pub(super) monitors: exclusive::Shared,
}
impl Memory {
    pub(super) fn require_stopped(&self) -> Result<(), String> {
        if self.active.get().is_some() {
            return Err("R-profile memory mapping requires all processors to be stopped".into());
        }
        Ok(())
    }
    pub(super) fn register(&self, core: Weak<Execution>) -> usize {
        let mut s = self.state.borrow_mut();
        let id = s.cores.len();
        s.cores.push(core);
        id
    }
    pub(super) fn start(&self, core: usize) -> Result<(), String> {
        if self.active.get().is_some() {
            return Err("nested R-profile execution is refused".into());
        }
        self.active.set(Some(core));
        Ok(())
    }
    pub(super) fn end(&self) {
        self.active.set(None);
    }
    pub(super) fn validate_mapping(
        &self,
        pointer: usize,
        bytes: usize,
        physical: Option<u64>,
    ) -> Result<(), String> {
        let Some(physical) = physical else {
            return Ok(());
        };
        for v in &self.state.borrow().bindings {
            let Some(other) = v.physical else {
                continue;
            };
            let begin = pointer.max(v.pointer);
            let end = (pointer + bytes).min(v.pointer + v.bytes);
            if begin < end
                && physical + (begin - pointer) as u64 != other + (begin - v.pointer) as u64
            {
                return Err("shared backing has conflicting physical RAM identities".into());
            }
            let begin = physical.max(other);
            let end = (physical + bytes as u64).min(other + v.bytes as u64);
            if begin < end
                && pointer + (begin - physical) as usize != v.pointer + (begin - other) as usize
            {
                return Err("physical RAM identity has different shared backing".into());
            }
        }
        Ok(())
    }
    pub(super) fn declare(
        &self,
        core: usize,
        address: u64,
        pointer: usize,
        bytes: usize,
        physical: Option<u64>,
    ) {
        let mut s = self.state.borrow_mut();
        s.bindings.push(View {
            core,
            address,
            pointer,
            bytes,
            physical,
        });
        let capacity = s.bindings.len() + s.cores.len() * 4;
        let len = s.views.len();
        s.views.reserve(capacity.saturating_sub(len));
    }
    pub(super) fn undeclare(&self, core: usize, address: u64, bytes: usize) {
        Self::remove_range(&mut self.state.borrow_mut().bindings, core, address, bytes);
    }
    pub(super) fn forget(&self, core: usize) {
        let mut s = self.state.borrow_mut();
        s.views.retain(|v| v.core != core);
        s.bindings.retain(|v| v.core != core);
        s.cores[core] = Weak::new();
    }
    pub(super) fn map(
        &self,
        core: usize,
        address: u64,
        pointer: usize,
        bytes: usize,
        physical: Option<u64>,
    ) {
        let mut s = self.state.borrow_mut();
        let view = View {
            core,
            address,
            pointer,
            bytes,
            physical,
        };
        // A newly mapped alias must watch code already translated elsewhere.
        let State { pages, watches, .. } = &mut *s;
        for &page in pages.keys() {
            Self::aliases(&[view], page, watches);
        }
        s.views.push(view);
        // Translation-time reservation bounds all later code-write queues.
        let capacity = s
            .pages
            .len()
            .saturating_mul(s.views.len())
            .saturating_mul(2);
        let len = s.invalidations.len();
        s.invalidations.reserve(capacity.saturating_sub(len));
        let capacity = s.views.len() * 4;
        let len = s.watches.len();
        s.watches.reserve(capacity.saturating_sub(len));
    }
    pub(super) fn unmap(&self, core: usize, address: u64, bytes: usize) {
        Self::remove_range(&mut self.state.borrow_mut().views, core, address, bytes);
    }
    fn remove_range(views: &mut Vec<View>, core: usize, address: u64, bytes: usize) {
        let end = address + bytes as u64;
        let mut i = 0;
        while i < views.len() {
            let v = views[i];
            if v.core != core || v.address >= end || address >= v.address + v.bytes as u64 {
                i += 1;
                continue;
            }
            let left = v.address < address;
            if left {
                views[i].bytes = (address - v.address) as usize;
            }
            if v.address + v.bytes as u64 > end {
                let right = View {
                    address: end,
                    pointer: v.pointer + (end - v.address) as usize,
                    bytes: (v.address + v.bytes as u64 - end) as usize,
                    core,
                    physical: v.physical.map(|p| p + end - v.address),
                };
                if left {
                    views.push(right);
                } else {
                    views[i] = right;
                }
            } else if !left {
                views.swap_remove(i);
                continue;
            }
            i += 1;
        }
    }
    fn aliases(views: &[View], page: usize, out: &mut Vec<(usize, u64)>) {
        for v in views {
            let begin = page.max(v.pointer);
            let end = (page + 4096).min(v.pointer + v.bytes);
            if begin >= end {
                continue;
            }
            let first = (v.address + (begin - v.pointer) as u64) >> 12;
            let last = (v.address + (end - v.pointer - 1) as u64) >> 12;
            for p in first..=last {
                out.push((v.core, p << 12));
            }
        }
    }
    fn canonical_pointer(views: &[View], pointer: usize, bytes: usize) -> Option<u64> {
        views.iter().find_map(|v| {
            if pointer >= v.pointer && pointer.checked_add(bytes)? <= v.pointer + v.bytes {
                v.physical.map(|p| p + (pointer - v.pointer) as u64)
            } else {
                None
            }
        })
    }
    pub(super) fn canonical(&self, core: usize, address: u64, bytes: usize) -> Result<u64, String> {
        let s = self.state.borrow();
        let (pointer, _) = Chunks::new(&s.views, core, address, bytes, true)?
            .next()
            .ok_or("empty R-profile exclusive range")??;
        Self::canonical_pointer(&s.bindings, pointer, bytes)
            .ok_or("R-profile exclusive is outside checked physical RAM views".into())
    }
    pub(super) fn watch_exclusive(
        &self,
        core: usize,
        address: u64,
        bytes: usize,
    ) -> Result<(), String> {
        {
            let mut s = self.state.borrow_mut();
            let (pointer, _) = Chunks::new(&s.views, core, address, bytes, true)?
                .next()
                .ok_or("empty R-profile exclusive range")??;
            let State { views, watches, .. } = &mut *s;
            for page in ((pointer & !4095)..=((pointer + bytes - 1) & !4095)).step_by(4096) {
                Self::aliases(views, page, watches);
            }
        }
        self.apply_watches()
    }
    pub(super) fn note_code(
        &self,
        core: usize,
        address: u64,
        bytes: usize,
    ) -> Result<bool, String> {
        let mut s = self.state.borrow_mut();
        let State {
            views,
            pages,
            watches,
            invalidations,
            dirty_pages,
            ..
        } = &mut *s;
        let old = watches.len();
        for chunk in Chunks::new(views, core, address, bytes, true)? {
            let (pointer, bytes) = chunk?;
            for byte in pointer..pointer + bytes {
                let base = byte & !4095;
                if let std::collections::hash_map::Entry::Vacant(entry) = pages.entry(base) {
                    entry.insert(Page {
                        bits: Box::new([0; 64]),
                        invalid: false,
                    });
                    Self::aliases(views, base, watches);
                    let capacity = pages.len() * views.len() * 2;
                    invalidations.reserve(capacity.saturating_sub(invalidations.len()));
                    dirty_pages.reserve(pages.len().saturating_sub(dirty_pages.len()));
                }
                let bits = &mut pages.get_mut(&base).unwrap().bits;
                let offset = byte & 4095;
                bits[offset / 64] |= 1 << (offset % 64);
            }
        }
        Ok(watches.len() != old)
    }
    pub(super) fn apply_watches(&self) -> Result<(), String> {
        loop {
            let next = {
                let mut s = self.state.borrow_mut();
                s.watches
                    .pop()
                    .map(|(id, page)| (s.cores[id].upgrade(), page))
            };
            let Some((core, page)) = next else {
                return Ok(());
            };
            if let Some(core) = core {
                core.cpu.watch_stores(page, 4096)?;
            }
        }
    }
    pub(super) fn written(&self, core: usize, address: u64, bytes: usize) -> Result<bool, String> {
        self.record_write(core, address, bytes, true)
    }
    pub(super) fn guest_store(
        &self,
        core: usize,
        address: u64,
        bytes: usize,
    ) -> Result<bool, String> {
        // Unicorn's MMIO slow path calls every covering store hook, including
        // watched hooks. Non-RAM accesses have no translated backing here;
        // the mapped device or the processor's fault path owns their effects.
        self.record_write(core, address, bytes, false)
    }
    fn record_write(
        &self,
        core: usize,
        address: u64,
        bytes: usize,
        require_backing: bool,
    ) -> Result<bool, String> {
        if bytes == 0 {
            return Ok(false);
        }
        let mut s = self.state.borrow_mut();
        let State {
            views,
            bindings,
            pages,
            invalidations,
            dirty_pages,
            ..
        } = &mut *s;
        let mut changed = false;
        for chunk in Chunks::new(views, core, address, bytes, require_backing)? {
            let (pointer, count) = chunk?;
            if let Some(physical) = Self::canonical_pointer(bindings, pointer, count) {
                self.monitors
                    .lock()
                    .map_err(|_| "exclusive monitors poisoned")?
                    .written(Some((physical, count)))?;
            }
            changed |=
                Self::record_backing(views, pages, invalidations, dirty_pages, pointer, count);
        }
        Ok(changed)
    }
    pub(super) fn backing_written(
        &self,
        physical: u64,
        pointer: usize,
        bytes: usize,
    ) -> Result<bool, String> {
        if bytes == 0 {
            return Ok(false);
        }
        self.validate_mapping(pointer, bytes, Some(physical))?;
        self.monitors
            .lock()
            .map_err(|_| "exclusive monitors poisoned")?
            .written(Some((physical, bytes)))?;
        let mut s = self.state.borrow_mut();
        let State {
            views,
            pages,
            invalidations,
            dirty_pages,
            ..
        } = &mut *s;
        Ok(Self::record_backing(
            views,
            pages,
            invalidations,
            dirty_pages,
            pointer,
            bytes,
        ))
    }
    fn record_backing(
        views: &[View],
        pages: &mut HashMap<usize, Page>,
        invalidations: &mut Vec<(usize, u64)>,
        dirty_pages: &mut Vec<usize>,
        pointer: usize,
        bytes: usize,
    ) -> bool {
        let mut changed = false;
        for base in ((pointer & !4095)..=((pointer + bytes - 1) & !4095)).step_by(4096) {
            let Some(page) = pages.get_mut(&base) else {
                continue;
            };
            if page.invalid {
                continue;
            }
            let touched = (pointer.max(base)..(pointer + bytes).min(base + 4096)).any(|p| {
                let offset = p & 4095;
                page.bits[offset / 64] & (1 << (offset % 64)) != 0
            });
            if !touched {
                continue;
            }
            page.invalid = true;
            page.bits.fill(0);
            changed = true;
            dirty_pages.push(base);
            Self::aliases(views, base, invalidations);
        }
        changed
    }
    pub(super) fn flush_pending(&self) -> bool {
        !self.state.borrow().invalidations.is_empty()
    }
    pub(super) fn prepare_external_write(&self) -> Result<(), String> {
        let core = self
            .active
            .get()
            .and_then(|id| self.state.borrow().cores[id].upgrade());
        if let Some(core) = core {
            core.prepare_flush()?;
        } else {
            self.flush()?;
        }
        Ok(())
    }
    pub(super) fn flush(&self) -> Result<(), String> {
        if self.active.get().is_some() {
            return Err("code invalidation requires stopped processors".into());
        }
        loop {
            let next = {
                let mut s = self.state.borrow_mut();
                s.invalidations
                    .pop()
                    .map(|(id, page)| (s.cores[id].upgrade(), page))
            };
            let Some((core, page)) = next else {
                break;
            };
            if let Some(core) = core {
                core.cpu.invalidate_code(Some((page, page + 4096)))?;
            }
        }
        let mut s = self.state.borrow_mut();
        while let Some(page) = s.dirty_pages.pop() {
            s.pages.get_mut(&page).unwrap().invalid = false;
        }
        Ok(())
    }
}
