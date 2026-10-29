//! X-Processor Pro job engine at 0xfff88000, from GFX 50R ISR 0xe852a0.
//!
//! Every aligned word is retained. A +0x1c write with bit 0 set starts a job.
//! After `ticks` the job sets +0x34 bit 26, the job-done bit the ISR handles.
//! The IRQ is level: high while +0x34 bits 16-31 are set.
//! Writing 0 to +0x1c clears bit 26, as the ISR does.
//! The job moves no data; its transfer law is unrecovered.
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const SIZE: u64 = 0x2000;
const DONE: u32 = 1 << 26;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct State {
    words: BTreeMap<u64, u32>,
    due: Option<u64>,
}

pub struct JobEngine {
    base: u64,
    irq: u32,
    ticks: u64,
    state: State,
}

impl JobEngine {
    pub fn new(base: u64, irq: u32, ticks: u64) -> Result<Self, String> {
        if base & 0xfff != 0 || base.checked_add(SIZE).is_none() || irq < 32 || ticks == 0 {
            return Err("Invalid X-Processor Pro job engine wiring".into());
        }
        Ok(Self { base, irq, ticks, state: State::default() })
    }
    pub fn window(&self) -> (u64, u64) {
        (self.base, self.base + SIZE)
    }
    fn word(&self, offset: u64) -> u32 {
        self.state.words.get(&offset).copied().unwrap_or(0)
    }
    fn update_irq(&self, cx: &mut Cx<'_>) {
        cx.set_irq(self.irq, self.word(0x34) >> 16 != 0);
    }
}

impl Device for JobEngine {
    fn name(&self) -> &'static str {
        "xpp-job-engine"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let offset = address - self.base;
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        self.word(offset).into()
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let offset = address - self.base;
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return;
        }
        let value = value as u32;
        self.state.words.insert(offset, value);
        if offset == 0x1c {
            if value & 1 != 0 {
                let Some(due) = cx.now().checked_add(self.ticks) else {
                    cx.fail("X-Processor Pro job deadline overflow");
                    return;
                };
                self.state.due = Some(due);
            } else if value == 0 {
                let status = self.word(0x34) & !DONE;
                self.state.words.insert(0x34, status);
                self.update_irq(cx);
            }
        }
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self.state.due.is_some_and(|due| due <= cx.now()) {
            self.state.due = None;
            let status = self.word(0x34) | DONE;
            self.state.words.insert(0x34, status);
            self.update_irq(cx);
        }
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.state.due
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.state).unwrap()
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let state: State = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if state.words.keys().any(|o| o & 3 != 0 || *o >= SIZE) {
            return Err("Invalid X-Processor Pro job engine checkpoint".into());
        }
        self.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::{TestHost, advance, read, write};

    #[test]
    fn started_job_sets_done_and_level_irq_until_cleared() {
        let mut h = TestHost::new();
        let mut d = JobEngine::new(0xfff88000, 92, 10).unwrap();
        write(&mut d, &mut h, 0xfff88010, 0x044ad000);
        write(&mut d, &mut h, 0xfff8801c, 0x0f100201);
        assert_eq!(d.next_deadline(0), Some(10));
        advance(&mut d, &mut h, 10);
        assert_eq!(read(&mut d, &mut h, 0xfff88034), DONE as u64);
        assert_eq!(read(&mut d, &mut h, 0xfff88010), 0x044ad000);
        write(&mut d, &mut h, 0xfff8801c, 0);
        assert_eq!(read(&mut d, &mut h, 0xfff88034), 0);
        assert_eq!(d.next_deadline(10), None);
    }
}
