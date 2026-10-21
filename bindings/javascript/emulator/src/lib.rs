//! Single-worker C ABI. See camera_wasm.h for ownership and view lifetimes.
mod camera;
mod commands;
mod display;
mod fingerprint;
mod history;
mod jit;
mod lifecycle;
mod session;
use session::Session;
use std::{cell::RefCell, collections::BTreeMap};

#[repr(C)]
#[derive(Default)]
pub struct Bytes {
    pub data: *const u8,
    pub len: u32,
}
#[repr(C)]
#[derive(Default)]
pub struct Status {
    pub ticks: u64,
    pub instructions: u64,
    pub failed: u32,
}
#[repr(C)]
#[derive(Default)]
pub struct Frame {
    pub data: *const u8,
    pub len: u32,
    pub width: u32,
    pub height: u32,
    pub output: u32,
    pub sequence: u64,
}
#[derive(Default)]
struct Owner {
    next: u32,
    sessions: BTreeMap<u32, Session>,
    error: String,
    execution_policy: fujimod_emulator_cpu::ExecutionPolicy,
}
thread_local! { static OWNER: RefCell<Owner> = RefCell::new(Owner::default()); }

fn action(handle: u32, f: impl FnOnce(&mut Session) -> Result<(), String>) -> i32 {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        let Some(session) = owner.sessions.get_mut(&handle) else {
            owner.error = "invalid session handle".into();
            return -1;
        };
        match f(session) {
            Ok(()) => {
                session.error.clear();
                0
            }
            Err(e) => {
                session.error = e;
                -1
            }
        }
    })
}

/// Select the same board/CPU execution policy used by native clients.
/// # Safety
/// The UTF-8 policy span must be valid and immutable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_policy_setup(data: *const u8, len: u32) -> i32 {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        let result = (|| {
            if !owner.sessions.is_empty() {
                return Err("configure execution policy before creating sessions".to_owned());
            }
            let name =
                std::str::from_utf8(unsafe { bytes(data, len)? }).map_err(|e| e.to_string())?;
            owner.execution_policy = fujimod_emulator_cpu::ExecutionPolicy::parse(name)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                owner.error.clear();
                0
            }
            Err(error) => {
                owner.error = error;
                -1
            }
        }
    })
}

/// Allocate an upload buffer; pair with camera_free using the exact length.
#[unsafe(no_mangle)]
pub extern "C" fn camera_alloc(len: u32) -> *mut u8 {
    let Ok(layout) = std::alloc::Layout::from_size_align((len as usize).max(1), 8) else {
        return std::ptr::null_mut();
    };
    unsafe { std::alloc::alloc_zeroed(layout) }
}
/// # Safety
/// `data,len` must be an allocation returned by camera_alloc, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_free(data: *mut u8, len: u32) {
    if !data.is_null() {
        let layout = std::alloc::Layout::from_size_align((len as usize).max(1), 8)
            .expect("invalid camera allocation");
        unsafe { std::alloc::dealloc(data, layout) };
    }
}
unsafe fn bytes<'a>(data: *const u8, len: u32) -> Result<&'a [u8], String> {
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() || len > isize::MAX as u32 {
        return Err("invalid input buffer".into());
    }
    Ok(unsafe { std::slice::from_raw_parts(data, len as usize) })
}
/// # Safety
/// Input spans must be valid and immutable for this call. Inputs are copied.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_create(
    config: *const u8,
    config_len: u32,
    firmware: *const u8,
    firmware_len: u32,
) -> u32 {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        let result = (|| {
            let handle = owner
                .next
                .checked_add(1)
                .ok_or("session handle space exhausted")?;
            let session = Session::create(
                unsafe { bytes(config, config_len)? },
                unsafe { bytes(firmware, firmware_len)? },
                owner.execution_policy,
            )?;
            owner.sessions.insert(handle, session);
            owner.next = handle;
            Ok::<_, String>(handle)
        })();
        match result {
            Ok(handle) => {
                owner.error.clear();
                handle
            }
            Err(e) => {
                owner.error = e;
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn camera_destroy(handle: u32) -> i32 {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        if let Some(mut session) = owner.sessions.remove(&handle) {
            match session.core.board.finish() {
                Ok(()) => {
                    owner.error.clear();
                    0
                }
                Err(e) => {
                    owner.error = e;
                    -1
                }
            }
        } else {
            owner.error = "invalid session handle".into();
            -1
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn camera_run_until(handle: u32, tick: u64) -> i32 {
    let result = action(handle, |s| s.run(tick));
    #[cfg(target_os = "emscripten")]
    fujimod_emulator_cpu_unicorn::jit::emscripten::flush();
    result
}
/// # Safety
/// `key,key_len` must be a valid immutable UTF-8 span for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_input(
    handle: u32,
    key: *const u8,
    key_len: u32,
    pressed: u32,
    at: u64,
) -> i32 {
    action(handle, |s| {
        if pressed > 1 {
            return Err("pressed must be zero or one".into());
        }
        let key =
            std::str::from_utf8(unsafe { bytes(key, key_len)? }).map_err(|e| e.to_string())?;
        s.input(key, pressed != 0, at)
    })
}
/// # Safety
/// `out` must point to writable, aligned Status storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_status(handle: u32, out: *mut Status) -> i32 {
    if out.is_null() {
        return -1;
    }
    OWNER.with(|owner| {
        let owner = owner.borrow();
        let Some(s) = owner.sessions.get(&handle) else {
            return -1;
        };
        unsafe {
            out.write(Status {
                ticks: s.core.board.now(),
                instructions: s.core.board.instruction_counts().iter().sum(),
                failed: s.failed.into(),
            })
        };
        0
    })
}
/// # Safety
/// `out` must point to writable, aligned Frame storage. View is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_frame(handle: u32, output: u32, out: *mut Frame) -> i32 {
    if out.is_null() {
        return -1;
    }
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        let Some(s) = owner.sessions.get_mut(&handle) else {
            return -1;
        };
        unsafe { out.write(Frame::default()) };
        if let Err(error) = s.observe_frame(output as usize) {
            s.error = error;
            return -1;
        }
        let Some(f) = s.frames.get(&(output as usize)) else {
            return 1;
        };
        unsafe {
            out.write(Frame {
                data: f.rgba.as_ptr(),
                len: f.rgba.len() as u32,
                width: f.width,
                height: f.height,
                output,
                sequence: f.sequence,
            })
        };
        0
    })
}
/// # Safety
/// `out` must point to writable, aligned Bytes storage. View is borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_error(handle: u32, out: *mut Bytes) -> i32 {
    if out.is_null() {
        return -1;
    }
    OWNER.with(|owner| {
        let owner = owner.borrow();
        let error = if handle == 0 {
            &owner.error
        } else if let Some(s) = owner.sessions.get(&handle) {
            &s.error
        } else {
            return -1;
        };
        unsafe {
            out.write(Bytes {
                data: error.as_ptr(),
                len: error.len() as u32,
            })
        };
        0
    })
}

#[repr(C)]
#[derive(Default)]
pub struct ResultView {
    pub json: Bytes,
    pub bytes: Bytes,
}

/// # Safety
/// Input spans must remain valid and immutable during construction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_session_create(
    config: *const u8,
    config_len: u32,
    dat: *const u8,
    dat_len: u32,
) -> u32 {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        let result = (|| {
            let handle = owner
                .next
                .checked_add(1)
                .ok_or("session handle space exhausted")?;
            let session = Session::create_camera(
                unsafe { bytes(config, config_len)? },
                unsafe { bytes(dat, dat_len)? },
                owner.execution_policy,
            )?;
            owner.sessions.insert(handle, session);
            owner.next = handle;
            Ok::<_, String>(handle)
        })();
        match result {
            Ok(handle) => {
                owner.error.clear();
                handle
            }
            Err(e) => {
                owner.error = e;
                0
            }
        }
    })
}
/// # Safety
/// Inputs must be valid immutable spans; out must point to aligned writable storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_command(
    handle: u32,
    json: *const u8,
    json_len: u32,
    upload: *const u8,
    upload_len: u32,
    out: *mut ResultView,
) -> i32 {
    if out.is_null() {
        return -1;
    }
    unsafe { out.write(ResultView::default()) };
    action(handle, |s| {
        s.command(handle, unsafe { bytes(json, json_len)? }, unsafe {
            bytes(upload, upload_len)?
        })?;
        unsafe {
            out.write(ResultView {
                json: Bytes {
                    data: s.result_json.as_ptr(),
                    len: s.result_json.len() as u32,
                },
                bytes: Bytes {
                    data: s.result_bytes.as_ptr(),
                    len: s.result_bytes.len() as u32,
                },
            })
        };
        Ok(())
    })
}
/// # Safety
/// out must point to aligned writable Bytes storage. The returned span is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_scene_buffer(
    handle: u32,
    width: u32,
    height: u32,
    out: *mut Bytes,
) -> i32 {
    if out.is_null() {
        return -1;
    }
    unsafe { out.write(Bytes::default()) };
    action(handle, |s| {
        s.reserve_scene(width, height)?;
        unsafe {
            out.write(Bytes {
                data: s.scene_upload.as_ptr(),
                len: s.scene_upload.len() as u32,
            })
        };
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn camera_scene_commit(handle: u32, enabled: u32) -> i32 {
    action(handle, |s| s.commit_scene(enabled))
}
