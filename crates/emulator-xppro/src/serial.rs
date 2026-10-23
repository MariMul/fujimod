//! X-Processor Pro serial port status law, without a peer.
//!
//! Status +0x28 holds flags; a write to +0x2c clears the written status bits.
//! A send word written to +0x60 completes at once and sets done bit 2. Bit 3
//! reads set because the send queue never fills. The receive word +0x48
//! reads zero. Other words retain writes. No peer, timing or IRQ is modeled;
//! this is a provisional law for discovery.
use fujimod_emulator_devices::{Cx, Device};
use serde_json::{Value, json};

/// Register window size in bytes.
pub const REGISTERS: u64 = 0x100;
const STATUS: usize = 0x28 / 4;
const CLEAR: u64 = 0x2c;
const RECEIVE: u64 = 0x48;
const SEND: u64 = 0x60;
const DONE: u32 = 1 << 2;
const ROOM: u32 = 1 << 3;

pub struct SerialPort {
    base: u64,
    registers: Vec<u32>,
}
impl SerialPort {
    pub fn new(base: u64) -> Result<Self, String> {
        if base & 0xfff != 0 {
            return Err("Invalid X-Processor Pro serial port base".into());
        }
        Ok(Self {
            base,
            registers: vec![0; (REGISTERS / 4) as usize],
        })
    }
}
impl Device for SerialPort {
    fn name(&self) -> &'static str {
        "xppro-serial"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let o = address - self.base;
        if size != 4 || o & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        u64::from(match o {
            RECEIVE => 0,
            0x28 => self.registers[STATUS] | ROOM,
            _ => self.registers[(o / 4) as usize],
        })
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let o = address - self.base;
        if size != 4 || o & 3 != 0 || o == 0x28 {
            cx.unmodeled();
            return;
        }
        match o {
            CLEAR => self.registers[STATUS] &= !(value as u32),
            SEND => {
                self.registers[(SEND / 4) as usize] = value as u32;
                self.registers[STATUS] |= DONE;
            }
            _ => self.registers[(o / 4) as usize] = value as u32,
        }
    }
    fn pollable(&self, address: u64, _size: u8) -> bool {
        address - self.base == 0x28
    }
    fn save(&self) -> Value {
        json!({ "registers": self.registers })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let registers: Vec<u32> =
            serde_json::from_value(value["registers"].clone()).map_err(|e| e.to_string())?;
        if registers.len() != self.registers.len() {
            return Err("X-Processor Pro serial checkpoint size differs".into());
        }
        self.registers = registers;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::{TestHost, read, write};

    #[test]
    fn send_sets_done_and_clear_drops_written_status_bits() {
        let base = 0xfff2a000;
        let mut port = SerialPort::new(base).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut port, &mut h, base + 0x28), u64::from(ROOM));
        write(&mut port, &mut h, base + 0x2c, 0x7f);
        assert_eq!(read(&mut port, &mut h, base + 0x48), 0);
        write(&mut port, &mut h, base + 0x60, 0xe800);
        assert_eq!(read(&mut port, &mut h, base + 0x28), u64::from(ROOM | DONE));
        write(&mut port, &mut h, base + 0x2c, DONE);
        assert_eq!(read(&mut port, &mut h, base + 0x28), u64::from(ROOM));
        write(&mut port, &mut h, base + 0x14, 0xa);
        assert_eq!(read(&mut port, &mut h, base + 0x14), 0xa);
        let mut cx = h.cx();
        port.write(&mut cx, base + 0x28, 4, 0);
        assert!(cx.is_unmodeled());
    }
}
