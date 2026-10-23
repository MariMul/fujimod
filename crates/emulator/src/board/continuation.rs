//! Passive host continuation boundaries. None of this state is modeled or saved.
use super::{Board, State, bus::Bus, factory::MemoryMap};
use fujimod_emulator_cpu::{Access, Cpu, Hook, HookId, HookRange};
use std::{
    cell::RefCell,
    collections::BTreeSet,
    ffi::c_void,
    rc::{Rc, Weak},
};

#[derive(Default, serde::Serialize)]
pub struct ReadSet {
    pub stores: Vec<(u64, u64)>,
    pub devices: BTreeSet<usize>,
}
#[derive(Default)]
pub(super) struct Stops {
    pub stores: Vec<(u64, u64)>,
    pub devices: BTreeSet<usize>,
    pub fired: bool,
}
impl Stops {
    pub fn store(&mut self, address: u64, bytes: u64) {
        self.fired |= self
            .stores
            .iter()
            .any(|&(lo, hi)| address < hi && lo < address.saturating_add(bytes));
    }
}
pub(super) fn fired(stops: &RefCell<Stops>) -> bool {
    stops.borrow().fired
}
pub(super) fn finish_instruction(cpu: &dyn Cpu) -> Result<(), String> {
    let progress = cpu
        .instruction_progress()
        .ok_or("Continuation requires exact instruction progress")?;
    cpu.tighten_instruction_limit(progress)
}
pub(super) struct StoreWatch {
    contexts: Vec<Box<Context>>,
    hooks: Vec<(usize, Vec<HookId>)>,
}
struct Context {
    state: Weak<RefCell<State>>,
    cpu: Weak<dyn Cpu>,
    maps: Vec<MemoryMap>,
    core: usize,
}
unsafe fn written(_: Access, address: u64, size: i32, _: i64, data: *mut c_void) {
    let context = unsafe { &*data.cast::<Context>() };
    let (Some(state), Some(cpu)) = (context.state.upgrade(), context.cpu.upgrade()) else {
        return;
    };
    let result = (|| {
        let size = u64::try_from(size).map_err(|_| "Invalid continuation store width")?;
        let state = state.borrow();
        let physical = state
            .decoders
            .observed_physical(context.core, address, size)
            .or_else(|| {
                context
                    .maps
                    .iter()
                    .find(|m| {
                        m.address <= address
                            && address.saturating_add(size) <= m.address + m.size as u64
                    })
                    .map(|m| m.physical + address - m.address)
            });
        if let Some(physical) = physical {
            state.continuation.borrow_mut().store(physical, size);
            if fired(&state.continuation) {
                finish_instruction(&*cpu)?;
            }
        }
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        state.borrow_mut().fail(error);
        let _ = cpu.stop();
    }
}
/// Record a half-open span, joining it to the previous one when they overlap
/// or touch: a predicate that reads memory in order keeps a single span.
fn add_span(spans: &mut Vec<(u64, u64)>, (lo, hi): (u64, u64)) {
    if let Some(last) = spans.last_mut()
        && lo <= last.1
        && last.0 <= hi
    {
        last.0 = last.0.min(lo);
        last.1 = last.1.max(hi);
    } else {
        spans.push((lo, hi));
    }
}
fn union(mut spans: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    spans.sort_unstable();
    let mut out: Vec<(u64, u64)> = vec![];
    for (lo, hi) in spans {
        if let Some(last) = out.last_mut()
            && lo <= last.1
        {
            last.1 = last.1.max(hi);
        } else {
            out.push((lo, hi));
        }
    }
    out
}
impl Board {
    pub fn read_set_begin(&self) -> Result<(), String> {
        let mut reads = self.read_set.borrow_mut();
        if reads.is_some() {
            return Err("A predicate read set is already active".into());
        }
        *reads = Some(ReadSet::default());
        Ok(())
    }
    pub fn read_set_end(&self) -> Result<ReadSet, String> {
        let mut reads = self
            .read_set
            .borrow_mut()
            .take()
            .ok_or("No predicate read set is active")?;
        reads.stores = union(reads.stores);
        Ok(reads)
    }
    pub(super) fn track_all_devices(&self, bus: &Bus) {
        if let Some(reads) = self.read_set.borrow_mut().as_mut() {
            reads.devices.extend(0..bus.len());
        }
    }
    pub(super) fn track_device_read(&self, id: usize) {
        if let Some(reads) = self.read_set.borrow_mut().as_mut() {
            reads.devices.insert(id);
        }
    }
    pub(super) fn track_device_kind(&self, kind: &str, bus: &Bus) {
        if let Some(reads) = self.read_set.borrow_mut().as_mut() {
            reads.devices.extend(bus.device_ids(kind));
        }
    }
    pub(super) fn track_physical_read(&self, address: u64, bytes: usize) -> Result<(), String> {
        if let Some(reads) = self.read_set.borrow_mut().as_mut() {
            for span in self.physical_memory.physical_ranges(address, bytes)? {
                add_span(&mut reads.stores, span);
            }
        }
        Ok(())
    }
    pub(super) fn track_cpu_read(
        &self,
        core: usize,
        address: u64,
        bytes: usize,
    ) -> Result<(), String> {
        if self.read_set.borrow().is_some() {
            let physical = self.cpu(core)?.translate(address, Access::Read)?;
            self.track_physical_read(physical, bytes)?;
        }
        Ok(())
    }
    pub fn continuation_begin(
        &mut self,
        stores: Vec<(u64, u64)>,
        devices: BTreeSet<usize>,
        maps: Vec<MemoryMap>,
    ) -> Result<(), String> {
        if self.continuation.is_some() {
            return Err("A continuation watch is already active".into());
        }
        if stores.len() > 4096
            || stores.iter().any(|&(lo, hi)| lo >= hi)
            || devices
                .iter()
                .any(|&id| id >= self.state.borrow().bus.len())
        {
            return Err("Invalid continuation watches".into());
        }
        if self.cpus.iter().any(|cpu| {
            !cpu.features().instruction_progress || !cpu.features().tighten_instruction_limit
        }) {
            return Err("Continuation watches require exact instruction limits".into());
        }
        let stores = union(stores);
        for &(lo, hi) in &stores {
            let mut cursor = lo;
            while cursor < hi {
                let (base, backing) = self
                    .physical_memory
                    .regions()
                    .iter()
                    .find(|(base, backing)| {
                        *base <= cursor && cursor < *base + backing.len() as u64
                    })
                    .ok_or("Continuation store range has no complete physical backing")?;
                cursor = hi.min(*base + backing.len() as u64);
            }
        }
        let stops = self.state.borrow().continuation.clone();
        *stops.borrow_mut() = Stops {
            stores: stores.clone(),
            devices,
            fired: false,
        };
        self.physical_memory
            .continuation
            .replace(Some(Rc::downgrade(&stops)));
        self.state.borrow_mut().bus.continuation(Some(stops));
        self.continuation = Some(Box::new(StoreWatch {
            contexts: vec![],
            hooks: vec![],
        }));
        let result = (|| {
            for (core, cpu) in self.cpus.iter().enumerate() {
                let mut views = maps
                    .iter()
                    .filter(|m| m.core == core)
                    .cloned()
                    .collect::<Vec<_>>();
                views.extend(self.state.borrow().decoders.observation_maps(core));
                let mut spans = vec![];
                for map in &views {
                    for &(lo, hi) in &stores {
                        let begin = lo.max(map.physical);
                        let end = hi.min(map.physical + map.size as u64);
                        if begin < end {
                            spans.push((
                                (map.address + begin - map.physical).saturating_sub(15),
                                map.address + end - map.physical,
                            ));
                        }
                    }
                }
                let watch = self.continuation.as_mut().unwrap();
                watch.contexts.push(Box::new(Context {
                    state: Rc::downgrade(&self.state),
                    cpu: Rc::downgrade(cpu),
                    maps: views,
                    core,
                }));
                let data = (&**watch.contexts.last().unwrap() as *const Context)
                    .cast_mut()
                    .cast();
                let hooks = union(spans)
                    .into_iter()
                    .map(|(begin, end)| Hook::CompletedStore {
                        callback: written,
                        data,
                        range: HookRange::Inclusive {
                            begin,
                            end: end - 1,
                        },
                    })
                    .collect::<Vec<_>>();
                if !hooks.is_empty() {
                    watch
                        .hooks
                        .push((core, unsafe { cpu.install_hooks(&hooks) }?));
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.continuation_end()?;
        }
        result
    }
    pub fn continuation_end(&mut self) -> Result<bool, String> {
        if let Some(watch) = self.continuation.as_mut() {
            for (core, hooks) in &watch.hooks {
                self.cpus[*core].remove_hooks(hooks)?;
            }
        }
        self.continuation = None;
        self.physical_memory.continuation.replace(None);
        let mut state = self.state.borrow_mut();
        state.bus.continuation(None);
        let fired = state.continuation.borrow().fired;
        *state.continuation.borrow_mut() = Stops::default();
        Ok(fired)
    }
}

#[cfg(test)]
#[path = "continuation_tests.rs"]
mod tests;
