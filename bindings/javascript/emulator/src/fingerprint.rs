//! Compact complete-machine identity without an encoded JSON allocation.
use sha2::{Digest, Sha256};
use std::io::{self, Write};

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn session(
    session: &fujimod_emulator::session::Session,
    powered_on: bool,
) -> Result<serde_json::Value, String> {
    let fingerprint = serde_json::json!({"poweredOn":powered_on,"machine":session.fingerprint()?});
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, &fingerprint).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"schema":1,"sha256":format!("{:x}",writer.0.finalize())}))
}
