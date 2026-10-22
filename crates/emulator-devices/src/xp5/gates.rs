//! Recovered reset/gate latches and request/status mirrors. Initial values
//! are explicitly zero session assumptions, not hardware reset measurements.
use crate::{Cx, Device};
use serde_json::Value;
use std::collections::BTreeMap;
pub struct Gates {
    power: bool,
    regs: BTreeMap<u64, u32>,
    read_values: BTreeMap<u64, u32>,
    panel_clocks: Option<super::panel_clock::Clocks>,
    companion_cpu: Option<usize>,
}
impl Gates {
    pub fn new(power: bool) -> Self {
        Self::with_panel_clocks(power, None)
    }
    pub fn with_panel_clocks(
        power: bool,
        panel_clocks: Option<super::panel_clock::Clocks>,
    ) -> Self {
        Self {
            power,
            regs: BTreeMap::new(),
            read_values: BTreeMap::new(),
            panel_clocks,
            companion_cpu: None,
        }
    }
    /// BIS reset bank10: c928 bit21 clear holds reset, set releases it.
    pub fn with_companion(mut self, cpu: Option<usize>) -> Self {
        self.companion_cpu = cpu;
        self
    }
    /// Supply aligned, read-only register values from the caller's hardware inputs.
    pub fn with_read_values(mut self, values: BTreeMap<u64, u32>) -> Result<Self, String> {
        if values
            .keys()
            .any(|offset| offset & 3 != 0 || self.latch(*offset) || self.status(*offset).is_some())
        {
            return Err("gate read values must name aligned, otherwise-unmodeled registers".into());
        }
        self.read_values = values;
        Ok(self)
    }
    fn drive_reset(&self, cx: &mut Cx<'_>) {
        if !self.power {
            if let Some(cpu) = self.companion_cpu {
                if let Err(e) = cx.reset_core(
                    cpu,
                    self.regs.get(&0x928).copied().unwrap_or(0) & 0x200000 == 0,
                ) {
                    cx.fail(e);
                }
            }
        }
    }
    fn latch(&self, o: u64) -> bool {
        if self.power {
            matches!(
                o,
                0x510 | 0xb00 | 0xb04 | 0xb08 | 0xb0c | 0xb10 | 0xb1c | 0xb44
            )
        } else {
            (0x300..=0x334).contains(&o)
                || (0x900..=0x944).contains(&o)
                || matches!(o, 0x600 | 0x604 | 0x608 | 0x700)
        }
    }
    fn status(&self, o: u64) -> Option<u64> {
        if self.power && matches!(o, 0xb20 | 0xb24 | 0xb28 | 0xb2c | 0xb30 | 0xb3c | 0xb64) {
            Some(o - 0x20)
        } else {
            None
        }
    }
}
impl Device for Gates {
    fn name(&self) -> &'static str {
        if self.power {
            "power-gates"
        } else {
            "clock-reset-gates"
        }
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, n: u8) -> u64 {
        let o = address.wrapping_sub(if self.power { 0xff90d000 } else { 0xff90c000 });
        if n != 4 || o & 3 != 0 {
            cx.unmodeled();
            return 0;
        }
        let a = if self.latch(o) {
            o
        } else if let Some(a) = self.status(o) {
            a
        } else if self.read_values.contains_key(&o) {
            o
        } else {
            cx.unmodeled();
            return 0;
        };
        self.regs
            .get(&a)
            .or_else(|| self.read_values.get(&a))
            .copied()
            .unwrap_or(0) as u64
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, n: u8, v: u64) {
        let o = address.wrapping_sub(if self.power { 0xff90d000 } else { 0xff90c000 });
        if n != 4 || o & 3 != 0 || !self.latch(o) {
            cx.unmodeled();
            return;
        }
        let previous = self.regs.insert(o, v as u32).unwrap_or(0);
        if !self.power && o == 0x928 && (previous ^ v as u32) & 0x200000 != 0 {
            self.drive_reset(cx);
        }
        if !self.power && o == 0x608 {
            if let Some(clocks) = &self.panel_clocks {
                clocks.selector(v as u32);
            }
        }
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.regs).unwrap()
    }
    fn restore(&mut self, v: &Value) -> Result<(), String> {
        self.regs = serde_json::from_value(v.clone())
            .map_err(|e| format!("invalid gate checkpoint: {e}"))?;
        // CPU reset state is restored by the framework with its CPU snapshot.
        if !self.power {
            if let Some(clocks) = &self.panel_clocks {
                clocks.selector(self.regs.get(&0x608).copied().unwrap_or(0));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xp5::tests::TestHost;

    #[test]
    fn reset_output_is_explicit_and_checkpoint_reads_are_observational() {
        let mut gates = Gates::new(false).with_companion(Some(4));
        let mut host = TestHost::default();
        gates.write(&mut Cx::new(&mut host, 0, 0, 0), 0xff90c928, 4, 0x200000);
        gates.write(&mut Cx::new(&mut host, 0, 0, 0), 0xff90c928, 4, 0);
        assert_eq!(host.resets, [(4, false), (4, true)]);
        let state = gates.save();
        gates.restore(&state).unwrap();
        assert_eq!(state, gates.save());
        assert_eq!(host.resets, [(4, false), (4, true)]);
    }
}
