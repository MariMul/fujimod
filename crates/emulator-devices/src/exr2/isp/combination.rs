//! FFF480 reset/idle configuration. Active image combination is unsupported:
//! no DMA or arithmetic law is assumed by this register model.
use crate::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub const BASE: u64 = 0xfff48000;
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub reset_ticks: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    registers: BTreeMap<u16, u32>,
    reset_due: Option<u64>,
    resets: u64,
}
pub struct Combination {
    options: Options,
    state: State,
}
impl Combination {
    pub fn new(options: Options) -> Result<Self, String> {
        if options.reset_ticks == 0 {
            return Err("image combination requires a positive reset delay".into());
        }
        Ok(Self {
            options,
            state: State {
                registers: BTreeMap::from([(0, 1), (0xc4, 0x331)]),
                reset_due: None,
                resets: 0,
            },
        })
    }
    fn offset(address: u64, size: u8) -> Option<u16> {
        let o = address.checked_sub(BASE)?;
        (size == 4
            && o & 3 == 0
            && (matches!(o, 0 | 4 | 0x60 | 0x64 | 0xc0 | 0xc4 | 0xc8 | 0xd0 | 0xd4)
                || (0x10..0x5c).contains(&o)
                || (0x70..0xbc).contains(&o)
                || (0x100..0x500).contains(&o)))
        .then_some(o as u16)
    }
    fn word(&self, o: u16) -> u32 {
        self.state.registers.get(&o).copied().unwrap_or(0)
    }
}
impl Combination {
    fn validate_state(&self, state: State) -> Result<State, String> {
        if state.registers.iter().any(|(o, v)| {
            Self::offset(BASE + u64::from(*o), 4).is_none()
                || match o {
                    0 => *v > 1,
                    4 => *v & !0x3377 != 0 || *v & 7 > 4,
                    0xc0 | 0xc4 | 0xc8 => *v & !0x331 != 0,
                    _ => false,
                }
        }) || state.reset_due.is_some()
            && (state.resets == u64::MAX || state.registers.get(&0) != Some(&0))
        {
            return Err("invalid image combination register/deadline state".into());
        }

        Ok(state)
    }
}
impl Device for Combination {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        crate::checkpoint::Checkpoint::encode(self.name(), &self.state)
    }
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        self.validate_state(saved.decode(self.name())?).map(|_| ())
    }
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let state = self.validate_state(saved.decode(self.name())?)?;
        self.state = state;
        Ok(())
    }
    fn fingerprint(&self) -> Result<Value, String> {
        Ok(json!({"options":self.options,"state":self.state}))
    }

    fn name(&self) -> &'static str {
        "image-combination"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let Some(o) = Self::offset(address, size) else {
            cx.unmodeled();
            return 0;
        };
        self.word(o).into()
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let Some(o) = Self::offset(address, size) else {
            cx.unmodeled();
            return;
        };
        let Ok(value) = u32::try_from(value) else {
            cx.fail("image combination requires a 32-bit word");
            return;
        };
        match o {
            0 => {
                if value != 0 {
                    cx.fail("image combination active processing has no raster/DMA law");
                    return;
                }
                if self.state.resets == u64::MAX {
                    cx.fail("image combination reset count exhausted");
                    return;
                }
                let Some(due) = cx.now().checked_add(self.options.reset_ticks) else {
                    cx.fail("image combination reset deadline overflow");
                    return;
                };
                self.state.reset_due = Some(due);
            }
            0xc0 => {
                cx.fail("image combination status is read-only");
                return;
            }
            0xc4 | 0xc8 => {
                if value & !0x331 != 0 {
                    cx.fail("unknown image combination interrupt bits");
                    return;
                }
                if o == 0xc8 {
                    self.state.registers.insert(0xc0, self.word(0xc0) & !value);
                    return;
                }
            }
            4 => {
                if value & !0x3377 != 0 || value & 7 > 4 {
                    cx.fail("unknown image combination mode/selectors");
                    return;
                }
            }
            _ => {}
        }
        self.state.registers.insert(o, value);
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        self.state.reset_due
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        if self.state.reset_due.is_some_and(|due| due <= cx.now()) {
            self.state.registers.insert(0, 1);
            self.state.reset_due = None;
            self.state.resets += 1;
        }
    }
    fn pollable(&self, address: u64, size: u8) -> bool {
        Self::offset(address, size).is_some()
    }
    fn save(&self) -> Value {
        json!(self.state)
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let state: State = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        let state = self.validate_state(state)?;
        self.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exr2::isp::tests::support::{TestHost, advance, read, write};
    #[test]
    fn reset_roundtrip_and_early_ticks_preserve_exact_deadline_without_pixels() {
        let mut host = TestHost::new();
        let mut d = Combination::new(Options { reset_ticks: 100 }).unwrap();
        write(&mut d, &mut host, BASE, 0);
        let saved = d.save();
        let checkpoint = d.checkpoint().unwrap();
        advance(&mut d, &mut host, 99);
        assert_eq!(d.save(), saved);
        assert_eq!(read(&mut d, &mut host, BASE), 0);
        let mut restored = Combination::new(Options { reset_ticks: 100 }).unwrap();
        restored.restore_checkpoint(&checkpoint).unwrap();
        advance(&mut restored, &mut host, 100);
        advance(&mut d, &mut host, 100);
        assert_eq!(restored.save(), d.save());
        assert_eq!(read(&mut d, &mut host, BASE), 1);
        assert!(host.effects.is_empty());
        let before = d.save();
        let mut cx = host.cx();
        d.write(&mut cx, BASE, 4, 1);
        assert!(cx.take_refusal().unwrap().contains("raster/DMA law"));
        assert_eq!(d.save(), before);
    }
}
