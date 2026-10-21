//! Native owner-thread proxy for one R-profile processor.
//!
//! Unicorn, the R-profile adapter and their `Rc` callback graph are created
//! and destroyed on the dedicated processor thread. The proxy stays on its
//! board thread and services guest callbacks there through synchronous RPC.
#![cfg(not(target_os = "emscripten"))]

use super::{core, memory::SharedMemory};
use crate::Library;
use fujimod_emulator_cpu::*;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

/// Inputs needed to construct one native R-profile processor on its owner
/// thread. The library is opened there; only its path crosses the thread edge.
pub struct Config {
    pub library: PathBuf,
    pub translation_cache_bytes: u32,
    pub options: core::Options,
    pub memory: SharedMemory,
}

/// Owner-thread CPU proxy. This value is intentionally thread-affine because
/// it holds callback pointers into the board and pumps them on its creator.
pub struct CpuWorker {
    commands: Sender<Message>,
    events: RefCell<Receiver<Event>>,
    worker: RefCell<Option<JoinHandle<()>>>,
    identity: Identity,
    features: Features,
    memory: SharedMemory,
    callbacks: RefCell<BTreeMap<usize, CallbackEntry>>,
    hooks: RefCell<BTreeMap<usize, RemoteHook>>,
    next_callback: Cell<usize>,
    next_run: Cell<u64>,
    running: Cell<bool>,
    callback_depth: Cell<u32>,
    _owner: PhantomData<Rc<()>>,
}

struct RemoteHook {
    remote: HookId,
    callback: usize,
}

enum CallbackEntry {
    Mmio(Mmio),
    Hook(Hook),
}

struct RemoteSnapshot {
    owner: Identity,
    token: u64,
    commands: Sender<Message>,
}
impl Snapshot for RemoteSnapshot {
    fn owner(&self) -> Identity {
        self.owner
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Drop for RemoteSnapshot {
    fn drop(&mut self) {
        let _ = self.commands.send(Message::ReleaseSnapshot(self.token));
    }
}

/// Start a dedicated owner thread and construct the Unicorn-backed Core there.
pub fn spawn(config: Config) -> Result<CpuWorker, String> {
    let memory = config.memory.clone();
    let (commands, command_rx) = mpsc::channel();
    let (event_tx, events) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("camera-rprofile-cpu".into())
        .spawn(move || serve(config, command_rx, event_tx, ready_tx))
        .map_err(|error| format!("start R-profile CPU worker: {error}"))?;
    let ready = match ready_rx.recv() {
        Ok(ready) => ready,
        Err(_) => {
            let _ = worker.join();
            return Err("R-profile CPU worker disconnected during construction".into());
        }
    };
    match ready {
        Ok((identity, features)) => Ok(CpuWorker {
            commands,
            events: RefCell::new(events),
            worker: RefCell::new(Some(worker)),
            identity,
            features,
            memory,
            callbacks: RefCell::new(BTreeMap::new()),
            hooks: RefCell::new(BTreeMap::new()),
            next_callback: Cell::new(1),
            next_run: Cell::new(1),
            running: Cell::new(false),
            callback_depth: Cell::new(0),
            _owner: PhantomData,
        }),
        Err(error) => {
            let _ = worker.join();
            Err(error)
        }
    }
}

enum Message {
    Request {
        operation: Operation,
        reply: Sender<Result<Value, String>>,
    },
    Run {
        id: u64,
        request: Run,
    },
    CallbackResponse {
        id: u64,
        result: CallbackResult,
    },
    ReleaseSnapshot(u64),
    Shutdown,
}

enum Event {
    Callback {
        id: u64,
        callback: usize,
        call: CallbackCall,
    },
    RunComplete {
        id: u64,
        result: Result<Exit, ExecutionError>,
    },
}

enum CallbackCall {
    MmioRead {
        offset: u64,
        size: u32,
    },
    MmioWrite {
        offset: u64,
        size: u32,
        value: u64,
    },
    Instruction {
        address: u64,
        size: u32,
    },
    Block(Block),
    Exception(arm::Exception),
    Memory {
        access: Access,
        address: u64,
        size: i32,
        value: i64,
    },
    Fault {
        fault: Fault,
        address: u64,
        size: i32,
        value: i64,
    },
}

#[derive(Clone, Copy)]
enum CallbackResult {
    Unit,
    U64(u64),
    Bool(bool),
    Error,
}

enum Operation {
    SetLoopMode(loops::LoopMode),
    SetLoopWindow(bool),
    SetExecutionPolicy(ExecutionPolicy),
    CheckpointBackings,
    Fingerprint,
    SetGuestClock(Clock),
    ReadRegister(Register, usize),
    WriteRegister(Register, Vec<u8>),
    Coprocessor(arm::Coprocessor, Option<u64>),
    ReadMemory(u64, usize),
    Translate(u64, Access),
    ReadVirtual(u64, Access, usize),
    WriteMemory(u64, Vec<u8>),
    MemoryWritten(u64, usize),
    BackingWritten(u64, usize, usize),
    MemoryRegions,
    MapBacking(u64, usize, u32, usize),
    MapPhysical(u64, u64, usize, u32, usize),
    ReplacePhysical(u64, u64, usize, u32, usize),
    MapZeroed(u64, usize, u32),
    MapMmio(u64, usize, usize, Vec<FastMmioRead>),
    Unmap(u64, usize),
    Protect(u64, usize, u32),
    InvalidateCode(Option<(u64, u64)>),
    TightenInstructionLimit(u64),
    Progress,
    Stop,
    StopBeforeInstruction,
    RprofileIdentity(Option<u32>),
    SetInterrupt(Interrupt, bool),
    Snapshot(Option<u64>),
    Restore(u64),
    InstallHook(usize, HookKind),
    RemoveHook(HookId),
    ObserveBackings(Vec<BackingRange>),
    WatchStores(u64, u64),
    HookWatched(HookId, bool),
    PreciseStorePc(bool),
    ExitCounters,
}

#[derive(Clone, Copy)]
enum HookKind {
    Instruction(HookRange),
    Block(HookRange),
    Exception,
    Memory {
        read: bool,
        write: bool,
        range: HookRange,
    },
    Fault,
}

#[derive(Clone, Copy)]
struct BackingRange {
    backing: usize,
    bytes: usize,
    bitmap: usize,
    page_shift: u32,
}

enum Value {
    Unit,
    U64(u64),
    OptionalU64(Option<u64>),
    Bytes(Vec<u8>),
    Backings(Vec<(usize, usize)>),
    MemoryRegions(Vec<MemoryRegion>),
    Fingerprint(serde_json::Value),
    Hook(HookId),
    Snapshot(u64),
    Identity(u32),
    Counters(Vec<(&'static str, u64)>),
}

struct CallbackContext {
    loop_state: *const WorkerLoop,
    callback: usize,
}
struct WorkerLoop {
    commands: RefCell<Receiver<Message>>,
    events: Sender<Event>,
    core: Cell<*const core::Core>,
    snapshots: RefCell<HashMap<u64, Box<dyn Snapshot>>>,
    callback_contexts: RefCell<BTreeMap<usize, Box<CallbackContext>>>,
    next_snapshot: Cell<u64>,
    next_event: Cell<u64>,
    callback_error: RefCell<Option<String>>,
}
impl WorkerLoop {
    fn callback_context(&self, callback: usize) -> *mut std::ffi::c_void {
        let mut contexts = self.callback_contexts.borrow_mut();
        contexts
            .entry(callback)
            .or_insert_with(|| {
                Box::new(CallbackContext {
                    loop_state: self,
                    callback,
                })
            })
            .as_mut() as *mut CallbackContext as *mut std::ffi::c_void
    }
    fn wait_callback(&self, callback: usize, call: CallbackCall) -> CallbackResult {
        let id = self.next_event.get();
        self.next_event.set(id.wrapping_add(1).max(1));
        if self
            .events
            .send(Event::Callback { id, callback, call })
            .is_err()
        {
            return CallbackResult::Error;
        }
        loop {
            let message = match self.commands.borrow_mut().recv() {
                Ok(message) => message,
                Err(_) => return CallbackResult::Error,
            };
            match message {
                Message::CallbackResponse {
                    id: response_id,
                    result,
                } if response_id == id => {
                    if matches!(result, CallbackResult::Error) {
                        *self.callback_error.borrow_mut() =
                            Some("owner callback failed while servicing R-profile CPU".into());
                        unsafe { &*self.core.get() }.stop().ok();
                    }
                    return result;
                }
                Message::Request { operation, reply } => {
                    let result = match unsafe { self.core.get().as_ref() } {
                        Some(cpu) => dispatch(self, cpu, operation),
                        None => Err("R-profile CPU worker is not initialized".into()),
                    };
                    let _ = reply.send(result);
                }
                Message::ReleaseSnapshot(token) => {
                    self.snapshots.borrow_mut().remove(&token);
                }
                Message::Run { id, .. } => {
                    let _ = self.events.send(Event::RunComplete {
                        id,
                        result: Err(ExecutionError::Backend(
                            "nested R-profile worker execution is refused".into(),
                        )),
                    });
                }
                Message::Shutdown => {
                    if let Some(cpu) = unsafe { self.core.get().as_ref() } {
                        cpu.stop().ok();
                    }
                    return CallbackResult::Error;
                }
                Message::CallbackResponse { .. } => {
                    return CallbackResult::Error;
                }
            }
        }
    }
}

unsafe fn callback_context(data: *mut std::ffi::c_void) -> &'static CallbackContext {
    unsafe { &*data.cast::<CallbackContext>() }
}
unsafe fn callback_loop(data: *mut std::ffi::c_void) -> &'static WorkerLoop {
    unsafe { &*callback_context(data).loop_state }
}
unsafe fn mmio_read(offset: u64, size: u32, data: *mut std::ffi::c_void) -> u64 {
    let context = unsafe { callback_context(data) };
    match unsafe {
        callback_loop(data).wait_callback(context.callback, CallbackCall::MmioRead { offset, size })
    } {
        CallbackResult::U64(value) => value,
        _ => 0,
    }
}
unsafe fn mmio_write(offset: u64, size: u32, value: u64, data: *mut std::ffi::c_void) {
    let context = unsafe { callback_context(data) };
    unsafe {
        callback_loop(data).wait_callback(
            context.callback,
            CallbackCall::MmioWrite {
                offset,
                size,
                value,
            },
        );
    }
}
unsafe fn instruction(address: u64, size: u32, data: *mut std::ffi::c_void) {
    let context = unsafe { callback_context(data) };
    unsafe {
        callback_loop(data).wait_callback(
            context.callback,
            CallbackCall::Instruction { address, size },
        );
    }
}
unsafe fn block(block: Block, data: *mut std::ffi::c_void) {
    let context = unsafe { callback_context(data) };
    unsafe {
        callback_loop(data).wait_callback(context.callback, CallbackCall::Block(block));
    }
}
unsafe fn exception(exception: arm::Exception, data: *mut std::ffi::c_void) {
    let context = unsafe { callback_context(data) };
    unsafe {
        callback_loop(data).wait_callback(context.callback, CallbackCall::Exception(exception));
    }
}
unsafe fn memory(access: Access, address: u64, size: i32, value: i64, data: *mut std::ffi::c_void) {
    let context = unsafe { callback_context(data) };
    unsafe {
        callback_loop(data).wait_callback(
            context.callback,
            CallbackCall::Memory {
                access,
                address,
                size,
                value,
            },
        );
    }
}
unsafe fn fault(
    fault: Fault,
    address: u64,
    size: i32,
    value: i64,
    data: *mut std::ffi::c_void,
) -> bool {
    let context = unsafe { callback_context(data) };
    match unsafe {
        callback_loop(data).wait_callback(
            context.callback,
            CallbackCall::Fault {
                fault,
                address,
                size,
                value,
            },
        )
    } {
        CallbackResult::Bool(value) => value,
        _ => false,
    }
}

fn serve(
    config: Config,
    commands: Receiver<Message>,
    events: Sender<Event>,
    ready: Sender<Result<(Identity, Features), String>>,
) {
    let context = Box::new(WorkerLoop {
        commands: RefCell::new(commands),
        events: events.clone(),
        core: Cell::new(std::ptr::null()),
        snapshots: RefCell::new(HashMap::new()),
        callback_contexts: RefCell::new(BTreeMap::new()),
        next_snapshot: Cell::new(1),
        next_event: Cell::new(1),
        callback_error: RefCell::new(None),
    });
    let library = match Library::load(&config.library) {
        Ok(library) => library,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let engine = match library.open(Model::CortexR5F, config.translation_cache_bytes) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let cpu = match core::Core::new(
        engine,
        config.options,
        core::Wiring {
            memory: config.memory.processor(),
        },
    ) {
        Ok(cpu) => Box::new(cpu),
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    context.core.set(&*cpu);
    if ready.send(Ok((cpu.identity(), cpu.features()))).is_err() {
        return;
    }
    loop {
        let message = match context.commands.borrow_mut().recv() {
            Ok(message) => message,
            Err(_) => break,
        };
        match message {
            Message::Request { operation, reply } => {
                let _ = reply.send(dispatch(&context, &cpu, operation));
            }
            Message::Run { id, request } => {
                context.callback_error.borrow_mut().take();
                let mut result = cpu.run(request);
                if let Some(error) = context.callback_error.borrow_mut().take() {
                    result = Err(ExecutionError::Backend(error));
                }
                let _ = events.send(Event::RunComplete { id, result });
            }
            Message::ReleaseSnapshot(token) => {
                context.snapshots.borrow_mut().remove(&token);
            }
            Message::Shutdown => break,
            Message::CallbackResponse { .. } => break,
        }
    }
    context.core.set(std::ptr::null());
}

fn dispatch(context: &WorkerLoop, cpu: &core::Core, operation: Operation) -> Result<Value, String> {
    let result = match operation {
        Operation::SetLoopMode(mode) => {
            cpu.set_loop_mode(mode)?;
            Value::Unit
        }
        Operation::SetLoopWindow(stable) => {
            cpu.set_loop_window(stable)?;
            Value::Unit
        }
        Operation::SetExecutionPolicy(policy) => {
            cpu.set_execution_policy(policy)?;
            Value::Unit
        }
        Operation::CheckpointBackings => Value::Backings(
            cpu.checkpoint_backings()?
                .into_iter()
                .map(|(pointer, bytes)| (pointer.as_ptr() as usize, bytes))
                .collect(),
        ),
        Operation::Fingerprint => Value::Fingerprint(cpu.fingerprint()?),
        Operation::SetGuestClock(clock) => {
            cpu.set_guest_clock(clock)?;
            Value::Unit
        }
        Operation::ReadRegister(register, bytes) => {
            let mut out = vec![0; bytes];
            let read = cpu.read_register(register, &mut out)?;
            out.truncate(read);
            Value::Bytes(out)
        }
        Operation::WriteRegister(register, bytes) => {
            cpu.write_register(register, &bytes)?;
            Value::Unit
        }
        Operation::Coprocessor(register, value) => Value::U64(cpu.coprocessor(register, value)?),
        Operation::ReadMemory(address, bytes) => {
            let mut out = vec![0; bytes];
            cpu.read_memory(address, &mut out)?;
            Value::Bytes(out)
        }
        Operation::Translate(address, access) => Value::U64(cpu.translate(address, access)?),
        Operation::ReadVirtual(address, access, bytes) => {
            let mut out = vec![0; bytes];
            cpu.read_virtual(address, access, &mut out)?;
            Value::Bytes(out)
        }
        Operation::WriteMemory(address, bytes) => {
            cpu.write_memory(address, &bytes)?;
            Value::Unit
        }
        Operation::MemoryWritten(address, bytes) => {
            cpu.memory_written(address, bytes)?;
            Value::Unit
        }
        Operation::BackingWritten(physical, pointer, bytes) => {
            unsafe {
                cpu.backing_written(
                    physical,
                    NonNull::new(pointer as *mut u8).ok_or("null backing pointer")?,
                    bytes,
                )?;
            }
            Value::Unit
        }
        Operation::MemoryRegions => Value::MemoryRegions(cpu.memory_regions()?),
        Operation::MapBacking(address, bytes, permissions, pointer) => {
            unsafe {
                cpu.map_backing(
                    address,
                    bytes,
                    permissions,
                    NonNull::new(pointer as *mut u8).ok_or("null backing pointer")?,
                )?;
            }
            Value::Unit
        }
        Operation::MapPhysical(address, physical, bytes, permissions, pointer) => {
            unsafe {
                cpu.map_physical(
                    address,
                    physical,
                    bytes,
                    permissions,
                    NonNull::new(pointer as *mut u8).ok_or("null backing pointer")?,
                )?;
            }
            Value::Unit
        }
        Operation::ReplacePhysical(address, physical, bytes, permissions, pointer) => {
            unsafe {
                cpu.replace_physical(
                    address,
                    physical,
                    bytes,
                    permissions,
                    NonNull::new(pointer as *mut u8).ok_or("null backing pointer")?,
                )?;
            }
            Value::Unit
        }
        Operation::MapZeroed(address, bytes, permissions) => {
            cpu.map_zeroed(address, bytes, permissions)?;
            Value::Unit
        }
        Operation::MapMmio(address, bytes, callback, fast_reads) => {
            let data = context.callback_context(callback);
            let callbacks = Mmio {
                read: mmio_read,
                write: mmio_write,
                data,
            };
            unsafe {
                if !fast_reads.is_empty() {
                    cpu.map_mmio_fast(address, bytes, callbacks, fast_reads)?;
                } else {
                    cpu.map_mmio(address, bytes, callbacks)?;
                }
            }
            Value::Unit
        }
        Operation::Unmap(address, bytes) => {
            cpu.unmap(address, bytes)?;
            Value::Unit
        }
        Operation::Protect(address, bytes, permissions) => {
            cpu.protect(address, bytes, permissions)?;
            Value::Unit
        }
        Operation::InvalidateCode(range) => {
            cpu.invalidate_code(range)?;
            Value::Unit
        }
        Operation::TightenInstructionLimit(total) => {
            cpu.tighten_instruction_limit(total)?;
            Value::Unit
        }
        Operation::Progress => Value::OptionalU64(cpu.instruction_progress()),
        Operation::Stop => {
            cpu.stop()?;
            Value::Unit
        }
        Operation::StopBeforeInstruction => {
            cpu.stop_before_instruction()?;
            Value::Unit
        }
        Operation::RprofileIdentity(value) => Value::Identity(cpu.rprofile_identity(value)?),
        Operation::SetInterrupt(line, asserted) => {
            cpu.set_interrupt(line, asserted)?;
            Value::Unit
        }
        Operation::Snapshot(previous) => {
            let snapshots = context.snapshots.borrow();
            let previous = previous.and_then(|token| snapshots.get(&token).map(Box::as_ref));
            let snapshot = cpu.snapshot(previous)?;
            drop(snapshots);
            let token = context.next_snapshot.get();
            context.next_snapshot.set(token.wrapping_add(1).max(1));
            context.snapshots.borrow_mut().insert(token, snapshot);
            Value::Snapshot(token)
        }
        Operation::Restore(token) => {
            let snapshots = context.snapshots.borrow();
            let snapshot = snapshots
                .get(&token)
                .ok_or("unknown worker snapshot token")?;
            cpu.restore(snapshot.as_ref())?;
            Value::Unit
        }
        Operation::InstallHook(callback, kind) => {
            let data = context.callback_context(callback);
            let hook = match kind {
                HookKind::Instruction(range) => Hook::Instruction {
                    callback: instruction,
                    data,
                    range,
                },
                HookKind::Block(range) => Hook::Block {
                    callback: block,
                    data,
                    range,
                },
                HookKind::Exception => Hook::Exception {
                    callback: exception,
                    data,
                },
                HookKind::Memory { read, write, range } => Hook::Memory {
                    callback: memory,
                    data,
                    read,
                    write,
                    range,
                },
                HookKind::Fault => Hook::Fault {
                    callback: fault,
                    data,
                },
            };
            Value::Hook(unsafe { cpu.install_hook(hook)? })
        }
        Operation::RemoveHook(hook) => {
            cpu.remove_hook(hook)?;
            Value::Unit
        }
        Operation::ObserveBackings(ranges) => {
            let ranges = ranges
                .into_iter()
                .map(|range| BackingObserver {
                    backing: NonNull::new(range.backing as *mut u8).expect("checked backing"),
                    bytes: range.bytes,
                    bitmap: NonNull::new(range.bitmap as *mut u64).expect("checked bitmap"),
                    page_shift: range.page_shift,
                })
                .collect::<Vec<_>>();
            unsafe { cpu.observe_backings(&ranges)? };
            Value::Unit
        }
        Operation::WatchStores(address, bytes) => {
            cpu.watch_stores(address, bytes)?;
            Value::Unit
        }
        Operation::HookWatched(hook, watched) => {
            cpu.hook_watched(hook, watched)?;
            Value::Unit
        }
        Operation::PreciseStorePc(on) => {
            cpu.precise_store_pc(on)?;
            Value::Unit
        }
        Operation::ExitCounters => Value::Counters(cpu.exit_counters()?.into_iter().collect()),
    };
    Ok(result)
}

impl CpuWorker {
    fn rpc(&self, operation: Operation) -> Result<Value, String> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(Message::Request { operation, reply })
            .map_err(|_| "R-profile CPU worker disconnected")?;
        response
            .recv()
            .map_err(|_| "R-profile CPU worker reply disconnected")?
    }
    fn unit(&self, operation: Operation) -> Result<(), String> {
        match self.rpc(operation)? {
            Value::Unit => Ok(()),
            _ => Err("R-profile CPU worker returned an unexpected reply".into()),
        }
    }
    fn next_callback(&self) -> Result<usize, String> {
        let id = self.next_callback.get();
        self.next_callback
            .set(id.checked_add(1).ok_or("callback identity overflow")?);
        Ok(id)
    }
    fn service_callback(&self, callback: usize, call: CallbackCall) -> CallbackResult {
        let entry = self
            .callbacks
            .borrow()
            .get(&callback)
            .map(|entry| match entry {
                CallbackEntry::Mmio(mmio) => CallbackEntry::Mmio(*mmio),
                CallbackEntry::Hook(hook) => CallbackEntry::Hook(*hook),
            });
        let Some(entry) = entry else {
            return CallbackResult::Error;
        };
        self.callback_depth
            .set(self.callback_depth.get().saturating_add(1));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            match (entry, call) {
                (CallbackEntry::Mmio(mmio), CallbackCall::MmioRead { offset, size }) => {
                    CallbackResult::U64((mmio.read)(offset, size, mmio.data))
                }
                (
                    CallbackEntry::Mmio(mmio),
                    CallbackCall::MmioWrite {
                        offset,
                        size,
                        value,
                    },
                ) => {
                    (mmio.write)(offset, size, value, mmio.data);
                    CallbackResult::Unit
                }
                (
                    CallbackEntry::Hook(Hook::Instruction { callback, data, .. }),
                    CallbackCall::Instruction { address, size },
                ) => {
                    callback(address, size, data);
                    CallbackResult::Unit
                }
                (
                    CallbackEntry::Hook(Hook::Block { callback, data, .. }),
                    CallbackCall::Block(block),
                ) => {
                    callback(block, data);
                    CallbackResult::Unit
                }
                (
                    CallbackEntry::Hook(Hook::Exception { callback, data }),
                    CallbackCall::Exception(exception),
                ) => {
                    callback(exception, data);
                    CallbackResult::Unit
                }
                (
                    CallbackEntry::Hook(Hook::Memory { callback, data, .. }),
                    CallbackCall::Memory {
                        access,
                        address,
                        size,
                        value,
                    },
                ) => {
                    callback(access, address, size, value, data);
                    CallbackResult::Unit
                }
                (
                    CallbackEntry::Hook(Hook::Fault { callback, data }),
                    CallbackCall::Fault {
                        fault,
                        address,
                        size,
                        value,
                    },
                ) => CallbackResult::Bool(callback(fault, address, size, value, data)),
                _ => CallbackResult::Error,
            }
        }));
        self.callback_depth
            .set(self.callback_depth.get().saturating_sub(1));
        result.unwrap_or(CallbackResult::Error)
    }
}

impl Cpu for CpuWorker {
    fn set_loop_mode(&self, mode: loops::LoopMode) -> Result<(), String> {
        self.unit(Operation::SetLoopMode(mode))
    }
    fn set_loop_window(&self, stable: bool) -> Result<(), String> {
        self.unit(Operation::SetLoopWindow(stable))
    }
    fn identity(&self) -> Identity {
        self.identity
    }
    fn set_execution_policy(&self, policy: ExecutionPolicy) -> Result<(), String> {
        self.unit(Operation::SetExecutionPolicy(policy))
    }
    fn checkpoint_backings(&self) -> Result<Vec<(NonNull<u8>, usize)>, String> {
        match self.rpc(Operation::CheckpointBackings)? {
            Value::Backings(backings) => backings
                .into_iter()
                .map(|(pointer, bytes)| {
                    Ok((
                        NonNull::new(pointer as *mut u8).ok_or("null backing pointer")?,
                        bytes,
                    ))
                })
                .collect(),
            _ => Err("R-profile CPU worker returned an unexpected backing list".into()),
        }
    }
    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        match self.rpc(Operation::Fingerprint)? {
            Value::Fingerprint(value) => Ok(value),
            _ => Err("R-profile CPU worker returned an unexpected fingerprint".into()),
        }
    }
    fn exclusive_domain(&self) -> Option<exclusive::Shared> {
        Some(self.memory.exclusive_domain())
    }
    fn model(&self) -> Model {
        Model::CortexR5F
    }
    fn features(&self) -> Features {
        self.features
    }
    fn set_guest_clock(&self, clock: Clock) -> Result<(), String> {
        self.unit(Operation::SetGuestClock(clock))
    }
    fn read_register(&self, register: Register, out: &mut [u8]) -> Result<usize, String> {
        match self.rpc(Operation::ReadRegister(register, out.len()))? {
            Value::Bytes(bytes) => {
                if bytes.len() > out.len() {
                    return Err("worker register width exceeds output buffer".into());
                }
                out[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            _ => Err("R-profile CPU worker returned unexpected register bytes".into()),
        }
    }
    fn write_register(&self, register: Register, bytes: &[u8]) -> Result<(), String> {
        self.unit(Operation::WriteRegister(register, bytes.to_vec()))
    }
    fn coprocessor(&self, register: arm::Coprocessor, value: Option<u64>) -> Result<u64, String> {
        match self.rpc(Operation::Coprocessor(register, value))? {
            Value::U64(value) => Ok(value),
            _ => Err("R-profile CPU worker returned an unexpected coprocessor value".into()),
        }
    }
    fn read_memory(&self, address: u64, out: &mut [u8]) -> Result<(), String> {
        match self.rpc(Operation::ReadMemory(address, out.len()))? {
            Value::Bytes(bytes) if bytes.len() == out.len() => {
                out.copy_from_slice(&bytes);
                Ok(())
            }
            Value::Bytes(_) => Err("worker memory read returned an unexpected length".into()),
            _ => Err("R-profile CPU worker returned unexpected memory bytes".into()),
        }
    }
    fn translate(&self, address: u64, access: Access) -> Result<u64, String> {
        match self.rpc(Operation::Translate(address, access))? {
            Value::U64(address) => Ok(address),
            _ => Err("R-profile CPU worker returned an unexpected translation".into()),
        }
    }
    fn read_virtual(&self, address: u64, access: Access, out: &mut [u8]) -> Result<(), String> {
        match self.rpc(Operation::ReadVirtual(address, access, out.len()))? {
            Value::Bytes(bytes) if bytes.len() == out.len() => {
                out.copy_from_slice(&bytes);
                Ok(())
            }
            Value::Bytes(_) => Err("worker virtual read returned an unexpected length".into()),
            _ => Err("R-profile CPU worker returned unexpected virtual bytes".into()),
        }
    }
    fn write_memory(&self, address: u64, bytes: &[u8]) -> Result<(), String> {
        self.unit(Operation::WriteMemory(address, bytes.to_vec()))
    }
    fn memory_written(&self, address: u64, bytes: usize) -> Result<(), String> {
        self.unit(Operation::MemoryWritten(address, bytes))
    }
    unsafe fn backing_written(
        &self,
        physical: u64,
        backing: NonNull<u8>,
        bytes: usize,
    ) -> Result<(), String> {
        self.unit(Operation::BackingWritten(
            physical,
            backing.as_ptr() as usize,
            bytes,
        ))
    }
    fn memory_regions(&self) -> Result<Vec<MemoryRegion>, String> {
        match self.rpc(Operation::MemoryRegions)? {
            Value::MemoryRegions(regions) => Ok(regions),
            _ => Err("R-profile CPU worker returned unexpected memory regions".into()),
        }
    }
    unsafe fn map_backing(
        &self,
        address: u64,
        bytes: usize,
        permissions: u32,
        backing: NonNull<u8>,
    ) -> Result<(), String> {
        self.unit(Operation::MapBacking(
            address,
            bytes,
            permissions,
            backing.as_ptr() as usize,
        ))
    }
    unsafe fn map_physical(
        &self,
        address: u64,
        physical: u64,
        bytes: usize,
        permissions: u32,
        backing: NonNull<u8>,
    ) -> Result<(), String> {
        self.unit(Operation::MapPhysical(
            address,
            physical,
            bytes,
            permissions,
            backing.as_ptr() as usize,
        ))
    }
    unsafe fn replace_physical(
        &self,
        address: u64,
        physical: u64,
        bytes: usize,
        permissions: u32,
        backing: NonNull<u8>,
    ) -> Result<(), String> {
        self.unit(Operation::ReplacePhysical(
            address,
            physical,
            bytes,
            permissions,
            backing.as_ptr() as usize,
        ))
    }
    fn map_zeroed(&self, address: u64, bytes: usize, permissions: u32) -> Result<(), String> {
        self.unit(Operation::MapZeroed(address, bytes, permissions))
    }
    unsafe fn map_mmio(&self, address: u64, bytes: usize, callbacks: Mmio) -> Result<(), String> {
        let id = self.next_callback()?;
        self.callbacks
            .borrow_mut()
            .insert(id, CallbackEntry::Mmio(callbacks));
        if let Err(error) = self.unit(Operation::MapMmio(address, bytes, id, Vec::new())) {
            self.callbacks.borrow_mut().remove(&id);
            return Err(error);
        }
        Ok(())
    }
    unsafe fn map_mmio_fast(
        &self,
        address: u64,
        bytes: usize,
        callbacks: Mmio,
        fast_reads: Vec<FastMmioRead>,
    ) -> Result<(), String> {
        let id = self.next_callback()?;
        self.callbacks
            .borrow_mut()
            .insert(id, CallbackEntry::Mmio(callbacks));
        if let Err(error) = self.unit(Operation::MapMmio(address, bytes, id, fast_reads)) {
            self.callbacks.borrow_mut().remove(&id);
            return Err(error);
        }
        Ok(())
    }
    fn unmap(&self, address: u64, bytes: usize) -> Result<(), String> {
        self.unit(Operation::Unmap(address, bytes))
    }
    fn protect(&self, address: u64, bytes: usize, permissions: u32) -> Result<(), String> {
        self.unit(Operation::Protect(address, bytes, permissions))
    }
    fn invalidate_code(&self, range: Option<(u64, u64)>) -> Result<(), String> {
        self.unit(Operation::InvalidateCode(range))
    }
    fn run(&self, request: Run) -> Result<Exit, ExecutionError> {
        if self.running.replace(true) {
            return Err(ExecutionError::Backend(
                "nested R-profile worker execution is refused".into(),
            ));
        }
        let id = self.next_run.get();
        self.next_run.set(id.wrapping_add(1).max(1));
        if self.commands.send(Message::Run { id, request }).is_err() {
            self.running.set(false);
            return Err(ExecutionError::Backend(
                "R-profile CPU worker disconnected".into(),
            ));
        }
        loop {
            let event = match self.events.borrow_mut().recv() {
                Ok(event) => event,
                Err(_) => {
                    self.running.set(false);
                    return Err(ExecutionError::Backend(
                        "R-profile CPU worker disconnected".into(),
                    ));
                }
            };
            match event {
                Event::Callback {
                    id: callback_id,
                    callback,
                    call,
                } => {
                    let result = self.service_callback(callback, call);
                    if self
                        .commands
                        .send(Message::CallbackResponse {
                            id: callback_id,
                            result,
                        })
                        .is_err()
                    {
                        self.running.set(false);
                        return Err(ExecutionError::Backend(
                            "R-profile CPU worker disconnected during callback".into(),
                        ));
                    }
                }
                Event::RunComplete {
                    id: completed,
                    result,
                } if completed == id => {
                    self.running.set(false);
                    return result;
                }
                Event::RunComplete { .. } => continue,
            }
        }
    }
    fn tighten_instruction_limit(&self, total: u64) -> Result<(), String> {
        self.unit(Operation::TightenInstructionLimit(total))
    }
    fn instruction_progress(&self) -> Option<u64> {
        match self.rpc(Operation::Progress).ok()? {
            Value::OptionalU64(progress) => progress,
            _ => None,
        }
    }
    fn stop(&self) -> Result<(), String> {
        self.unit(Operation::Stop)
    }
    fn stop_before_instruction(&self) -> Result<(), String> {
        self.unit(Operation::StopBeforeInstruction)
    }
    fn rprofile_identity(&self, value: Option<u32>) -> Result<u32, String> {
        match self.rpc(Operation::RprofileIdentity(value))? {
            Value::Identity(value) => Ok(value),
            _ => Err("R-profile CPU worker returned unexpected identity".into()),
        }
    }
    fn set_interrupt(&self, line: Interrupt, asserted: bool) -> Result<(), String> {
        self.unit(Operation::SetInterrupt(line, asserted))
    }
    fn snapshot(&self, previous: Option<&dyn Snapshot>) -> Result<Box<dyn Snapshot>, String> {
        let previous = previous
            .map(|snapshot| -> Result<u64, String> {
                if snapshot.owner() != self.identity {
                    return Err("CPU snapshot belongs to a different processor".into());
                }
                snapshot
                    .as_any()
                    .downcast_ref::<RemoteSnapshot>()
                    .map(|snapshot| snapshot.token)
                    .ok_or_else(|| "CPU snapshot belongs to a different backend".into())
            })
            .transpose()?;
        let token = match self.rpc(Operation::Snapshot(previous))? {
            Value::Snapshot(token) => token,
            _ => return Err("R-profile CPU worker returned unexpected snapshot".into()),
        };
        Ok(Box::new(RemoteSnapshot {
            owner: self.identity,
            token,
            commands: self.commands.clone(),
        }))
    }
    fn restore(&self, snapshot: &dyn Snapshot) -> Result<(), String> {
        if snapshot.owner() != self.identity {
            return Err("CPU snapshot belongs to a different processor".into());
        }
        let snapshot = snapshot
            .as_any()
            .downcast_ref::<RemoteSnapshot>()
            .ok_or("CPU snapshot belongs to a different backend")?;
        self.unit(Operation::Restore(snapshot.token))
    }
    unsafe fn install_hook(&self, hook: Hook) -> Result<HookId, String> {
        let callback = self.next_callback()?;
        let kind = match hook {
            Hook::Instruction { range, .. } => HookKind::Instruction(range),
            Hook::Block { range, .. } => HookKind::Block(range),
            Hook::Exception { .. } => HookKind::Exception,
            Hook::Memory {
                read, write, range, ..
            } => HookKind::Memory { read, write, range },
            Hook::Fault { .. } => HookKind::Fault,
        };
        self.callbacks
            .borrow_mut()
            .insert(callback, CallbackEntry::Hook(hook));
        let remote = match self.rpc(Operation::InstallHook(callback, kind))? {
            Value::Hook(hook) => hook,
            _ => {
                self.callbacks.borrow_mut().remove(&callback);
                return Err("R-profile CPU worker returned unexpected hook".into());
            }
        };
        self.hooks
            .borrow_mut()
            .insert(remote.key, RemoteHook { remote, callback });
        Ok(remote)
    }
    fn remove_hook(&self, hook: HookId) -> Result<(), String> {
        let remote = self
            .hooks
            .borrow()
            .get(&hook.key)
            .filter(|remote| hook.owner == self.identity && remote.remote == hook)
            .map(|remote| (remote.remote, remote.callback))
            .ok_or("hook belongs to a different CPU or was removed")?;
        self.unit(Operation::RemoveHook(remote.0))?;
        self.hooks.borrow_mut().remove(&hook.key);
        self.callbacks.borrow_mut().remove(&remote.1);
        Ok(())
    }
    unsafe fn translation_hooks(&self, _: Option<TranslationHooks>) -> Result<(), String> {
        Err("R-profile translation hooks are owned by the backend".into())
    }
    fn retry_block(&self, _: u32) -> Result<(), String> {
        Err("R-profile block retries are owned by the backend".into())
    }
    fn request_restart(&self) -> Result<(), String> {
        Err("R-profile restarts are owned by the backend".into())
    }
    fn accounting(&self) -> Option<NonNull<BlockAccounting>> {
        None
    }
    unsafe fn observe_backings(&self, ranges: &[BackingObserver]) -> Result<(), String> {
        let ranges = ranges
            .iter()
            .map(|range| BackingRange {
                backing: range.backing.as_ptr() as usize,
                bytes: range.bytes,
                bitmap: range.bitmap.as_ptr() as usize,
                page_shift: range.page_shift,
            })
            .collect();
        self.unit(Operation::ObserveBackings(ranges))
    }
    fn watch_stores(&self, address: u64, bytes: u64) -> Result<(), String> {
        self.unit(Operation::WatchStores(address, bytes))
    }
    fn hook_watched(&self, hook: HookId, watched: bool) -> Result<(), String> {
        if hook.owner != self.identity || !self.hooks.borrow().contains_key(&hook.key) {
            return Err("hook belongs to a different CPU or was removed".into());
        }
        self.unit(Operation::HookWatched(hook, watched))
    }
    fn precise_store_pc(&self, on: bool) -> Result<(), String> {
        self.unit(Operation::PreciseStorePc(on))
    }
    fn exit_counters(&self) -> Result<BTreeMap<&'static str, u64>, String> {
        match self.rpc(Operation::ExitCounters)? {
            Value::Counters(counters) => Ok(counters.into_iter().collect()),
            _ => Err("R-profile CPU worker returned unexpected exit counters".into()),
        }
    }
}

impl Drop for CpuWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(Message::Shutdown);
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_cpu::memory::Backing;

    fn worker(memory: SharedMemory) -> CpuWorker {
        let library = std::env::var_os("CAMERA_UNICORN_LIBRARY")
            .expect("CAMERA_UNICORN_LIBRARY must select the counted Unicorn library");
        spawn(Config {
            library: PathBuf::from(library),
            translation_cache_bytes: 1024 * 1024,
            options: core::Options::default(),
            memory,
        })
        .expect("start R-profile worker")
    }

    fn read_u32(cpu: &CpuWorker, register: Register) -> u32 {
        let mut bytes = [0; 4];
        assert_eq!(cpu.read_register(register, &mut bytes).unwrap(), 4);
        u32::from_le_bytes(bytes)
    }

    fn write_words(cpu: &CpuWorker, address: u64, words: &[u32]) {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        cpu.write_memory(address, &bytes).unwrap();
    }

    fn one_step(cpu: &CpuWorker, begin: u64) -> Result<Exit, ExecutionError> {
        cpu.run(Run {
            begin,
            until: None,
            watchdog_us: 0,
            instructions: Some(1),
        })
    }

    #[test]
    fn two_worker_cores_share_backing_and_keep_snapshots_worker_local() {
        let memory = SharedMemory::default();
        let backing = Backing::aligned_zeros(4096, 4096).unwrap();
        let pointer = NonNull::new(backing.as_ptr()).unwrap();
        let first = worker(memory.clone());
        let second = worker(memory);
        unsafe {
            first
                .map_physical(0x1000, 0x2000, 4096, 7, pointer)
                .unwrap();
            second
                .map_physical(0x1000, 0x2000, 4096, 7, pointer)
                .unwrap();
        }
        write_words(&first, 0x1000, &[0xe3a00007, 0xe2800001, 0xe320f003]);

        one_step(&first, 0x1000).unwrap();
        assert_eq!(read_u32(&first, arm::CORE[0]), 7);
        let snapshot = first.snapshot(None).unwrap();
        one_step(&first, 0x1004).unwrap();
        assert_eq!(read_u32(&first, arm::CORE[0]), 8);
        first.restore(snapshot.as_ref()).unwrap();
        assert_eq!(read_u32(&first, arm::CORE[0]), 7);
        assert_eq!(read_u32(&first, arm::PC), 0x1004);
        assert!(second.restore(snapshot.as_ref()).is_err());

        one_step(&second, 0x1000).unwrap();
        assert_eq!(read_u32(&second, arm::CORE[0]), 7);
    }

    #[test]
    fn owner_mmio_callback_can_make_a_nested_worker_request() {
        struct Probe {
            cpu: *const CpuWorker,
            calls: u32,
            progress: Option<u64>,
        }
        unsafe fn read(_: u64, size: u32, data: *mut std::ffi::c_void) -> u64 {
            assert_eq!(size, 4);
            let probe = unsafe { &mut *data.cast::<Probe>() };
            probe.calls += 1;
            probe.progress = Some(unsafe { &*probe.cpu }.instruction_progress().unwrap());
            0x1234_5678
        }
        unsafe fn write(_: u64, _: u32, _: u64, _: *mut std::ffi::c_void) {}

        let cpu = worker(SharedMemory::default());
        cpu.map_zeroed(0x1000, 4096, 7).unwrap();
        write_words(&cpu, 0x1000, &[0xe5910000, 0xe320f003]); // ldr r0,[r1]; wfi
        cpu.write_register(arm::CORE[1], &0x8000u32.to_le_bytes())
            .unwrap();
        let mut probe = Probe {
            cpu: &cpu,
            calls: 0,
            progress: None,
        };
        unsafe {
            cpu.map_mmio(
                0x8000,
                4096,
                Mmio {
                    read,
                    write,
                    data: (&mut probe as *mut Probe).cast(),
                },
            )
            .unwrap();
        }
        one_step(&cpu, 0x1000).unwrap();
        assert_eq!(read_u32(&cpu, arm::CORE[0]), 0x1234_5678);
        assert_eq!(probe.calls, 1);
        assert!(probe.progress.is_some());
    }

    #[test]
    fn interactive_fast_mmio_reads_use_declared_atomic_snapshots() {
        struct ReadCount(Cell<u32>);
        unsafe fn read(_: u64, size: u32, data: *mut std::ffi::c_void) -> u64 {
            assert_eq!(size, 4);
            let count = unsafe { &*data.cast::<ReadCount>() };
            count.0.set(count.0.get() + 1);
            0x1111
        }
        unsafe fn write(_: u64, _: u32, _: u64, _: *mut std::ffi::c_void) {}

        let cpu = worker(SharedMemory::default());
        cpu.set_execution_policy(ExecutionPolicy::Interactive)
            .unwrap();
        cpu.map_zeroed(0x1000, 4096, 7).unwrap();
        write_words(&cpu, 0x1000, &[0xe5910000, 0xe320f003]);
        cpu.write_register(arm::CORE[1], &0x8000u32.to_le_bytes())
            .unwrap();
        let mut count = ReadCount(Cell::new(0));
        let callbacks = Mmio {
            read,
            write,
            data: (&mut count as *mut ReadCount).cast(),
        };
        let value = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0x2222));
        let fast_read = FastMmioRead::atomic_u32(0, value.clone());
        unsafe {
            cpu.map_mmio_fast(0x8000, 4096, callbacks, vec![fast_read.clone()])
                .unwrap();
        }

        value.store(0x3333, std::sync::atomic::Ordering::Release);
        cpu.run(Run {
            begin: 0x1000,
            until: None,
            watchdog_us: 0,
            instructions: Some(1),
        })
        .unwrap();
        assert_eq!(read_u32(&cpu, arm::CORE[0]), 0x3333);
        assert_eq!(read_u32(&cpu, arm::PC), 0x1004);
        assert_eq!(cpu.instruction_progress().unwrap(), 1);
        assert_eq!(count.0.get(), 0);

        cpu.set_execution_policy(ExecutionPolicy::Exact).unwrap();
        cpu.write_register(arm::PC, &0x1000u32.to_le_bytes())
            .unwrap();
        cpu.run(Run {
            begin: 0x1000,
            until: None,
            watchdog_us: 0,
            instructions: Some(3),
        })
        .unwrap();
        assert_eq!(read_u32(&cpu, arm::CORE[0]), 0x1111);
        assert_eq!(count.0.get(), 1);
    }
}
