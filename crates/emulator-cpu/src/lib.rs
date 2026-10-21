//! CPU execution mechanisms. The runtime owns memory, devices and guest time;
//! a backend owns only its processors and translation/execution machinery.
//!
//! Backend handles and ABI register numbers never cross this boundary. Host
//! watchdogs only cancel execution; their elapsed time is not guest time.
use std::any::Any;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub mod arm;
pub mod arm64;
pub mod exclusive;
pub mod loops;
pub mod memory;
pub mod timeline;
#[cfg(target_os = "emscripten")]
mod wasm_backing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture {
    Arm32,
    Arm64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum Model {
    #[cfg_attr(feature = "serde", serde(rename = "cortex-r5f"))]
    CortexR5F,
    #[cfg_attr(feature = "serde", serde(rename = "cortex-a7"))]
    CortexA7,
    #[cfg_attr(feature = "serde", serde(rename = "cortex-m4"))]
    CortexM4,
    #[cfg_attr(feature = "serde", serde(rename = "armv8a"))]
    ArmV8A,
}
impl Model {
    pub fn architecture(self) -> Architecture {
        match self {
            Self::ArmV8A => Architecture::Arm64,
            _ => Architecture::Arm32,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identity(u64);
impl Identity {
    pub fn allocate() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookId {
    pub owner: Identity,
    pub key: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum Diagnostic {
    ExceptionSyndrome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum Register {
    Diagnostic(Diagnostic),
    Arm32(arm::Register),
    X(u8),
    Pc,
    Sp,
    Pstate,
    Fpcr,
    Fpsr,
    V(u8),
    Arm64System(arm::SystemRegister),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryRegion {
    pub begin: u64,
    /// Last mapped address, inclusive.
    pub end: u64,
    pub permissions: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Run {
    pub begin: u64,
    pub until: Option<u64>,
    /// Host cancellation only. Zero disables the watchdog.
    pub watchdog_us: u64,
    /// None runs until a requested stop or an architectural exit. Some(0)
    /// performs no execution and leaves the processor's PC unchanged.
    pub instructions: Option<usize>,
}
/// Instruction accounting behavior selected by the execution owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionPolicy {
    /// Stop at the requested instruction boundary and preserve exact progress.
    #[default]
    Exact,
    /// Permit backend-defined block-boundary overshoot for interactive runs.
    Interactive,
}
impl ExecutionPolicy {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Interactive => "interactive",
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "exact" => Ok(Self::Exact),
            "interactive" => Ok(Self::Interactive),
            _ => Err(format!("unknown CPU execution policy: {value}")),
        }
    }
}
/// Guest-clock anchor for architectural registers implemented by a backend.
/// Its phase and ratio are supplied by the execution owner, never host time.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub now: u64,
    pub phase: u64,
    pub ratio: crate::timeline::Ratio,
}
impl Clock {
    pub fn after(self, entries: u64) -> Result<Self, String> {
        let (ticks, phase) = self.ratio.advance_phase(self.phase, entries)?;
        let now = self
            .now
            .checked_add(u64::try_from(ticks).map_err(|_| "CPU guest clock span overflow")?)
            .ok_or("CPU guest clock exhausted")?;
        Ok(Self { now, phase, ..self })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Fetch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interrupt {
    Irq,
    Fiq,
}
/// Guest accesses to a mapped device. Offsets are relative to the mapping;
/// widths are in bytes. Host inspection never invokes these callbacks.
#[derive(Clone, Copy)]
pub struct Mmio {
    pub read: unsafe fn(u64, u32, *mut c_void) -> u64,
    pub write: unsafe fn(u64, u32, u64, *mut c_void),
    pub data: *mut c_void,
}
/// A device-published read-only register snapshot that can be read directly
/// by an execution backend. The device remains responsible for publishing
/// only values valid at the current guest-time boundary.
#[derive(Clone)]
pub struct FastMmioRead {
    pub register_offset: u64,
    pub value: Arc<AtomicU32>,
    enabled: Arc<AtomicBool>,
}
impl FastMmioRead {
    pub fn atomic_u32(register_offset: u64, value: Arc<AtomicU32>) -> Self {
        Self {
            register_offset,
            value,
            enabled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
    /// Return a snapshot only for the exact declared 32-bit register access.
    pub fn read(&self, offset: u64, size: u32) -> Option<u64> {
        if self.enabled.load(Ordering::Acquire) && offset == self.register_offset && size == 4 {
            Some(u64::from(self.value.load(Ordering::Acquire)))
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    ReadUnmapped,
    WriteUnmapped,
    FetchUnmapped,
    ReadProtected,
    WriteProtected,
    FetchProtected,
}
#[derive(Clone, Copy, Debug)]
pub struct Trap {
    pub syndrome: u64,
    pub virtual_address: u64,
    pub physical_address: u64,
}
/// Architectural hint completed by the backend. PC already names the next
/// instruction, and the hint contributes one entered instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    WaitForInterrupt,
    WaitForEvent,
    SendEvent,
    SendEventLocal,
}
#[derive(Clone, Copy, Debug)]
pub enum Exit {
    Stopped,
    Hint(Hint),
    Trap(Trap),
    Timer,
}
#[derive(Clone, Debug)]
pub enum ExecutionError {
    InvalidInstruction(String),
    Memory { fault: Fault, message: String },
    Backend(String),
}
impl ExecutionError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidInstruction(_) => "invalid-instruction",
            Self::Memory { fault, .. } => match fault {
                Fault::ReadUnmapped => "read-unmapped",
                Fault::WriteUnmapped => "write-unmapped",
                Fault::FetchUnmapped => "fetch-unmapped",
                Fault::ReadProtected => "read-protected",
                Fault::WriteProtected => "write-protected",
                Fault::FetchProtected => "fetch-protected",
            },
            Self::Backend(_) => "backend",
        }
    }
}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInstruction(message)
            | Self::Memory { message, .. }
            | Self::Backend(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ExecutionError {}
impl From<String> for ExecutionError {
    fn from(message: String) -> Self {
        Self::Backend(message)
    }
}
impl From<&str> for ExecutionError {
    fn from(message: &str) -> Self {
        Self::Backend(message.into())
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    /// Architectural counters consume an explicit guest-clock anchor per run.
    pub guest_clock: bool,
    pub instruction_limit: bool,
    /// RAM map/unmap/protect operations affect every CPU in the owning VM.
    /// MMIO callback installation remains per processor.
    pub shared_physical_maps: bool,
    /// Bounded runs expose their entered-instruction count through
    /// `instruction_progress`, including while stopped in a callback.
    pub instruction_progress: bool,
    /// Framework-owned architectural operations exit directly from run;
    /// the scheduler does not need an instruction hook to discover them.
    pub architectural_exits: bool,
    /// A callback can lower the current run's total entered-instruction cap.
    pub tighten_instruction_limit: bool,
    pub translation_filter: bool,
    pub block_accounting: bool,
    pub watched_stores: bool,
    pub backing_observation: bool,
    pub store_pc_control: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum HookRange {
    All,
    Inclusive { begin: u64, end: u64 },
}
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub address: u64,
    pub bytes: u32,
    pub thumb: Option<bool>,
}
#[derive(Clone, Copy)]
pub enum Hook {
    Instruction {
        callback: unsafe fn(u64, u32, *mut c_void),
        data: *mut c_void,
        range: HookRange,
    },
    Block {
        callback: unsafe fn(Block, *mut c_void),
        data: *mut c_void,
        range: HookRange,
    },
    Exception {
        callback: unsafe fn(arm::Exception, *mut c_void),
        data: *mut c_void,
    },
    Memory {
        callback: unsafe fn(Access, u64, i32, i64, *mut c_void),
        data: *mut c_void,
        read: bool,
        write: bool,
        range: HookRange,
    },
    Fault {
        callback: unsafe fn(Fault, u64, i32, i64, *mut c_void) -> bool,
        data: *mut c_void,
    },
}
#[derive(Clone, Copy, Debug)]
pub enum InstructionDisposition {
    Execute,
    Observe,
    ObserveAndEndBlock,
}
#[derive(Clone, Copy, Debug)]
pub struct Translation {
    pub instruction: InstructionDisposition,
    pub observe_block: bool,
}
#[derive(Clone, Copy)]
pub struct TranslationHooks {
    pub classify: unsafe fn(*mut c_void, u64, u32) -> Translation,
    pub restart: unsafe fn(*mut c_void, u64),
    pub data: *mut c_void,
}
/// Optional ARM32 block-budget accelerator. The instruction-span ring and its
/// lifetime belong to the runtime. Fields match the published accelerator ABI;
/// the runtime accesses this record only while execution is stopped/in a hook.
/// `enabled` selects exact ring accounting (1), coarse interactive accounting
/// without ring writes (2), or disabled accounting (0).
#[repr(C)]
pub struct BlockAccounting {
    pub budget: i64,
    pub head: u64,
    pub ring: *mut u64,
    pub mask: u64,
    pub enabled: u64,
}
/// An in-process CPU snapshot; architectural portable checkpoints are a
/// runtime/profile responsibility. A snapshot cannot restore a different CPU.
pub trait Snapshot: Any {
    fn owner(&self) -> Identity;
    fn as_any(&self) -> &dyn Any;
}
/// One thread-affine processor. This trait is deliberately not Send/Sync.
/// The owner serializes execution, inspection, maps and callback installation.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackingObserver {
    pub backing: NonNull<u8>,
    pub bytes: usize,
    pub bitmap: NonNull<u64>,
    pub page_shift: u32,
}

pub trait Cpu {
    fn identity(&self) -> Identity;
    /// Configure instruction-proven loops; statistics remain observational.
    fn set_loop_mode(&self, mode: loops::LoopMode) -> Result<(), String> {
        if mode == loops::LoopMode::Disabled { Ok(()) } else { Err("backend has no exact loop acceleration".into()) }
    }
    /// The execution owner guarantees no RAM/input changes before this run's
    /// instruction cap: stop at device, DMA, interrupt and runnable-peer events.
    /// The permission expires when the bounded run returns.
    fn set_loop_window(&self, _stable: bool) -> Result<(), String> { Ok(()) }
    /// Select how a bounded run handles its instruction target. Backends that
    /// do not support coarse interactive execution keep exact behavior only.
    fn set_execution_policy(&self, policy: ExecutionPolicy) -> Result<(), String> {
        match policy {
            ExecutionPolicy::Exact => Ok(()),
            ExecutionPolicy::Interactive => {
                Err("processor backend does not support interactive execution policy".into())
            }
        }
    }
    /// External mutable backing covered by a whole-board keyframe. Private
    /// memory retained by the processor snapshot is excluded. Unknown owners
    /// refuse, so a board cannot silently omit independently mapped RAM.
    fn checkpoint_backings(&self) -> Result<Vec<(NonNull<u8>, usize)>, String> {
        Err("processor cannot prove keyframe backing ownership".into())
    }
    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        Err("this processor has no complete state fingerprint".into())
    }
    /// Logical reservations shared by processors in one physical address space.
    /// Framework architectural adapters join this domain through checked RAM views.
    fn exclusive_domain(&self) -> Option<exclusive::Shared> {
        None
    }
    fn model(&self) -> Model;
    fn features(&self) -> Features;
    fn set_guest_clock(&self, _clock: Clock) -> Result<(), String> {
        Err("backend does not consume a guest-clock anchor".into())
    }
    fn read_register(&self, register: Register, out: &mut [u8]) -> Result<usize, String>;
    fn write_register(&self, register: Register, bytes: &[u8]) -> Result<(), String>;
    fn coprocessor(&self, register: arm::Coprocessor, value: Option<u64>) -> Result<u64, String>;
    fn read_memory(&self, address: u64, bytes: &mut [u8]) -> Result<(), String>;
    /// Translate using this CPU's architectural MMU and access permissions.
    /// Host inspection refuses device accesses, including page-table reads.
    fn translate(&self, address: u64, access: Access) -> Result<u64, String>;
    /// Read through the architectural MMU with the specified access rights.
    /// This is observational: a range touching MMIO returns an error.
    fn read_virtual(&self, address: u64, access: Access, bytes: &mut [u8]) -> Result<(), String>;
    fn write_memory(&self, address: u64, bytes: &[u8]) -> Result<(), String>;
    /// Notify a write through shared external backing (DMA or a peer backend).
    /// Required for backends with logical exclusive monitors. Prefer
    /// write_memory for host writes; it performs this notification itself.
    fn memory_written(&self, _address: u64, _bytes: usize) -> Result<(), String> {
        Err("backend does not support external memory-write notification".into())
    }
    /// A committed write to physical backing, bypassing CPU-private mappings.
    /// # Safety
    /// `backing` must address `bytes` live bytes of the declared physical RAM,
    /// with execution serialized as for map_backing. The range may be hidden
    /// by private processor RAM and need not be mapped at `physical` on this CPU.
    unsafe fn backing_written(
        &self,
        physical: u64,
        _backing: NonNull<u8>,
        bytes: usize,
    ) -> Result<(), String> {
        if self.features().shared_physical_maps {
            self.memory_written(physical, bytes)?;
        }
        self.invalidate_code(None)
    }
    fn memory_regions(&self) -> Result<Vec<MemoryRegion>, String>;
    /// # Safety
    /// The backing must stay allocated at this address, and guest writes must
    /// be permitted by its Rust ownership, until this mapping is removed or
    /// the backend address space is destroyed. An owning VM can extend that
    /// lifetime beyond its CPUs. Mappings must not race execution/inspection
    /// on any processor sharing the address space. External writers must use
    /// the backend's memory_written notification when it maintains logical
    /// exclusive monitors; stopping CPUs alone does not invalidate a monitor.
    unsafe fn map_backing(
        &self,
        address: u64,
        bytes: usize,
        permissions: u32,
        backing: NonNull<u8>,
    ) -> Result<(), String>;
    /// Map backing with its canonical physical RAM identity. Address aliases
    /// must preserve that identity for code writes and exclusive reservations.
    /// # Safety
    /// The lifetime, ownership and serialization requirements of map_backing
    /// apply. `physical` must identify these same backing bytes on every core.
    unsafe fn map_physical(
        &self,
        address: u64,
        physical: u64,
        bytes: usize,
        permissions: u32,
        backing: NonNull<u8>,
    ) -> Result<(), String> {
        if address != physical && self.exclusive_domain().is_some() {
            return Err("backend does not support canonical physical RAM aliases".into());
        }
        unsafe { self.map_backing(address, bytes, permissions, backing) }
    }
    /// Replace an existing external RAM window while retaining private CPU
    /// overlays. All processors sharing the memory must be stopped.
    /// # Safety
    /// The backing and identity requirements of map_physical apply. Both
    /// the old and new backing must remain alive through this transaction.
    unsafe fn replace_physical(
        &self,
        _address: u64,
        _physical: u64,
        _bytes: usize,
        _permissions: u32,
        _backing: NonNull<u8>,
    ) -> Result<(), String> {
        Err("backend does not support physical RAM window replacement".into())
    }
    fn map_zeroed(&self, address: u64, bytes: usize, permissions: u32) -> Result<(), String>;
    /// # Safety
    /// Callback data must remain valid until the whole mapping is removed or
    /// the CPU is destroyed. Callbacks must not unwind and must obey the
    /// owner's bounded reentrancy rules. Partial unmaps retain this lifetime.
    unsafe fn map_mmio(&self, address: u64, bytes: usize, callbacks: Mmio) -> Result<(), String>;
    /// Map MMIO with one pure read-only register snapshot. All other reads
    /// and every write continue through `callbacks`.
    /// # Safety
    /// The callback requirements of `map_mmio` apply. The snapshot must remain
    /// valid and be published at every guest-time boundary where its value
    /// changes.
    unsafe fn map_mmio_fast(
        &self,
        address: u64,
        bytes: usize,
        callbacks: Mmio,
        _fast_reads: Vec<FastMmioRead>,
    ) -> Result<(), String> {
        unsafe { self.map_mmio(address, bytes, callbacks) }
    }
    fn unmap(&self, address: u64, bytes: usize) -> Result<(), String>;
    fn protect(&self, address: u64, bytes: usize, permissions: u32) -> Result<(), String>;
    fn invalidate_code(&self, range: Option<(u64, u64)>) -> Result<(), String>;
    fn run(&self, request: Run) -> Result<Exit, ExecutionError>;
    /// Lower the current run's total entered-instruction limit, including
    /// entries already measured in that run. Never extends the original cap.
    fn tighten_instruction_limit(&self, _total_entries: u64) -> Result<(), String> {
        Err("backend cannot tighten an active instruction limit".into())
    }
    /// Instructions entered in the current/most recent run, when measured.
    /// None means this run has no backend count; the runtime may maintain its
    /// own count through execution observations. Reading never changes it.
    fn instruction_progress(&self) -> Option<u64> {
        None
    }
    fn stop(&self) -> Result<(), String>;
    /// Explicit isolated-call interception: omit the current instruction entry.
    /// Valid only inside an instruction observer, before any guest effects.
    fn stop_before_instruction(&self) -> Result<(), String> {
        Err("backend cannot intercept an instruction entry".into())
    }
    /// Declared R-profile CP14 identity input, changed only while paused.
    fn rprofile_identity(&self, _value: Option<u32>) -> Result<u32, String> {
        Err("backend has no R-profile identity input".into())
    }
    fn set_interrupt(&self, _line: Interrupt, _asserted: bool) -> Result<(), String> {
        Err("this backend does not expose architectural interrupt inputs".into())
    }
    /// `previous` permits sharing immutable private-memory pages between
    /// keyframes. It must belong to this CPU; backends without private RAM
    /// may ignore it. Physical board RAM is captured by the execution owner.
    fn snapshot(&self, previous: Option<&dyn Snapshot>) -> Result<Box<dyn Snapshot>, String>;
    fn restore(&self, snapshot: &dyn Snapshot) -> Result<(), String>;
    /// # Safety
    /// Callback data must remain valid until the hook is removed or the CPU
    /// is destroyed; callbacks must not unwind and must obey the owner's
    /// bounded reentrancy rules.
    unsafe fn install_hook(&self, hook: Hook) -> Result<HookId, String>;
    fn remove_hook(&self, hook: HookId) -> Result<(), String>;
    /// # Safety
    /// The callback context must outlive installation, as for install_hook.
    unsafe fn translation_hooks(&self, hooks: Option<TranslationHooks>) -> Result<(), String>;
    fn retry_block(&self, instructions: u32) -> Result<(), String>;
    fn request_restart(&self) -> Result<(), String>;
    fn accounting(&self) -> Option<NonNull<BlockAccounting>>;
    /// # Safety
    /// The bitmap is writable and covers every backing page; both buffers
    /// outlive observation and are accessed only under the owner's CPU rules.
    unsafe fn observe_backings(&self, ranges: &[BackingObserver]) -> Result<(), String>;
    fn watch_stores(&self, address: u64, bytes: u64) -> Result<(), String>;
    fn hook_watched(&self, hook: HookId, watched: bool) -> Result<(), String>;
    fn precise_store_pc(&self, on: bool) -> Result<(), String>;
    fn exit_counters(&self) -> Result<std::collections::BTreeMap<&'static str, u64>, String>;
}
