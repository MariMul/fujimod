//! A command dial emits physical Gray-code levels at declared edge times.
//! Host inspection never emits an edge. Firmware decodes the GPIO levels;
//! optional counter wiring delivers the same phases to the pulse unit.
use super::pins::{Pin, Pins};
use crate::exr2::pulse::PulsePins;
use crate::{Cx, Device};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub pins: [Pin; 2],
    pub phase_ticks: u64,
    pub rest_phase: u8,
}
pub struct Wiring {
    pub gpio: Box<dyn Pins>,
    pub counter: Option<PulsePins>,
}
pub struct Encoder {
    options: Options,
    wiring: Wiring,
    input: EncoderInput,
    phase: u8,
}
#[derive(Clone)]
pub struct EncoderInput {
    events: Arc<Mutex<VecDeque<(u64, u8)>>>,
    phase_ticks: u64,
    rest_phase: u8,
}
impl EncoderInput {
    pub fn admit_cycles(&self, now: u64, cycles: i32) -> Result<(), String> {
        self.validate_cycles(now, cycles, &self.events.lock().unwrap())
            .map(|_| ())
    }
    fn validate_cycles(
        &self,
        now: u64,
        cycles: i32,
        events: &VecDeque<(u64, u8)>,
    ) -> Result<usize, String> {
        if cycles.unsigned_abs() > 384 {
            return Err("encoder cycle request exceeds the signal queue bound".into());
        }
        let edges = cycles.unsigned_abs() as usize * 4;
        if events.len() + edges > 4096 {
            return Err("encoder signal queue is full".into());
        }
        events
            .back()
            .map_or(now, |e| e.0)
            .checked_add(
                self.phase_ticks
                    .checked_mul(edges as u64)
                    .ok_or("encoder deadline overflow")?,
            )
            .ok_or("encoder deadline overflow")?;
        Ok(edges)
    }
    pub fn queue_cycles(&self, now: u64, cycles: i32) -> Result<(), String> {
        let mut events = self.events.lock().unwrap();
        self.validate_cycles(now, cycles, &events)?;
        let mut at = events.back().map_or(now, |e| e.0);
        let order = [0, 1, 3, 2];
        let start = order.iter().position(|p| *p == self.rest_phase).unwrap();
        let phases =
            [1, 2, 3, 4].map(|step| order[(start + if cycles >= 0 { step } else { 4 - step }) % 4]);
        for _ in 0..cycles.unsigned_abs() {
            for phase in phases {
                at += self.phase_ticks;
                events.push_back((at, phase));
            }
        }
        Ok(())
    }
}
impl Encoder {
    pub fn new(options: Options, wiring: Wiring) -> Result<Self, String> {
        for pin in options.pins {
            pin.validate()?;
        }
        if options.pins[0] == options.pins[1] || options.phase_ticks == 0 || options.rest_phase > 3
        {
            return Err(
                "encoder requires distinct pads, a positive edge period and a two-bit rest phase"
                    .into(),
            );
        }
        if wiring.counter.as_ref().is_some_and(|p| {
            !p.quadrature_wired() || p.phase() != [2, 3, 1, 0][options.rest_phase as usize]
        }) {
            return Err("dial counter connection must start at the declared rest phase".into());
        }
        let input = EncoderInput {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(4096))),
            phase_ticks: options.phase_ticks,
            rest_phase: options.rest_phase,
        };
        let phase = options.rest_phase;
        Ok(Self {
            options,
            wiring,
            input,
            phase,
        })
    }
    /// Body initialization drives the declared external rest state explicitly.
    pub fn initialize(&mut self, cx: &mut Cx<'_>) -> Result<(), String> {
        for (pin, level) in self
            .options
            .pins
            .into_iter()
            .zip([self.phase & 2 != 0, self.phase & 1 != 0])
        {
            self.wiring.gpio.input(cx, pin, level)?;
        }
        Ok(())
    }
    pub fn queue_cycles(&mut self, now: u64, cycles: i32) -> Result<(), String> {
        self.input.queue_cycles(now, cycles)
    }
    pub fn input(&self) -> EncoderInput {
        self.input.clone()
    }
    pub fn phase(&self) -> u8 {
        self.phase
    }
}
impl Device for Encoder {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        crate::checkpoint::Checkpoint::encode(
            self.name(),
            &(
                self.phase,
                &*self
                    .input
                    .events
                    .lock()
                    .map_err(|_| "encoder queue poisoned")?,
            ),
        )
    }
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let _: (u8, VecDeque<(u64, u8)>) = saved.decode(self.name())?;
        Ok(())
    }
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let (phase, events) = saved.decode(self.name())?;
        self.phase = phase;
        *self
            .input
            .events
            .lock()
            .map_err(|_| "encoder queue poisoned")? = events;
        Ok(())
    }

    fn fingerprint(&self) -> Result<serde_json::Value, String> {
        Ok(
            serde_json::json!({"phase":self.phase,"rest_phase":self.input.rest_phase,"phase_ticks":self.input.phase_ticks,"events":*self.input.events.lock().unwrap()}),
        )
    }
    fn name(&self) -> &'static str {
        "encoder"
    }
    fn read(&mut self, cx: &mut Cx<'_>, _: u64, _: u8) -> u64 {
        cx.unmodeled();
        0
    }
    fn write(&mut self, cx: &mut Cx<'_>, _: u64, _: u8, _: u64) {
        cx.unmodeled();
    }
    fn next_deadline(&self, _: u64) -> Option<u64> {
        self.input.events.lock().unwrap().front().map(|e| e.0)
    }
    fn advance(&mut self, cx: &mut Cx<'_>) {
        let mut events = self.input.events.lock().unwrap();
        while let Some(&(at, phase)) = events.front() {
            if at > cx.now() {
                break;
            }
            for (pin, level) in self
                .options
                .pins
                .into_iter()
                .zip([phase & 2 != 0, phase & 1 != 0])
            {
                if let Err(error) = self.wiring.gpio.input(cx, pin, level) {
                    cx.fail(error);
                    return;
                }
            }
            if let Some(counter) = &self.wiring.counter {
                let ordinal = [2, 3, 1, 0][phase as usize];
                if let Err(error) = counter.quadrature_phase(cx, ordinal) {
                    cx.fail(error);
                    return;
                }
            }
            self.phase = phase;
            events.pop_front();
        }
    }
    fn event(&mut self, cx: &mut Cx<'_>, _: u64) {
        self.advance(cx);
    }
}
