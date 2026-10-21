//! Transport configuration and borrowed diagnostic bytes for the Rust tier.
use super::{Bytes, OWNER};
use fujimod_emulator_cpu_unicorn::jit::Options;
use std::cell::RefCell;
thread_local! {static RESULT:RefCell<Vec<u8>>=const {RefCell::new(Vec::new())};}

fn configuration(bytes: &[u8]) -> Result<Option<Options>, String> {
    if bytes.is_empty() {
        return Ok(Some(Options::default()));
    }
    if bytes.len() > 4096 {
        return Err("tcgJit configuration exceeds 4096 bytes".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("tcgJit: {e}"))?;
    match value {
        serde_json::Value::Bool(false) => Ok(None),
        serde_json::Value::Bool(true) => Ok(Some(Options::default())),
        serde_json::Value::Object(_) => {
            let options: Options =
                serde_json::from_value(value).map_err(|e| format!("tcgJit: {e}"))?;
            Ok(Some(options.validate()?))
        }
        _ => Err("tcgJit must be false, true or an options object".into()),
    }
}
fn error(message: String) -> i32 {
    OWNER.with(|owner| owner.borrow_mut().error = message);
    -1
}
/// # Safety
/// Input is a borrowed JSON span. An empty span selects Rust defaults.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_jit_setup(pointer: *const u8, length: u32) -> i32 {
    if pointer.is_null() && length != 0 {
        return error("null tcgJit configuration".into());
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(pointer, length as usize) }
    };
    let result = configuration(bytes).and_then(|options| {
        #[cfg(target_os = "emscripten")]
        {
            fujimod_emulator_cpu_unicorn::jit::emscripten::setup(options)
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            let _ = options;
            Err("the generated-code tier requires Emscripten".into())
        }
    });
    match result {
        Ok(()) => 0,
        Err(message) => error(message),
    }
}
/// # Safety
/// `out` is writable Bytes storage; the result is borrowed until the next call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn camera_jit_stats(out: *mut Bytes) -> i32 {
    if out.is_null() {
        return error("null tcgJit statistics output".into());
    }
    #[cfg(target_os = "emscripten")]
    let result = match fujimod_emulator_cpu_unicorn::jit::emscripten::statistics() {
        Some(stats) => serde_json::to_vec(&stats),
        None => serde_json::to_vec(&serde_json::json!({"enabled":false})),
    };
    #[cfg(not(target_os = "emscripten"))]
    let result = serde_json::to_vec(&serde_json::json!({"enabled":false}));
    match result {
        Err(e) => error(e.to_string()),
        Ok(bytes) => RESULT.with(|result| {
            let mut result = result.borrow_mut();
            *result = bytes;
            unsafe {
                out.write(Bytes {
                    data: result.as_ptr(),
                    len: result.len() as u32,
                })
            };
            0
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rust_owns_default_selection_types_and_option_bounds() {
        assert_eq!(configuration(b"").unwrap().unwrap().max_functions, 32768);
        assert!(configuration(b"false").unwrap().is_none());
        assert_eq!(configuration(b"true").unwrap().unwrap().threshold, 32);
        assert_eq!(
            configuration(br#"{"threshold":2,"batchSize":1}"#)
                .unwrap()
                .unwrap()
                .threshold,
            2
        );
        for bytes in [
            &b"null"[..],
            b"0",
            br#""true""#,
            b"[]",
            br#"{"threshold":0}"#,
            br#"{"batchSize":1.5}"#,
            br#"{"unknown":1}"#,
        ] {
            assert!(configuration(bytes).is_err());
        }
    }
}
