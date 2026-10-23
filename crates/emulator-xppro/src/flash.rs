//! X-Processor Pro flash controller and its memory-mapped read aperture.
//!
//! The controller's sixteen configuration words retain writes; no status bit,
//! identification, erase or program operation is modeled. The aperture reads
//! the attached flash contents, erased (0xff) past the supplied image.
use fujimod_emulator_devices::{Cx, Device};
use serde_json::{Value, json};
use std::sync::Arc;

/// Controller register window size in bytes.
pub const REGISTERS: u64 = 0x40;

/// Read-only flash contents behind a physical window.
pub struct Aperture {
    base: u64,
    size: u64,
    image: Vec<u8>,
}
impl Aperture {
    pub fn new(base: u64, size: u64, image: Vec<u8>) -> Result<Self, String> {
        if base & 0xfff != 0
            || size == 0
            || image.len() as u64 > size
            || base.checked_add(size).is_none_or(|end| end > 1 << 32)
        {
            return Err("Invalid X-Processor Pro flash aperture".into());
        }
        Ok(Self { base, size, image })
    }
    pub fn base(&self) -> u64 {
        self.base
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn contains(&self, address: u64, len: usize) -> bool {
        address >= self.base
            && address
                .checked_add(len as u64)
                .is_some_and(|end| end <= self.base + self.size)
    }
    pub fn read(&self, address: u64, len: usize) -> Result<Vec<u8>, String> {
        if !self.contains(address, len) {
            return Err("Flash read outside the aperture".into());
        }
        let start = (address - self.base) as usize;
        let mut out = vec![0xff; len];
        if start < self.image.len() {
            let n = len.min(self.image.len() - start);
            out[..n].copy_from_slice(&self.image[start..start + n]);
        }
        Ok(out)
    }
}

pub struct FlashController {
    base: u64,
    aperture: Arc<Aperture>,
    registers: [u32; (REGISTERS / 4) as usize],
}
impl FlashController {
    pub fn new(base: u64, aperture: Arc<Aperture>) -> Result<Self, String> {
        if base & 0xfff != 0 || aperture.contains(base, REGISTERS as usize) {
            return Err("Invalid X-Processor Pro flash controller base".into());
        }
        Ok(Self {
            base,
            aperture,
            registers: [0; (REGISTERS / 4) as usize],
        })
    }
}
impl Device for FlashController {
    fn name(&self) -> &'static str {
        "xppro-flash"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        if self.aperture.contains(address, size as usize) {
            return match self.aperture.read(address, size as usize) {
                Ok(b) => b.iter().rev().fold(0, |v, &x| v << 8 | u64::from(x)),
                Err(e) => {
                    cx.fail(e);
                    0
                }
            };
        }
        if size != 4 || address & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        u64::from(self.registers[((address - self.base) / 4) as usize])
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        if self.aperture.contains(address, size as usize) {
            cx.fail("X-Processor Pro flash aperture writes are not modeled");
        } else if size != 4 || address & 3 != 0 {
            cx.unmodeled();
        } else {
            self.registers[((address - self.base) / 4) as usize] = value as u32;
        }
    }
    fn pollable(&self, address: u64, size: u8) -> bool {
        !self.aperture.contains(address, size as usize)
    }
    fn save(&self) -> Value {
        json!({ "registers": self.registers })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        self.registers =
            serde_json::from_value(value["registers"].clone()).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::{TestHost, read, write};

    #[test]
    fn aperture_reads_the_image_then_erased_bytes_and_registers_retain_writes() {
        let aperture =
            Arc::new(Aperture::new(0xf0000000, 0x08000000, vec![1, 2, 3, 4, 5]).unwrap());
        let mut flash = FlashController::new(0xfff14000, aperture.clone()).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut flash, &mut h, 0xf0000000), 0x04030201);
        assert_eq!(read(&mut flash, &mut h, 0xf0000004), 0xffffff05);
        write(&mut flash, &mut h, 0xfff14008, 0x700);
        assert_eq!(read(&mut flash, &mut h, 0xfff14008), 0x700);
        let saved = flash.save();
        let mut resumed = FlashController::new(0xfff14000, aperture).unwrap();
        resumed.restore(&saved).unwrap();
        assert_eq!(resumed.save(), saved);
        let mut cx = h.cx();
        flash.write(&mut cx, 0xf0000000, 4, 0);
        assert!(cx.refusal().is_some());
        assert!(Aperture::new(0xf0000000, 4, vec![0; 5]).is_err());
    }
}
