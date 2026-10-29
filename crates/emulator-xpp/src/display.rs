//! X-Processor Pro display frame interrupts.
//!
//! An interrupt law, not a register model of the display controller: each
//! declared line receives one edge per frame period, counted from tick zero.
//! The X-T20 2.11 frame handlers (0xc9d854, 0xc9d994) keep their callback
//! state in RAM and read no controller register, so nothing is acknowledged.
//! Periods that pass between device calls raise one edge per line. The period
//! is a declared input; the panel rate is not measured.
//!
//! [`Output`] is the register window of one display output: word registers
//! read back their last write, except for declared status bits that read set
//! (a request the controller completes at once) or clear (a busy bit the
//! controller clears at once). No frame timing is attached to either.
use fujimod_emulator_devices::{Cx, Device};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct FrameClock {
    irqs: Vec<u32>,
    period: u64,
    next: u64,
}
impl FrameClock {
    pub fn new(irqs: Vec<u32>, period: u64) -> Result<Self, String> {
        let mut unique = irqs.clone();
        unique.sort_unstable();
        unique.dedup();
        if irqs.is_empty()
            || unique.len() != irqs.len()
            || irqs.iter().any(|irq| !(32..512).contains(irq))
            || period == 0
        {
            return Err("Invalid X-Processor Pro display frame interrupts".into());
        }
        Ok(Self {
            irqs,
            period,
            next: period,
        })
    }
}
impl Device for FrameClock {
    fn name(&self) -> &'static str {
        "xpp-display-frames"
    }
    fn read(&mut self, cx: &mut Cx<'_>, _address: u64, _size: u8) -> u64 {
        cx.unmodeled();
        0
    }
    fn write(&mut self, cx: &mut Cx<'_>, _address: u64, _size: u8, _value: u64) {
        cx.unmodeled();
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        let now = cx.now();
        if now < self.next {
            return;
        }
        for &irq in &self.irqs {
            cx.set_irq(irq, true);
            cx.set_irq(irq, false);
        }
        let frames = (now - self.next) / self.period + 1;
        match frames
            .checked_mul(self.period)
            .and_then(|step| self.next.checked_add(step))
        {
            Some(next) => self.next = next,
            None => cx.fail("Display frame deadline overflow"),
        }
    }
    fn next_deadline(&self, _now: u64) -> Option<u64> {
        Some(self.next)
    }
    fn save(&self) -> Value {
        json!({ "next": self.next })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let next = value["next"]
            .as_u64()
            .filter(|next| next % self.period == 0 && *next > 0)
            .ok_or("Invalid display frame checkpoint")?;
        self.next = next;
        Ok(())
    }
}

pub struct Output {
    base: u64,
    size: u64,
    set: BTreeMap<u64, u32>,
    clear: BTreeMap<u64, u32>,
    words: BTreeMap<u64, u32>,
}
impl Output {
    /// `set` and `clear` map word offsets to the status bits that read as
    /// set or clear regardless of the last write.
    pub fn new(
        base: u64,
        size: u64,
        set: &[(u64, u32)],
        clear: &[(u64, u32)],
    ) -> Result<Self, String> {
        let mut laws = [BTreeMap::new(), BTreeMap::new()];
        for (law, bits) in laws.iter_mut().zip([set, clear]) {
            for &(offset, mask) in bits {
                if offset & 3 != 0 || offset >= size || mask == 0 {
                    return Err("Invalid display output status bits".into());
                }
                *law.entry(offset).or_insert(0) |= mask;
            }
        }
        let [set, clear] = laws;
        if base & 0xfff != 0
            || size == 0
            || size & 0xfff != 0
            || set
                .iter()
                .any(|(offset, mask)| clear.get(offset).is_some_and(|c| c & mask != 0))
        {
            return Err("Invalid display output window".into());
        }
        Ok(Self {
            base,
            size,
            set,
            clear,
            words: BTreeMap::new(),
        })
    }
    fn offset(&self, cx: &mut Cx<'_>, address: u64, size: u8) -> Option<u64> {
        let offset = address.wrapping_sub(self.base);
        if size != 4 || offset & 3 != 0 || offset >= self.size {
            cx.unmodeled();
            return None;
        }
        Some(offset)
    }
}
impl Device for Output {
    fn name(&self) -> &'static str {
        "xpp-display-output"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        let Some(offset) = self.offset(cx, address, size) else {
            return 0;
        };
        let word = self.words.get(&offset).copied().unwrap_or(0);
        let set = self.set.get(&offset).copied().unwrap_or(0);
        let clear = self.clear.get(&offset).copied().unwrap_or(0);
        u64::from((word | set) & !clear)
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        let Some(offset) = self.offset(cx, address, size) else {
            return;
        };
        match u32::try_from(value) {
            Ok(0) => {
                self.words.remove(&offset);
            }
            Ok(value) => {
                self.words.insert(offset, value);
            }
            Err(_) => cx.unmodeled(),
        }
    }
    fn save(&self) -> Value {
        json!({ "words": self.words.iter().map(|(o, v)| [*o, u64::from(*v)]).collect::<Vec<_>>() })
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let rows: Vec<(u64, u32)> = serde_json::from_value(value["words"].clone())
            .map_err(|e| format!("Invalid display output checkpoint: {e}"))?;
        if rows
            .iter()
            .any(|(offset, v)| offset & 3 != 0 || *offset >= self.size || *v == 0)
        {
            return Err("Invalid display output checkpoint".into());
        }
        self.words = rows.into_iter().collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::{Host, Record};

    #[derive(Default)]
    struct Clock {
        now: u64,
        edges: Vec<(u64, u32)>,
    }
    impl Host for Clock {
        fn now(&self) -> u64 {
            self.now
        }
        fn schedule(&mut self, _: usize, _: u64, _: u64) {
            unreachable!()
        }
        fn set_irq(&mut self, irq: u32, level: bool) {
            if level {
                self.edges.push((self.now, irq));
            }
        }
        fn raise_irq(&mut self, _: usize, _: u32) -> Result<(), String> {
            unreachable!()
        }
        fn ram_read(&self, _: u64, _: &mut [u8]) -> Result<(), String> {
            unreachable!()
        }
        fn ram_write(&mut self, _: u64, _: &[u8]) -> Result<(), String> {
            unreachable!()
        }
        fn record(&mut self, _: Record) {}
    }
    fn advance(clock: &mut FrameClock, host: &mut Clock, now: u64) {
        host.now = now;
        clock.advance(&mut Cx::new(host, 0, 0, 0));
    }

    #[test]
    fn each_period_raises_one_edge_per_line_and_skips_missed_frames() {
        assert!(FrameClock::new(vec![], 10).is_err());
        assert!(FrameClock::new(vec![112, 112], 10).is_err());
        assert!(FrameClock::new(vec![112], 0).is_err());
        let mut clock = FrameClock::new(vec![112, 117], 10).unwrap();
        assert_eq!(clock.next_deadline(0), Some(10));
        let mut host = Clock::default();
        advance(&mut clock, &mut host, 9);
        assert!(host.edges.is_empty());
        advance(&mut clock, &mut host, 35);
        assert_eq!(host.edges, vec![(35, 112), (35, 117)]);
        assert_eq!(clock.next_deadline(35), Some(40));
        let saved = clock.save();
        advance(&mut clock, &mut host, 40);
        assert_eq!(clock.next_deadline(40), Some(50));
        clock.restore(&saved).unwrap();
        assert_eq!(clock.next_deadline(40), Some(40));
        assert!(clock.restore(&json!({ "next": 41 })).is_err());
    }

    #[test]
    fn outputs_read_back_words_with_declared_status_bits() {
        use fujimod_emulator_devices::test_host::{TestHost, read, write};
        const BASE: u64 = 0xfff64000;
        assert!(Output::new(BASE, 0x3000, &[(0, 0x10000)], &[(0, 0x10000)]).is_err());
        assert!(Output::new(BASE, 0x3000, &[(0x3000, 1)], &[]).is_err());
        let mut out = Output::new(BASE, 0x3000, &[(0, 0x10000)], &[(0x2000, 1 << 28)]).unwrap();
        let mut h = TestHost::new();
        assert_eq!(read(&mut out, &mut h, BASE), 0x10000);
        write(&mut out, &mut h, BASE, 0x10001);
        write(&mut out, &mut h, BASE, 1);
        assert_eq!(read(&mut out, &mut h, BASE), 0x10001);
        write(&mut out, &mut h, BASE + 0x2000, 0x1002_5000);
        assert_eq!(read(&mut out, &mut h, BASE + 0x2000), 0x25000);
        write(&mut out, &mut h, BASE + 0x1004, 0x1c1044b);
        let saved = out.save();
        write(&mut out, &mut h, BASE + 0x1004, 0);
        assert_eq!(read(&mut out, &mut h, BASE + 0x1004), 0);
        out.restore(&saved).unwrap();
        assert_eq!(read(&mut out, &mut h, BASE + 0x1004), 0x1c1044b);
        let mut cx = h.cx();
        out.read(&mut cx, BASE + 2, 2);
        assert!(cx.is_unmodeled());
    }
}
