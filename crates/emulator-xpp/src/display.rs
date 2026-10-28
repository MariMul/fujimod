//! X-Processor Pro display frame interrupts.
//!
//! An interrupt law, not a register model of the display controller: each
//! declared line receives one edge per frame period, counted from tick zero.
//! The X-T20 2.11 frame handlers (0xc9d854, 0xc9d994) keep their callback
//! state in RAM and read no controller register, so nothing is acknowledged.
//! Periods that pass between device calls raise one edge per line. The period
//! is a declared input; the panel rate is not measured.
use fujimod_emulator_devices::{Cx, Device};
use serde_json::{Value, json};

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
}
