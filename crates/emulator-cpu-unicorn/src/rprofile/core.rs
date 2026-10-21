//! Counted R-profile execution over the pinned Unicorn accelerator.
//! The owner supplies physical backing and interrupt pins; devices never see
//! translation metadata or the processor's architectural state.
use super::{
    accounting::{Accounting, Span},
    memory::Memory,
};
use crate::Core as UnicornCore;
use fujimod_emulator_cpu::memory::Backing;
use fujimod_emulator_cpu::*;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::{ffi::c_void, ptr::NonNull};

mod backend;
mod exclusive;
mod loops;
mod mapping;
mod pmu;
mod run;
mod snapshot;
mod tcm;
use mapping::Mapping;

#[cfg(test)]
mod tcm_tests;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Options {
    pub cp14_identity: u32,
    pub tcm: super::tcm::Options,
}
pub struct Wiring {
    pub memory: Memory,
}

struct Observer {
    hook: Hook,
    native: Option<HookId>,
}
pub(super) struct Execution {
    // Drop the engine before its callback data, accounting ring and backing.
    pub(super) cpu: Rc<UnicornCore>,
    memory: Memory,
    monitor: fujimod_emulator_cpu::exclusive::Owner,
    identity_input: Cell<u32>,
    clock: Cell<Option<Clock>>,
    clock_overshoot: Cell<u64>,
    policy: Cell<ExecutionPolicy>,
    pmu: Cell<pmu::Counter>,
    tcm: [super::tcm::Bank; 2],
    tcm_pending: Cell<Option<(usize, u32)>>,
    observed_backings: RefCell<Vec<BackingObserver>>,
    maps: RefCell<Vec<Mapping>>,
    id: Cell<usize>,
    record: NonNull<BlockAccounting>,
    ring: UnsafeCell<Box<[u64; 64]>>,
    owned: RefCell<Vec<Backing>>,
    mmio_hooks: RefCell<BTreeMap<u64, (usize, HookId)>>,
    fast_reads: RefCell<Vec<FastMmioRead>>,
    loop_reads: RefCell<Vec<(u64, FastMmioRead)>>,
    loops: loops::Loops,
    count: Cell<u64>,
    cap: Cell<u64>,
    synced: Cell<i64>,
    settled_head: Cell<u64>,
    thumb: Cell<bool>,
    restarted: Cell<Option<u64>>,
    active: Cell<bool>,
    user_stop: Cell<bool>,
    irq: Cell<bool>,
    hint: Cell<Option<Hint>>,
    exception: Cell<Option<arm::Exception>>,
    failure: RefCell<Option<String>>,
    observers: RefCell<BTreeMap<usize, Observer>>,
    next_observer: Cell<usize>,
    instruction_observers: Cell<usize>,
    block_observers: Cell<usize>,
    at_block: Cell<bool>,
    at_instruction: Cell<bool>,
}

pub struct Core {
    execution: Rc<Execution>,
}
impl Core {
    /// A raw engine has no other owner: translation and accounting hooks are
    /// private to this backend for its entire lifetime.
    pub fn new(cpu: UnicornCore, options: Options, wiring: Wiring) -> Result<Self, String> {
        let features = cpu.features();
        if cpu.model() != Model::CortexR5F
            || !features.translation_filter
            || !features.block_accounting
            || !features.watched_stores
            || !features.store_pc_control
        {
            return Err("counted R-profile execution requires the R5 translation, accounting and watched-store accelerator".into());
        }
        if !cpu.memory_regions()?.is_empty() {
            return Err("R-profile backing must be mapped through its execution owner".into());
        }
        let record = cpu
            .accounting()
            .ok_or("missing R-profile accounting record")?;
        let tcm = super::tcm::banks(options.tcm)?;
        let monitor = wiring
            .memory
            .monitors
            .lock()
            .map_err(|_| "exclusive monitors poisoned")?
            .register(4)?;
        let e = Rc::new(Execution {
            cpu: Rc::new(cpu),
            memory: wiring.memory,
            monitor,
            identity_input: Cell::new(options.cp14_identity),
            clock: Cell::new(None),
            clock_overshoot: Cell::new(0),
            policy: Cell::new(ExecutionPolicy::Exact),
            pmu: Cell::new(pmu::Counter::default()),
            tcm,
            tcm_pending: Cell::new(None),
            observed_backings: RefCell::new(Vec::new()),
            maps: RefCell::new(Vec::new()),
            id: Cell::new(usize::MAX),
            record,
            ring: UnsafeCell::new(Box::new([0; 64])),
            owned: RefCell::new(Vec::new()),
            mmio_hooks: RefCell::new(BTreeMap::new()),
            fast_reads: RefCell::new(Vec::new()),
            loop_reads: RefCell::new(Vec::new()),
            loops: loops::Loops::default(),
            count: Cell::new(0),
            cap: Cell::new(0),
            synced: Cell::new(0),
            settled_head: Cell::new(0),
            thumb: Cell::new(false),
            restarted: Cell::new(None),
            active: Cell::new(false),
            user_stop: Cell::new(false),
            irq: Cell::new(false),
            hint: Cell::new(None),
            exception: Cell::new(None),
            failure: RefCell::new(None),
            observers: RefCell::new(BTreeMap::new()),
            next_observer: Cell::new(0),
            instruction_observers: Cell::new(0),
            block_observers: Cell::new(0),
            at_block: Cell::new(false),
            at_instruction: Cell::new(false),
        });
        e.id.set(e.memory.register(Rc::downgrade(&e)));
        unsafe {
            let r = &mut *record.as_ptr();
            r.ring = (*e.ring.get()).as_mut_ptr();
            r.mask = 63;
            r.enabled = 1;
            r.head = 0;
            r.budget = 0;
        }
        let data = Rc::as_ptr(&e).cast_mut().cast();
        // Core owns all callback lifetimes. No callback can retain a mutable
        // state borrow while calling into a device or a user observer.
        unsafe {
            e.cpu.translation_hooks(Some(TranslationHooks {
                classify,
                restart,
                data,
            }))?;
            e.cpu.install_hook(Hook::Block {
                callback: block,
                data,
                range: HookRange::All,
            })?;
            e.cpu.install_hook(Hook::Instruction {
                callback: instruction,
                data,
                range: HookRange::All,
            })?;
            e.cpu.install_hook(Hook::Exception {
                callback: exception,
                data,
            })?;
            let stores = e.cpu.install_hook(Hook::Memory {
                callback: store,
                data,
                read: false,
                write: true,
                range: HookRange::All,
            })?;
            e.cpu.hook_watched(stores, true)?;
        }
        e.cpu.precise_store_pc(true)?;
        for bank in 0..2 {
            e.declare_tcm(bank)?;
            e.map_tcm(bank)?;
        }
        Ok(Self { execution: e })
    }
}
impl Execution {
    pub(super) fn set_policy(&self, policy: ExecutionPolicy) {
        self.policy.set(policy);
        let enabled = policy == ExecutionPolicy::Interactive;
        for read in self.fast_reads.borrow().iter() {
            read.set_enabled(enabled);
        }
    }
    pub(super) fn accounting_mode(&self) -> u64 {
        if self.thumb.get() {
            0
        } else {
            match self.policy.get() {
                ExecutionPolicy::Exact => 1,
                ExecutionPolicy::Interactive => 2,
            }
        }
    }
    fn accounting(&self) -> Accounting {
        unsafe { Accounting::from_raw(self.record) }
    }
    fn scalar(&self, register: Register) -> Result<u32, String> {
        let mut bytes = [0; 4];
        if self.cpu.read_register(register, &mut bytes)? != 4 {
            return Err("R-profile register is not 32 bits".into());
        }
        Ok(u32::from_le_bytes(bytes))
    }
    fn set(&self, register: Register, value: u32) -> Result<(), String> {
        self.cpu.write_register(register, &value.to_le_bytes())
    }
    fn tail(&self, pc: u64, entered: bool) -> u64 {
        if self.policy.get() == ExecutionPolicy::Interactive {
            return 0;
        }
        self.accounting()
            .newest_since(self.settled_head.get())
            .map_or(0, |span| span.tail(pc, entered))
    }
    fn settle(&self, tail: u64, give_back: bool) -> Result<(), String> {
        let mut a = self.accounting();
        let p = a.settle(self.synced.get(), tail, give_back)?;
        self.count.set(
            self.count
                .get()
                .checked_add(p.instructions)
                .ok_or("R-profile instruction count overflow")?,
        );
        self.synced.set(p.budget);
        self.settled_head.set(a.counters().1);
        Ok(())
    }
    fn reset_budget(&self) {
        self.accounting().reset();
        self.synced.set(0);
    }
    fn progress(&self) -> Option<u64> {
        if !self.active.get() || self.at_instruction.get() {
            return Some(self.count.get());
        }
        let a = self.accounting();
        let tail = self.tail(self.scalar(arm::PC).ok()? as u64, !self.at_block.get());
        let delta = self
            .synced
            .get()
            .checked_sub(a.counters().0)?
            .checked_sub(tail as i64)?;
        self.count.get().checked_add(u64::try_from(delta).ok()?)
    }
    pub(super) fn prepare_flush(&self) -> Result<(), String> {
        // The write already happened (for example DMA inside MMIO). Finish
        // this access once, then stop at the following block for invalidation.
        if !self.at_instruction.get() {
            self.settle(self.tail(self.scalar(arm::PC)? as u64, true), true)?;
        }
        self.reset_budget();
        Ok(())
    }
    fn tighten(&self, total: u64) -> Result<(), String> {
        if !self.active.get() {
            return Err("no active R-profile run to tighten".into());
        }
        if total >= self.cap.get() {
            return Ok(());
        }
        if !self.at_instruction.get() {
            self.settle(
                self.tail(self.scalar(arm::PC)? as u64, !self.at_block.get()),
                true,
            )?;
        }
        self.cap.set(total.max(self.count.get()));
        self.reset_budget();
        Ok(())
    }
    fn fail(&self, message: String) {
        if self.failure.borrow().is_none() {
            *self.failure.borrow_mut() = Some(message);
        }
        let _ = self.cpu.stop();
    }
    fn device_hook(&self, address: u64, bytes: usize) -> Result<(), String> {
        // The accelerator identifies a device boundary through memory hooks.
        // MMIO callbacks alone neither end a block nor synchronize its PC.
        let hook = unsafe {
            self.cpu.install_hook(Hook::Memory {
                callback: device_boundary,
                data: std::ptr::null_mut(),
                read: true,
                write: true,
                range: HookRange::Inclusive {
                    begin: address,
                    end: address + bytes as u64 - 1,
                },
            })?
        };
        self.mmio_hooks.borrow_mut().insert(address, (bytes, hook));
        Ok(())
    }
    fn guarded(&self, action: impl FnOnce() -> Result<(), String>) {
        match catch_unwind(AssertUnwindSafe(action)) {
            Ok(Ok(())) => {}
            Ok(Err(message)) => self.fail(message),
            Err(_) => {
                self.fail("R-profile callback panicked; execution stopped at the C boundary".into())
            }
        }
    }
    fn classify(&self, address: u64, word: u32) -> Result<Translation, String> {
        let watch = self.memory.note_code(self.id.get(), address, 4)?;
        Ok(Translation {
            instruction: if self.instruction_observers.get() != 0
                && self.observers.borrow().values().any(|observer| {
                    matches!(observer.hook, Hook::Instruction { range, .. } if includes(range, address))
                })
                || special(word) || self.loop_candidate(address, word)
            {
                InstructionDisposition::ObserveAndEndBlock
            } else {
                InstructionDisposition::Execute
            },
            observe_block: watch || self.block_observers.get() != 0,
        })
    }
    fn block(&self, block: Block) -> Result<(), String> {
        self.settle(0, true)?;
        self.at_block.set(true);
        let result = self.enter_block(block);
        self.at_block.set(false);
        result
    }
    fn enter_block(&self, block: Block) -> Result<(), String> {
        let thumb = block
            .thumb
            .ok_or("R-profile block has no instruction-set metadata")?;
        self.thumb.set(thumb);
        unsafe { (*self.record.as_ptr()).enabled = self.accounting_mode() };
        let restarted = self.restarted.get() == Some(block.address);
        for observer in self.observers.borrow().values() {
            if let Hook::Block {
                callback,
                data,
                range,
            } = observer.hook
                && includes(range, block.address)
            {
                unsafe {
                    callback(block, data);
                }
            }
        }
        if self.user_stop.get() {
            return Ok(());
        }
        if self.memory.flush_pending() && !restarted {
            self.reset_budget();
            return self.cpu.stop();
        }
        let mut room = self.cap.get().saturating_sub(self.count.get());
        if restarted {
            room = room.min(1);
        }
        if room == 0 {
            self.reset_budget();
            return self.cpu.stop();
        }
        if thumb {
            self.memory
                .note_code(self.id.get(), block.address, block.bytes as usize)?;
        } else {
            if block.bytes == 0 || !block.bytes.is_multiple_of(4) {
                return Err("invalid R-profile ARM block size".into());
            }
            let n = u64::from(block.bytes / 4);
            if n > room && self.policy.get() == ExecutionPolicy::Exact {
                self.reset_budget();
                return self.cpu.retry_block(room as u32);
            }
            let span = Span {
                address: block.address,
                instructions: n,
            };
            self.synced.set(match self.policy.get() {
                ExecutionPolicy::Exact => self.accounting().admit(span, room)?,
                ExecutionPolicy::Interactive => self.accounting().admit_coarse(span, room)?,
            });
        }
        if restarted {
            self.restarted.set(None);
        }
        self.memory.apply_watches()?;
        Ok(())
    }
    fn instruction(&self, address: u64, size: u32) -> Result<(), String> {
        if !self.thumb.get() {
            self.settle(self.tail(address, false), false)?;
        }
        let coarse_arm = !self.thumb.get() && self.policy.get() == ExecutionPolicy::Interactive;
        if !coarse_arm {
            if self.count.get() >= self.cap.get() {
                self.reset_budget();
                return self.cpu.stop();
            }
            self.count.set(
                self.count
                    .get()
                    .checked_add(1)
                    .ok_or("R-profile instruction count overflow")?,
            );
        }
        self.at_instruction.set(true);
        let result = self.execute_observed(address, size);
        self.at_instruction.set(false);
        result
    }
    fn execute_observed(&self, address: u64, size: u32) -> Result<(), String> {
        for observer in self.observers.borrow().values() {
            if let Hook::Instruction {
                callback,
                data,
                range,
            } = observer.hook
                && includes(range, address)
            {
                unsafe {
                    callback(address, size, data);
                }
            }
        }
        if self.user_stop.get() {
            return Ok(());
        }
        if self.scalar(arm::PC)? as u64 != address {
            return Ok(());
        }
        let mut bytes = [0; 4];
        self.cpu.read_memory(address, &mut bytes[..size as usize])?;
        let word = u32::from_le_bytes(bytes);
        let cpsr = self.scalar(arm::CPSR)?;
        let thumb = self.thumb.get();
        if !thumb && self.execute_loop(address, word)? {
            return Ok(());
        }
        let condition = if thumb {
            let it = ((cpsr >> 25) & 3) | ((cpsr >> 8) & 0xfc);
            if it == 0 { 14 } else { it >> 4 }
        } else {
            word >> 28
        };
        if condition < 14 && !arm::condition_passes(condition, cpsr) {
            return Ok(());
        }
        if self.irq.get()
            && super::predict_irq_mask(
                word,
                size,
                thumb,
                |r| self.scalar(r),
                |address, bytes| self.cpu.read_memory(address as u64, bytes),
            )? == Some(0)
        {
            self.tighten(self.count.get())?;
        }
        let hint = if thumb && size == 2 {
            match word {
                0xbf20 => Some(Hint::WaitForEvent),
                0xbf30 => Some(Hint::WaitForInterrupt),
                0xbf40 => Some(Hint::SendEvent),
                _ => None,
            }
        } else if thumb {
            match word.rotate_left(16) {
                0xf3af8002 => Some(Hint::WaitForEvent),
                0xf3af8003 => Some(Hint::WaitForInterrupt),
                0xf3af8004 => Some(Hint::SendEvent),
                _ => None,
            }
        } else if word >> 28 != 15 {
            match word & 0x0fff_ffff {
                0x0320_f002 => Some(Hint::WaitForEvent),
                0x0320_f003 => Some(Hint::WaitForInterrupt),
                0x0320_f004 => Some(Hint::SendEvent),
                _ => None,
            }
        } else {
            None
        };
        if let Some(hint) = hint {
            self.advance(address, size, cpsr)?;
            self.hint.set(Some(hint));
            self.reset_budget();
            return self.cpu.stop();
        }
        if thumb {
            if size == 4 {
                if self.exclusive_instruction(word, address, size, cpsr)? {
                    return Ok(());
                }
                // T32 coprocessor transfers number fields in halfword order.
                self.coprocessor_instruction(word.rotate_left(16), address, size, cpsr)?;
            }
        } else {
            if self.exclusive_instruction(word, address, size, cpsr)? {
                return Ok(());
            }
            self.coprocessor_instruction(word, address, size, cpsr)?;
        }
        Ok(())
    }
    fn advance(&self, address: u64, size: u32, cpsr: u32) -> Result<(), String> {
        if self.thumb.get() {
            self.set(arm::CPSR, arm::thumb::advance_it(cpsr))?;
        }
        self.set(
            arm::PC,
            (address as u32).wrapping_add(size) | u32::from(self.thumb.get()),
        )
    }
    fn coprocessor_instruction(
        &self,
        word: u32,
        address: u64,
        size: u32,
        cpsr: u32,
    ) -> Result<(), String> {
        if word & 0x0fe0_0f00 == 0x0c40_0f00 && word & 15 == 9 {
            return Err("R-profile 64-bit PMU execution is not implemented".into());
        }
        if word & 0x0f00_0010 != 0x0e00_0010 {
            return Ok(());
        }
        let cp = (word >> 8) & 15;
        let (opc1, crn, crm, opc2) = (
            (word >> 21) & 7,
            (word >> 16) & 15,
            word & 15,
            (word >> 5) & 7,
        );
        if !self.thumb.get()
            && word >> 28 == 15
            && (cp == 14
                || cp == 15 && opc1 == 0 && crn == 9 && (crm == 1 || (12..=14).contains(&crm)))
        {
            return Err("unconditional R-profile coprocessor substitution is unmodeled".into());
        }
        if cp == 15 && opc1 == 0 && crn == 9 && crm == 1 {
            if opc2 > 1 {
                return Err("unmodeled R-profile TCM selector".into());
            }
            return self.tcm_instruction(opc2 as usize, word, address, size, cpsr);
        }
        if pmu::selected(arm::Coprocessor {
            cp,
            opc1,
            crn,
            crm,
            opc2,
            secure: 0,
            wide: false,
        }) {
            let target = ((word >> 12) & 15) as usize;
            if target == 15 {
                return Err("R-profile PMU transfer using PC is unmodeled".into());
            }
            let read = word & (1 << 20) != 0;
            let value = if read {
                None
            } else {
                Some(self.scalar(arm::CORE[target])?)
            };
            let result = self.pmu_access(crm, opc2, value)?;
            if read {
                self.set(arm::CORE[target], result)?;
            }
            return self.advance(address, size, cpsr);
        }
        if cp == 14 {
            let target = ((word >> 12) & 15) as usize;
            if word & (1 << 20) == 0 || (opc1, crn, crm, opc2) != (0, 2, 0, 0) || target == 15 {
                return Err("unmodeled R-profile CP14 access".into());
            }
            self.set(arm::CORE[target], self.identity_input.get())?;
            self.advance(address, size, cpsr)?;
        }
        Ok(())
    }
}
fn includes(range: HookRange, address: u64) -> bool {
    match range {
        HookRange::All => true,
        HookRange::Inclusive { begin, end } => (begin..=end).contains(&address),
    }
}
fn atomic(word: u32) -> bool {
    word & 0x0f90_0fff == 0x0190_0f9f || word & 0x0f90_0ff0 == 0x0180_0f90 || word == 0xf57f_f01f
}
fn thumb_atomic(word: u32) -> bool {
    let first = word as u16;
    let second = (word >> 16) as u16;
    match first & 0xfff0 {
        0xe840 => true,
        0xe850 => second & 0x0f00 == 0x0f00,
        0xe8c0 => matches!(second & 0x0ff0, 0x0f40 | 0x0f50) || second & 0x00f0 == 0x70,
        0xe8d0 => matches!(second & 0x0fff, 0x0f4f | 0x0f5f) || second & 0x00ff == 0x7f,
        _ => (first, second) == (0xf3bf, 0x8f2f),
    }
}
fn special(word: u32) -> bool {
    super::arm_irq_mask_source(word).is_some()
        || atomic(word)
        || matches!(word & 0x0fff_ffff, 0x0320_f002..=0x0320_f004)
        || coprocessor_boundary(word)
        || word & 0x0fe0_0f0f == 0x0c40_0f09
}
fn coprocessor_boundary(word: u32) -> bool {
    if word & 0x0f00_0010 != 0x0e00_0010 {
        return false;
    }
    if word >> 28 == 15 || (word >> 8) & 15 == 14 {
        return true;
    }
    let register = arm::Coprocessor {
        cp: (word >> 8) & 15,
        opc1: (word >> 21) & 7,
        crn: (word >> 16) & 15,
        crm: word & 15,
        opc2: (word >> 5) & 7,
        secure: 0,
        wide: false,
    };
    pmu::selected(register)
        || (register.cp == 15 && register.opc1 == 0 && register.crn == 9 && register.crm == 1)
}
unsafe fn context<'a>(data: *mut c_void) -> &'a Execution {
    unsafe { &*data.cast::<Execution>() }
}
unsafe fn classify(data: *mut c_void, address: u64, word: u32) -> Translation {
    let e = unsafe { context(data) };
    match catch_unwind(AssertUnwindSafe(|| e.classify(address, word))) {
        Ok(Ok(result)) => result,
        result => {
            e.fail(match result {
                Ok(Err(message)) => message,
                _ => "R-profile translation callback panicked".into(),
            });
            Translation {
                instruction: InstructionDisposition::ObserveAndEndBlock,
                observe_block: true,
            }
        }
    }
}
unsafe fn restart(data: *mut c_void, pc: u64) {
    let e = unsafe { context(data) };
    e.guarded(|| {
        e.settle(e.tail(pc, false), true)?;
        e.reset_budget();
        e.restarted.set(Some(pc));
        Ok(())
    });
}
unsafe fn block(block: Block, data: *mut c_void) {
    let e = unsafe { context(data) };
    e.guarded(|| e.block(block));
}
unsafe fn instruction(address: u64, size: u32, data: *mut c_void) {
    let e = unsafe { context(data) };
    e.guarded(|| e.instruction(address, size));
}
unsafe fn store(_: Access, address: u64, size: i32, _: i64, data: *mut c_void) {
    let e = unsafe { context(data) };
    e.guarded(|| {
        if size <= 0 {
            return Err("invalid R-profile store width".into());
        }
        if e.memory.guest_store(e.id.get(), address, size as usize)? {
            e.cpu.request_restart()?;
        }
        Ok(())
    });
}
unsafe fn exception(exception: arm::Exception, data: *mut c_void) {
    let e = unsafe { context(data) };
    e.guarded(|| {
        if matches!(
            exception,
            arm::Exception::PrefetchAbort | arm::Exception::DataAbort
        ) {
            e.settle(
                e.tail(
                    e.scalar(arm::PC)? as u64,
                    matches!(exception, arm::Exception::DataAbort),
                ),
                true,
            )?;
            e.exception.set(Some(exception));
            return e.cpu.stop();
        }
        Err(format!(
            "unmodeled R-profile architectural exception {exception:?}"
        ))
    });
}
unsafe fn device_boundary(_: Access, _: u64, _: i32, _: i64, _: *mut c_void) {}
