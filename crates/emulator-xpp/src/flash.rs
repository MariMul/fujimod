//! X-Processor Pro flash controller and its memory-mapped read aperture.
//!
//! The controller's sixteen configuration words retain writes; no status bit,
//! identification, erase or program operation is modeled. The aperture reads
//! the attached flash contents, erased (0xff) past the supplied image.
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Controller register window size in bytes.
pub const REGISTERS: u64 = 0x40;

/// Passive union of every byte read through the aperture, including DMA reads.
/// Ranges are medium offsets with exclusive ends, independent of guest code.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadAudit {
    pub bytes: u64,
    pub ranges: Vec<(u64, u64)>,
}
impl ReadAudit {
    fn record(&mut self, mut start: u64, mut end: u64) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(end - start)
            .ok_or("X-Processor Pro flash read counter overflow")?;
        if start == end {
            return Ok(());
        }
        let first = self.ranges.partition_point(|&(_, hi)| hi < start);
        let mut last = first;
        while last < self.ranges.len() && self.ranges[last].0 <= end {
            start = start.min(self.ranges[last].0);
            end = end.max(self.ranges[last].1);
            last += 1;
        }
        self.ranges.splice(first..last, [(start, end)]);
        Ok(())
    }
}

/// Read-only flash contents behind a physical window.
pub struct Aperture {
    base: u64,
    size: u64,
    image: Vec<u8>,
    reads: Mutex<ReadAudit>,
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
        Ok(Self {
            base,
            size,
            image,
            reads: Mutex::new(ReadAudit::default()),
        })
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
        self.reads
            .lock()
            .unwrap()
            .record(start as u64, start as u64 + len as u64)?;
        Ok(out)
    }
    pub fn read_audit(&self) -> ReadAudit {
        self.reads.lock().unwrap().clone()
    }
    fn restore_audit(&self, audit: ReadAudit) -> Result<(), String> {
        let mut previous = None;
        let mut unique = 0u64;
        for &(start, end) in &audit.ranges {
            if start >= end || end > self.size || previous.is_some_and(|p| p >= start) {
                return Err("Invalid X-Processor Pro flash read ranges".into());
            }
            unique += end - start;
            previous = Some(end);
        }
        if unique > audit.bytes || (audit.ranges.is_empty() && audit.bytes != 0) {
            return Err("Invalid X-Processor Pro flash read count".into());
        }
        *self.reads.lock().unwrap() = audit;
        Ok(())
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
        "xpp-flash"
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
        json!({ "registers": self.registers, "aperture_reads": self.aperture.read_audit() })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let registers =
            serde_json::from_value(value["registers"].clone()).map_err(|e| e.to_string())?;
        let audit =
            serde_json::from_value(value["aperture_reads"].clone()).map_err(|e| e.to_string())?;
        self.aperture.restore_audit(audit)?;
        self.registers = registers;
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

    #[test]
    fn cpu_and_shared_dma_reads_share_a_checkpointed_passive_audit() {
        let aperture = Arc::new(Aperture::new(0xf0000000, 0x1000, vec![0; 16]).unwrap());
        let mut flash = FlashController::new(0xfff14000, aperture.clone()).unwrap();
        let mut h = TestHost::new();
        read(&mut flash, &mut h, 0xf0000000);
        aperture.read(0xf0000008, 8).unwrap();
        aperture.read(0xf0000004, 8).unwrap();
        aperture.read(0xf0000010, 4).unwrap(); // erased backing still counts
        aperture.read(0xf0000100, 4).unwrap();
        let audit = aperture.read_audit();
        assert_eq!(audit.bytes, 28);
        assert_eq!(audit.ranges, [(0, 20), (0x100, 0x104)]);
        let saved = flash.save();
        aperture.read(0xf0000000, 4).unwrap();
        flash.restore(&saved).unwrap();
        assert_eq!(aperture.read_audit().bytes, 28);
        let mut bad = saved.clone();
        bad["aperture_reads"]["ranges"] = json!([[0, 0x1001]]);
        assert!(flash.restore(&bad).is_err());
        assert_eq!(flash.save(), saved);
        assert!(aperture.read(0xefffffff, 1).is_err());
        assert_eq!(flash.save(), saved);
    }
}
