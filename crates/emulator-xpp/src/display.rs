//! X-Processor Pro display unit control pages, from GFX 50R 0xcf6adc.
//!
//! Each bank retains every aligned word. A +0 read returns only bit 16, a
//! status: a written 1-to-0 change of bit 16 sets it after `ticks`, and a
//! write with bit 16 set clears it. A camera read of +0 while the display ran
//! returned 0, so +0 reads no stored bits. The delay is declared, not measured.
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const STATUS: u32 = 1 << 16;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct Bank {
    words: BTreeMap<u64, u32>,
    status: bool,
    due: Option<u64>,
}

pub struct DisplayControl {
    bases: [u64; 2],
    ticks: u64,
    banks: [Bank; 2],
}

impl DisplayControl {
    pub fn new(bases: [u64; 2], ticks: u64) -> Result<Self, String> {
        if bases.iter().any(|b| b & 0xfff != 0 || b.checked_add(0x1000).is_none())
            || bases[0] == bases[1]
            || ticks == 0
        {
            return Err("Invalid X-Processor Pro display control wiring".into());
        }
        Ok(Self { bases, ticks, banks: Default::default() })
    }
    pub fn windows(&self) -> [(u64, u64); 2] {
        self.bases.map(|b| (b, b + 0x1000))
    }
    fn locate(&self, address: u64) -> (usize, u64) {
        let bank = if (self.bases[0]..self.bases[0] + 0x1000).contains(&address) { 0 } else { 1 };
        (bank, address - self.bases[bank])
    }
}

impl Device for DisplayControl {
    fn name(&self) -> &'static str {
        "xpp-display-control"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let (bank, offset) = self.locate(address);
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        let bank = &self.banks[bank];
        if offset == 0 {
            return if bank.status { STATUS.into() } else { 0 };
        }
        bank.words.get(&offset).copied().unwrap_or(0).into()
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let (index, offset) = self.locate(address);
        if size != 4 || offset & 3 != 0 {
            cx.unmodeled();
            return;
        }
        let value = value as u32;
        let now = cx.now();
        let bank = &mut self.banks[index];
        if offset == 0 {
            let before = bank.words.get(&0).copied().unwrap_or(0);
            if value & STATUS != 0 {
                bank.status = false;
                bank.due = None;
            } else if before & STATUS != 0 {
                let Some(due) = now.checked_add(self.ticks) else {
                    cx.fail("X-Processor Pro display control deadline overflow");
                    return;
                };
                bank.due = Some(due);
            }
        }
        bank.words.insert(offset, value);
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        for bank in &mut self.banks {
            if bank.due.is_some_and(|due| due <= cx.now()) {
                bank.due = None;
                bank.status = true;
            }
        }
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.banks.iter().filter_map(|b| b.due).min()
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.banks).unwrap()
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let banks: [Bank; 2] = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if banks.iter().any(|b| b.words.keys().any(|o| o & 3 != 0 || *o >= 0x1000)) {
            return Err("Invalid X-Processor Pro display control checkpoint".into());
        }
        self.banks = banks;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::{TestHost, advance, read, write};

    #[test]
    fn clearing_bit16_reports_status_after_delay_until_set_again() {
        let mut h = TestHost::new();
        let mut d = DisplayControl::new([0xfff59000, 0xfff64000], 8).unwrap();
        write(&mut d, &mut h, 0xfff64000, 0x10000);
        write(&mut d, &mut h, 0xfff64000, 0);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0);
        advance(&mut d, &mut h, 8);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0x10000);
        assert_eq!(read(&mut d, &mut h, 0xfff59000), 0);
        write(&mut d, &mut h, 0xfff64010, 0x33);
        assert_eq!(read(&mut d, &mut h, 0xfff64010), 0x33);
        write(&mut d, &mut h, 0xfff64000, 0x10020);
        assert_eq!(read(&mut d, &mut h, 0xfff64000), 0);
    }
}
