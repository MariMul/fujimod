//! Shared power-peer seconds counter and timebase. Register laws live with
//! each peer protocol.
use serde::{Deserialize, Serialize};

// Field names and ordering preserve the three-byte peer's checkpoint encoding.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counter {
    pub(crate) seconds: u32,
    pub(crate) anchor: u64,
    pub(crate) running: bool,
    pub(crate) ticks_per_second: u64,
}
impl Counter {
    pub fn new(seconds: u32, running: bool, ticks_per_second: u64) -> Result<Self, String> {
        if !(1..=1_000_000_000).contains(&ticks_per_second) {
            return Err("Invalid RTC timebase".into());
        }
        Ok(Self {
            seconds,
            anchor: 0,
            running,
            ticks_per_second,
        })
    }
    pub fn counter(&self, tick: u64) -> Result<u32, String> {
        let elapsed = tick
            .checked_sub(self.anchor)
            .ok_or("Power RTC clock moved backwards")?;
        Ok(self.seconds.wrapping_add(if self.running {
            (elapsed / self.ticks_per_second) as u32
        } else {
            0
        }))
    }
    pub(crate) fn set_running(&mut self, running: bool, tick: u64) -> Result<(), String> {
        if self.running != running {
            self.seconds = self.counter(tick)?;
            self.anchor = tick;
            self.running = running;
        }
        Ok(())
    }
    pub(crate) fn load(&mut self, seconds: u32, tick: u64) {
        self.seconds = seconds;
        self.anchor = tick;
    }
}
