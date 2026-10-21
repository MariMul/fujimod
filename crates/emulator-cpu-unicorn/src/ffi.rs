//! Minimal, version-pinned Unicorn 2.1.4 public C ABI. No Python binding handles.
//! Definitions checked against include/unicorn/{unicorn,arm}.h at tag 2.1.4.
use fujimod_emulator_cpu::BackingObserver;
#[cfg(not(target_os = "emscripten"))]
use libloading::Library;
use std::ffi::{c_char, c_int, c_void};
use std::path::Path;

pub type Engine = *mut c_void;
pub type Context = *mut c_void;
pub type Error = c_int;
pub const fn ctl(kind: u32, count: u32, direction: u32) -> u32 {
    kind | (count << 26) | (direction << 30)
}

#[repr(C)]
pub struct CpReg {
    pub cp: u32,
    pub is64: u32,
    pub sec: u32,
    pub crn: u32,
    pub crm: u32,
    pub opc1: u32,
    pub opc2: u32,
    pub val: u64,
}
#[repr(C)]
pub struct MemRegion {
    pub begin: u64,
    pub end: u64,
    pub perms: u32,
}

/// Acceleration exports added by the pinned Unicorn source build.
/// Missing exports require the explicit slow-Unicorn opt-in before loading.
pub struct Extensions {
    pub write_hook_pc_sync: Option<unsafe extern "C" fn(Engine, bool) -> Error>,
    pub page_hooks: Option<unsafe extern "C" fn(Engine, bool) -> Error>,
    pub observe_backings:
        Option<unsafe extern "C" fn(Engine, *const BackingObserver, usize) -> Error>,
    pub hook_watched: Option<unsafe extern "C" fn(Engine, usize, bool) -> Error>,
    pub watch: Option<unsafe extern "C" fn(Engine, u64, u64) -> Error>,
    pub insn_filter: Option<
        unsafe extern "C" fn(Engine, Option<InsnFilter>, Option<Restart>, *mut c_void) -> Error,
    >,
    pub retry_block: Option<unsafe extern "C" fn(Engine, u32) -> Error>,
    pub request_restart: Option<unsafe extern "C" fn(Engine) -> Error>,
    // uc_embedder_features bits (0 without the export): 1 = block hook size bit 31
    // is the Thumb state while a translation filter is installed.
    // Bit 4: stoppable Thumb instruction entries with synchronized ITSTATE.
    pub features: u32,
    // uc_embedder_accounting (features bit 2): the engine's accounting record.
    pub accounting: Option<unsafe extern "C" fn(Engine) -> *mut Accounting>,
    pub exit_counters: Option<unsafe extern "C" fn(Engine) -> *const u64>,
}

impl Extensions {
    /// Explicit library paths and direct native clients must acknowledge the
    /// same slow path as the Python selector; a path alone is not an opt-in.
    #[cfg(not(target_os = "emscripten"))]
    fn require_acceleration(&self) -> Result<(), String> {
        if std::env::var("CAMERA_FORCE_SLOW_UNICORN").as_deref() == Ok("1") {
            return Ok(());
        }
        let missing: Vec<_> = [
            (
                self.write_hook_pc_sync.is_some(),
                "uc_embedder_set_write_hook_pc_sync",
            ),
            (self.page_hooks.is_some(), "uc_embedder_set_page_hooks"),
            (
                self.observe_backings.is_some(),
                "uc_embedder_observe_backings",
            ),
            (self.hook_watched.is_some(), "uc_embedder_set_hook_watched"),
            (self.watch.is_some(), "uc_embedder_watch"),
            (self.insn_filter.is_some(), "uc_embedder_set_insn_filter"),
            (self.retry_block.is_some(), "uc_embedder_retry_block"),
            (
                self.request_restart.is_some(),
                "uc_embedder_request_restart",
            ),
            (self.accounting.is_some(), "uc_embedder_accounting"),
            (
                self.features & 3 == 3,
                "uc_embedder_features (Thumb block state and budgeted accounting)",
            ),
        ]
        .into_iter()
        .filter_map(|(present, name)| (!present).then_some(name))
        .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(format!(
            "The required Unicorn fork acceleration is missing: {}. \
             To knowingly use slower Unicorn instead, set CAMERA_FORCE_SLOW_UNICORN=1 \
             (I know the fork would be faster).",
            missing.join(", ")
        ))
    }
}
/// Unicorn's embedder budgeted block accounting record (uc_priv.h embedder_acct).
/// `enabled` selects exact ring accounting (1), coarse interactive accounting
/// without ring writes (2), or disabled accounting (0).
#[repr(C)]
pub struct Accounting {
    pub budget: i64,
    pub head: u64,
    pub ring: *mut u64,
    pub mask: u64,
    pub enabled: u64,
}
pub type InsnFilter = unsafe extern "C" fn(*mut c_void, u64, u32) -> c_int;
pub type Restart = unsafe extern "C" fn(*mut c_void, u64);
macro_rules! api {
    ($($field:ident : unsafe extern "C" fn($($args:tt)*) -> $result:ty),+ $(,)?) => {
        pub struct Api {
            $(pub $field: unsafe extern "C" fn($($args)*) -> $result,)+
            pub extensions: Extensions,
            #[cfg(not(target_os = "emscripten"))]
            _library: Library,
        }
        #[cfg(target_os = "emscripten")]
        unsafe extern "C" { $(fn $field($($args)*) -> $result;)+ }
        impl Api {
            #[cfg(not(target_os = "emscripten"))]
            pub unsafe fn load(path: &Path) -> Result<Self, String> {
                // SAFETY: caller supplies the checked Unicorn library. Its
                // handle is retained until every engine/context is destroyed.
                let library = unsafe { Library::new(path) }.map_err(|e| e.to_string())?;
                $(let $field = *unsafe { library.get::<unsafe extern "C" fn($($args)*) -> $result>(concat!(stringify!($field), "\0").as_bytes()) }
                    .map_err(|e| e.to_string())?;)+
                let extensions = Extensions {
                    write_hook_pc_sync: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, bool) -> Error>(b"uc_embedder_set_write_hook_pc_sync\0")
                    }.ok().map(|f| *f),
                    observe_backings: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, *const BackingObserver, usize) -> Error>(b"uc_embedder_observe_backings\0")
                    }.ok().map(|f| *f),
                    page_hooks: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, bool) -> Error>(b"uc_embedder_set_page_hooks\0")
                    }.ok().map(|f| *f),
                    hook_watched: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, usize, bool) -> Error>(b"uc_embedder_set_hook_watched\0")
                    }.ok().map(|f| *f),
                    watch: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, u64, u64) -> Error>(b"uc_embedder_watch\0")
                    }.ok().map(|f| *f),
                    insn_filter: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, Option<InsnFilter>, Option<Restart>, *mut c_void) -> Error>(b"uc_embedder_set_insn_filter\0")
                    }.ok().map(|f| *f),
                    retry_block: unsafe {
                        library.get::<unsafe extern "C" fn(Engine, u32) -> Error>(b"uc_embedder_retry_block\0")
                    }.ok().map(|f| *f),
                    request_restart: unsafe {
                        library.get::<unsafe extern "C" fn(Engine) -> Error>(b"uc_embedder_request_restart\0")
                    }.ok().map(|f| *f),
                    accounting: unsafe {
                        library.get::<unsafe extern "C" fn(Engine) -> *mut Accounting>(b"uc_embedder_accounting\0")
                    }.ok().map(|f| *f),
                    exit_counters: unsafe {
                        library.get::<unsafe extern "C" fn(Engine) -> *const u64>(b"uc_embedder_exit_counters\0")
                    }.ok().map(|f| *f),
                    features: unsafe {
                        library.get::<unsafe extern "C" fn() -> u32>(b"uc_embedder_features\0")
                    }.ok().map(|f| unsafe { f() }).unwrap_or(0),
                };
                extensions.require_acceleration()?;
                Ok(Self { $($field,)+ extensions, _library: library })
            }
            #[cfg(target_os = "emscripten")]
            pub unsafe fn load(_path: &Path) -> Result<Self, String> {
                Ok(Self::linked())
            }
            #[cfg(target_os = "emscripten")]
            pub fn linked() -> Self {
                Self { $($field,)+ extensions: linked::extensions() }
            }
        }
    }
}
api! {
    uc_version: unsafe extern "C" fn(_: *mut u32, _: *mut u32) -> u32,
    uc_open: unsafe extern "C" fn(_: c_int, _: c_int, _: *mut Engine) -> Error,
    uc_close: unsafe extern "C" fn(_: Engine) -> Error,
    uc_strerror: unsafe extern "C" fn(_: Error) -> *const c_char,
    uc_ctl: unsafe extern "C" fn(_: Engine, _: u32,  ...) -> Error,
    uc_reg_read: unsafe extern "C" fn(_: Engine, _: c_int, _: *mut c_void) -> Error,
    uc_reg_write: unsafe extern "C" fn(_: Engine, _: c_int, _: *const c_void) -> Error,
    uc_reg_read2: unsafe extern "C" fn(_: Engine, _: c_int, _: *mut c_void, _: *mut usize) -> Error,
    uc_reg_write2: unsafe extern "C" fn(_: Engine, _: c_int, _: *const c_void, _: *mut usize) -> Error,
    uc_mem_read: unsafe extern "C" fn(_: Engine, _: u64, _: *mut c_void, _: u64) -> Error,
    uc_mem_write: unsafe extern "C" fn(_: Engine, _: u64, _: *const c_void, _: u64) -> Error,
    uc_vmem_translate: unsafe extern "C" fn(_: Engine, _: u64, _: u32, _: *mut u64) -> Error,
    uc_mem_map: unsafe extern "C" fn(_: Engine, _: u64, _: u64, _: u32) -> Error,
    uc_mem_map_ptr: unsafe extern "C" fn(_: Engine, _: u64, _: u64, _: u32, _: *mut c_void) -> Error,
    uc_mmio_map: unsafe extern "C" fn(_: Engine, _: u64, _: u64, _: *mut c_void, _: *mut c_void, _: *mut c_void, _: *mut c_void) -> Error,
    uc_mem_unmap: unsafe extern "C" fn(_: Engine, _: u64, _: u64) -> Error,
    uc_mem_protect: unsafe extern "C" fn(_: Engine, _: u64, _: u64, _: u32) -> Error,
    uc_mem_regions: unsafe extern "C" fn(_: Engine, _: *mut *mut MemRegion, _: *mut u32) -> Error,
    uc_free: unsafe extern "C" fn(_: *mut c_void) -> Error,
    uc_emu_start: unsafe extern "C" fn(_: Engine, _: u64, _: u64, _: u64, _: usize) -> Error,
    uc_emu_stop: unsafe extern "C" fn(_: Engine) -> Error,
    uc_hook_add: unsafe extern "C" fn(_: Engine, _: *mut usize, _: c_int, _: *mut c_void, _: *mut c_void, _: u64, _: u64,  ...) -> Error,
    uc_hook_del: unsafe extern "C" fn(_: Engine, _: usize) -> Error,
    uc_context_alloc: unsafe extern "C" fn(_: Engine, _: *mut Context) -> Error,
    uc_context_reg_read2: unsafe extern "C" fn(_: Context, _: c_int, _: *mut c_void, _: *mut usize) -> Error,
    uc_context_reg_write2: unsafe extern "C" fn(_: Context, _: c_int, _: *const c_void, _: *mut usize) -> Error,
    uc_context_save: unsafe extern "C" fn(_: Engine, _: Context) -> Error,
    uc_context_restore: unsafe extern "C" fn(_: Engine, _: Context) -> Error,
    uc_context_free: unsafe extern "C" fn(_: Context) -> Error,
}

/// The Emscripten extension owns its pinned static Unicorn; no runtime loader.
#[cfg(target_os = "emscripten")]
mod linked {
    use super::*;
    unsafe extern "C" {
        fn uc_embedder_set_write_hook_pc_sync(engine: Engine, enabled: bool) -> Error;
        fn uc_embedder_set_page_hooks(engine: Engine, enabled: bool) -> Error;
        fn uc_embedder_observe_backings(
            engine: Engine,
            ranges: *const BackingObserver,
            count: usize,
        ) -> Error;
        fn uc_embedder_set_hook_watched(engine: Engine, hook: usize, watched: bool) -> Error;
        fn uc_embedder_watch(engine: Engine, begin: u64, size: u64) -> Error;
        fn uc_embedder_set_insn_filter(
            engine: Engine,
            filter: Option<InsnFilter>,
            restart: Option<Restart>,
            data: *mut c_void,
        ) -> Error;
        fn uc_embedder_retry_block(engine: Engine, count: u32) -> Error;
        fn uc_embedder_request_restart(engine: Engine) -> Error;
        fn uc_embedder_accounting(engine: Engine) -> *mut Accounting;
        fn uc_embedder_exit_counters(engine: Engine) -> *const u64;
        fn uc_embedder_features() -> u32;
    }
    pub fn extensions() -> Extensions {
        Extensions {
            write_hook_pc_sync: Some(uc_embedder_set_write_hook_pc_sync),
            page_hooks: Some(uc_embedder_set_page_hooks),
            observe_backings: Some(uc_embedder_observe_backings),
            hook_watched: Some(uc_embedder_set_hook_watched),
            watch: Some(uc_embedder_watch),
            insn_filter: Some(uc_embedder_set_insn_filter),
            retry_block: Some(uc_embedder_retry_block),
            request_restart: Some(uc_embedder_request_restart),
            accounting: Some(uc_embedder_accounting),
            exit_counters: Some(uc_embedder_exit_counters),
            features: unsafe { uc_embedder_features() },
        }
    }
}
