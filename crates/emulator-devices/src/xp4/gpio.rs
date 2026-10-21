//! XP4 sixteen masked GPIO banks (10713d8/107162c). External levels are inputs.
use crate::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
pub const BASE: u64 = 0xfff41000;
#[derive(Clone, Serialize, Deserialize)]
pub struct Pins {
    regs: [[u32; 8]; 16],
    inputs: [u16; 16],
    changes: [[u64; 16]; 16],
}
pub type SharedPins = Arc<Mutex<Pins>>;
impl Pins {
    pub fn new(inputs: [u16; 16]) -> SharedPins {
        Arc::new(Mutex::new(Self {
            regs: [[0; 8]; 16],
            inputs,
            changes: [[0; 16]; 16],
        }))
    }
    pub fn output(&self, port: usize, pin: usize) -> (bool, u64) {
        (
            self.regs[port][0] & (1 << pin) != 0,
            self.changes[port][pin],
        )
    }
    pub fn set_input(&mut self, port: usize, pin: usize, high: bool) {
        if high {
            self.inputs[port] |= 1 << pin;
        } else {
            self.inputs[port] &= !(1 << pin);
        }
    }
}
pub struct Gpio {
    pins: SharedPins,
}
impl Gpio {
    pub fn new(pins: SharedPins) -> Self {
        Self { pins }
    }
}
impl Device for Gpio {
    fn name(&self) -> &'static str {
        "xp4-gpio"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let offset = address.wrapping_sub(BASE) as usize;
        let p = offset / 64;
        let r = offset % 64 / 4;
        if size != 4 || offset & 3 != 0 || p >= 16 || r >= 8 {
            cx.unmodeled();
            return 0;
        }
        let pins = self.pins.lock().unwrap();
        if r == 5 {
            let output = pins.regs[p][2] & 0xffff;
            u64::from((u32::from(pins.inputs[p]) & !output) | (pins.regs[p][1] & output))
        } else {
            u64::from(pins.regs[p][r])
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let offset = address.wrapping_sub(BASE) as usize;
        let p = offset / 64;
        let r = offset % 64 / 4;
        if size != 4 || offset & 3 != 0 || p >= 16 || r >= 8 || r == 5 {
            cx.unmodeled();
            return;
        }
        let mut pins = self.pins.lock().unwrap();
        let old = pins.regs[p][r];
        let v = value as u32;
        pins.regs[p][r] = if r >= 6 {
            v
        } else {
            let mask = v >> 16;
            (old & !mask) | (v & mask)
        };
        if r == 0 {
            for bit in 0..16 {
                if (old ^ pins.regs[p][r]) & (1 << bit) != 0 {
                    pins.changes[p][bit] += 1;
                }
            }
        }
    }
    fn save(&self) -> Value {
        serde_json::to_value(&*self.pins.lock().unwrap()).unwrap()
    }
    fn restore(&mut self, v: &Value) -> Result<(), String> {
        *self.pins.lock().unwrap() =
            serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        Ok(())
    }
}
