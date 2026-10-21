//! External resistor-key inputs with body-declared samples and pin wiring.
//! No firmware memory or execution state is available at this boundary.
//! The input is an explicit 12-bit resistor-ladder sample.
use crate::parts::controls::Controls;
use crate::parts::pins::{DriveInput, Pin};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    Menu,
    Back,
    Up,
    Down,
    Left,
    Right,
    ShutterHalf,
    ShutterFull,
    RearPlus,
    RearMinus,
    FrontPlus,
    FrontMinus,
}
impl Key {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_uppercase().as_str() {
            "MENU" | "OK" => Ok(Self::Menu),
            "BACK" | "DISP" | "DISP/BACK" => Ok(Self::Back),
            "UP" => Ok(Self::Up),
            "DOWN" => Ok(Self::Down),
            "LEFT" => Ok(Self::Left),
            "RIGHT" => Ok(Self::Right),
            "S1" | "HALF" | "SHUTTER_HALF" => Ok(Self::ShutterHalf),
            "S2" | "FULL" | "SHUTTER_FULL" => Ok(Self::ShutterFull),
            "REAR+" => Ok(Self::RearPlus),
            "REAR-" => Ok(Self::RearMinus),
            "FRONT+" => Ok(Self::FrontPlus),
            "FRONT-" => Ok(Self::FrontMinus),
            _ => Err(format!("unknown physical key {s:?}")),
        }
    }
    fn dial(self) -> bool {
        matches!(
            self,
            Self::RearPlus | Self::RearMinus | Self::FrontPlus | Self::FrontMinus
        )
    }
}
/// 12-bit resistor samples within the configured default bands.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeySamples {
    pub menu: u16,
    pub back: u16,
    pub menu_back: u16,
    pub up: u16,
    pub down: u16,
    pub left: u16,
    pub right: u16,
}
impl KeySamples {
    fn validate(self) -> Result<(), String> {
        if [
            self.menu,
            self.back,
            self.menu_back,
            self.up,
            self.down,
            self.left,
            self.right,
        ]
        .into_iter()
        .any(|v| v > 4095)
        {
            return Err("physical key samples must be 12-bit".into());
        }
        Ok(())
    }
}

/// Body-owned physical wiring. Dial pins are ordered as phase bit 2 then bit 1.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub shutter_half: Pin,
    pub shutter_full: Pin,
    pub rear_dial: [Pin; 2],
    pub front_dial: [Pin; 2],
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialLevel {
    pub pin: Pin,
    pub high: bool,
}

/// Electrical levels, resistor samples, and input timing supplied by the body.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub layout: Layout,
    pub initial_levels: Vec<InitialLevel>,
    pub samples: KeySamples,
    pub detent_return_ns: u64,
    pub shutter_half_delay_ns: u64,
}

impl Options {
    fn validate(&self) -> Result<(), String> {
        self.samples.validate()?;
        if self.detent_return_ns == 0 || self.shutter_half_delay_ns == 0 {
            return Err("physical input timing must be positive".into());
        }
        let layout = [
            self.layout.shutter_half,
            self.layout.shutter_full,
            self.layout.rear_dial[0],
            self.layout.rear_dial[1],
            self.layout.front_dial[0],
            self.layout.front_dial[1],
        ];
        let mut pins = BTreeSet::new();
        for pin in layout {
            pin.validate()?;
            if !pins.insert((pin.port, pin.pin)) {
                return Err("physical control layout reuses a GPIO pad".into());
            }
        }
        let mut initialized = BTreeSet::new();
        for level in &self.initial_levels {
            level.pin.validate()?;
            if !initialized.insert((level.pin.port, level.pin.pin)) {
                return Err("physical input initial level repeats a GPIO pad".into());
            }
        }
        if layout
            .iter()
            .any(|pin| !initialized.contains(&(pin.port, pin.pin)))
        {
            return Err("physical control layout is missing an initial GPIO level".into());
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Edge {
    at: Duration,
    key: Key,
    pressed: bool,
    automatic: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct InputState {
    held: BTreeSet<Key>,
    pending: Vec<Edge>,
    rear_phase: u8,
    front_phase: u8,
    now: Duration,
    automatic_half: bool,
    started: bool,
    used: bool,
}
/// The CLI and a future frontend enqueue the same physical edges. No UI event
/// queues, globals, debounce state or guest code are changed by this API.
#[derive(Clone)]
pub struct ResistorKeys {
    state: Arc<Mutex<InputState>>,
    pins: Arc<dyn DriveInput>,
    options: Options,
}
impl ResistorKeys {
    pub fn new(pins: Arc<dyn DriveInput>, options: Options) -> Result<Self, String> {
        options.validate()?;
        for level in &options.initial_levels {
            pins.set_input(level.pin, level.high);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(InputState::default())),
            pins,
            options,
        })
    }
    /// CLI times are relative to guest execution, excluding RAM/hash/shim
    /// setup. Pending events restored from checkpoints already have an epoch.
    pub fn start(&self, now: Duration) {
        let mut s = self.state.lock().unwrap();
        if !s.started {
            for edge in &mut s.pending {
                edge.at += now;
            }
            s.now = now;
            s.started = true;
        }
    }
    pub fn enqueue(&self, at: Duration, key: Key, pressed: bool) -> Result<(), String> {
        if key.dial() && !pressed {
            return Err("dial detents have an automatic return phase".into());
        }
        let mut s = self.state.lock().unwrap();
        let at = if at == Duration::ZERO { s.now } else { at };
        let return_at = if key.dial() {
            Some(
                at.checked_add(Duration::from_nanos(self.options.detent_return_ns))
                    .ok_or("key time overflow")?,
            )
        } else {
            None
        };
        s.used = true;
        s.pending.push(Edge {
            at,
            key,
            pressed,
            automatic: false,
        });
        if let Some(at) = return_at {
            s.pending.push(Edge {
                at,
                key,
                pressed: false,
                automatic: false,
            });
        }
        s.pending.sort_by_key(|e| e.at);
        Ok(())
    }
    pub fn script(&self, script: &str) -> Result<(), String> {
        self.script_at(script, Duration::ZERO)
    }
    /// Append a script relative to a resumed VM's release time, preserving
    /// the held keys and queued edges restored from the checkpoint.
    pub fn script_at(&self, script: &str, origin: Duration) -> Result<(), String> {
        // Parse all entries before mutating the queue.
        let mut edges = Vec::new();
        for entry in script.split(';').filter(|s| !s.trim().is_empty()) {
            let (time, name) = entry
                .trim()
                .split_once(':')
                .ok_or("keys need t=SECONDS:KEY")?;
            let time = time.strip_prefix("t=").ok_or("keys need t=SECONDS:KEY")?;
            let delay =
                Duration::try_from_secs_f64(time.parse::<f64>().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let at = origin.checked_add(delay).ok_or("key time overflow")?;
            let (pressed, name) = name.strip_prefix('-').map_or((true, name), |n| (false, n));
            let key = Key::parse(name)?;
            if key.dial() && !pressed {
                return Err("dial detents release automatically".into());
            }
            edges.push((at, key, pressed));
        }
        for (at, key, pressed) in edges {
            self.enqueue(at, key, pressed)?;
        }
        Ok(())
    }
    /// Exact deadline of the next queued physical edge, for the framework's
    /// external-input scheduler. Empty queues have no periodic deadline.
    pub fn next_deadline(&self) -> Option<u64> {
        self.state
            .lock()
            .unwrap()
            .pending
            .first()
            .map(|edge| edge.at.as_nanos().min(u64::MAX as u128) as u64)
    }
    pub fn used(&self) -> bool {
        self.state.lock().unwrap().used
    }
}
impl Controls for ResistorKeys {
    fn next_deadline(&self) -> Option<u64> {
        ResistorKeys::next_deadline(self)
    }
    fn advance(&self, now: u64) -> Vec<String> {
        let now = Duration::from_nanos(now);
        let mut lines = Vec::new();
        let mut s = self.state.lock().unwrap();
        s.now = now;
        while s.pending.first().is_some_and(|e| e.at <= now) {
            let mut e = s.pending.remove(0);
            if e.key == Key::ShutterFull && e.pressed && !s.held.contains(&Key::ShutterHalf) {
                s.pending.push(Edge {
                    at: e.at + Duration::from_nanos(self.options.shutter_half_delay_ns),
                    key: Key::ShutterFull,
                    pressed: true,
                    automatic: true,
                });
                s.automatic_half = true;
                e.key = Key::ShutterHalf;
                e.automatic = true;
            } else if e.key == Key::ShutterFull && !e.pressed {
                s.pending
                    .retain(|p| !(p.key == Key::ShutterFull && p.automatic && p.pressed));
                if s.automatic_half {
                    s.pending.push(Edge {
                        at: e.at + Duration::from_nanos(self.options.shutter_half_delay_ns),
                        key: Key::ShutterHalf,
                        pressed: false,
                        automatic: true,
                    });
                    s.automatic_half = false;
                }
            } else if e.key == Key::ShutterHalf && e.pressed && !e.automatic {
                s.automatic_half = false;
                s.pending
                    .retain(|p| !(p.key == Key::ShutterHalf && p.automatic && !p.pressed));
            }
            if e.pressed && matches!(e.key, Key::Up | Key::Down | Key::Left | Key::Right) {
                s.held
                    .retain(|k| !matches!(k, Key::Up | Key::Down | Key::Left | Key::Right));
            }
            s.pending.sort_by_key(|p| p.at);
            if e.pressed {
                s.held.insert(e.key);
            } else {
                s.held.remove(&e.key);
            }
            // Physical rotation has fixed quadrature polarity. The guest's
            // dial-direction preference is applied by its own scanner.
            let phase = |plus: bool| if plus { 1 } else { 2 };
            match e.key {
                Key::RearPlus | Key::RearMinus => {
                    s.rear_phase = if e.pressed {
                        phase(e.key == Key::RearPlus)
                    } else {
                        0
                    }
                }
                Key::FrontPlus | Key::FrontMinus => {
                    s.front_phase = if e.pressed {
                        phase(e.key == Key::FrontPlus)
                    } else {
                        0
                    }
                }
                _ => {}
            }
            // S2 electrically implies S1, including when the API receives only
            // FULL. Release of FULL retains explicitly held HALF.
            let full = s.held.contains(&Key::ShutterFull);
            self.pins.set_input(
                self.options.layout.shutter_half,
                !(full || s.held.contains(&Key::ShutterHalf)),
            );
            self.pins.set_input(self.options.layout.shutter_full, !full);
            self.pins
                .set_input(self.options.layout.rear_dial[0], s.rear_phase & 2 != 0);
            self.pins
                .set_input(self.options.layout.rear_dial[1], s.rear_phase & 1 != 0);
            self.pins
                .set_input(self.options.layout.front_dial[0], s.front_phase & 2 != 0);
            self.pins
                .set_input(self.options.layout.front_dial[1], s.front_phase & 1 != 0);
            lines.push(format!(
                "physical input {:?} {} at {:.3}s",
                e.key,
                if e.pressed { "press" } else { "release" },
                e.at.as_secs_f64()
            ));
        }
        lines
    }
    fn sample(&self, channel: usize, fallback: u16) -> u16 {
        let s = self.state.lock().unwrap();
        if channel == 11 {
            match (s.held.contains(&Key::Menu), s.held.contains(&Key::Back)) {
                (true, true) => self.options.samples.menu_back,
                (true, false) => self.options.samples.menu,
                (false, true) => self.options.samples.back,
                (false, false) => fallback,
            }
        } else if channel == 14 {
            [
                (Key::Up, self.options.samples.up),
                (Key::Down, self.options.samples.down),
                (Key::Left, self.options.samples.left),
                (Key::Right, self.options.samples.right),
            ]
            .into_iter()
            .find(|(k, _)| s.held.contains(k))
            .map_or(fallback, |(_, sample)| sample)
        } else {
            fallback
        }
    }
    fn save(&self) -> Value {
        serde_json::to_value(&*self.state.lock().unwrap()).unwrap()
    }
    fn restore(&self, v: &Value) -> Result<(), String> {
        let state = serde_json::from_value(v.clone())
            .map_err(|e| format!("physical controls checkpoint: {e}"))?;
        *self.state.lock().unwrap() = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xp5::gpio::{Pins, SharedPins};

    fn options() -> Options {
        Options {
            layout: Layout {
                shutter_half: Pin { port: 0, pin: 0 },
                shutter_full: Pin { port: 0, pin: 1 },
                rear_dial: [Pin { port: 5, pin: 7 }, Pin { port: 5, pin: 9 }],
                front_dial: [Pin { port: 5, pin: 8 }, Pin { port: 13, pin: 5 }],
            },
            initial_levels: [
                (0, 0, true),
                (0, 1, true),
                (5, 7, false),
                (5, 9, false),
                (5, 8, false),
                (13, 5, false),
                (11, 4, false),
                (4, 4, false),
            ]
            .into_iter()
            .map(|(port, pin, high)| InitialLevel {
                pin: Pin { port, pin },
                high,
            })
            .collect(),
            samples: KeySamples {
                menu: 3472,
                back: 2227,
                menu_back: 1999,
                up: 1558,
                down: 3409,
                left: 2829,
                right: 2068,
            },
            detent_return_ns: 60_000_000,
            shutter_half_delay_ns: 30_000_000,
        }
    }
    fn input() -> (ResistorKeys, SharedPins) {
        let pins = Pins::shared();
        let driver: Arc<dyn DriveInput> = pins.clone();
        (ResistorKeys::new(driver, options()).unwrap(), pins)
    }
    #[test]
    fn resistor_ladders_are_explicit_and_inspection_is_observational() {
        let (input, _) = input();
        for (key, channel, sample) in [
            (Key::Menu, 11, 3472),
            (Key::Back, 11, 2227),
            (Key::Up, 14, 1558),
            (Key::Down, 14, 3409),
            (Key::Left, 14, 2829),
            (Key::Right, 14, 2068),
        ] {
            input.enqueue(Duration::ZERO, key, true).unwrap();
            let before = input.save();
            assert_eq!(input.sample(channel, 4095), 4095);
            assert_eq!(input.save(), before);
            input.advance(0);
            assert_eq!(input.sample(channel, 4095), sample);
            input.enqueue(Duration::ZERO, key, false).unwrap();
            input.advance(0);
        }
        input.script("t=1:MENU;t=1:BACK").unwrap();
        input.advance(1_000_000_000);
        assert_eq!(input.sample(11, 4095), 1999);
    }
    #[test]
    fn shutter_sequence_and_dial_return_use_edge_time_not_service_frequency() {
        let (frequent, frequent_pins) = input();
        let (batched, batched_pins) = input();
        let script = "t=0.1:FULL;t=0.2:-FULL;t=0.3:REAR+;t=0.4:REAR-;t=0.5:FRONT+;t=0.6:FRONT-";
        for input in [&frequent, &batched] {
            input.script(script).unwrap();
        }
        frequent.advance(100_000_000);
        assert_eq!(frequent_pins.lock().unwrap().word(0) & 3, 2); // S1 only
        frequent.advance(129_999_999);
        assert_eq!(frequent_pins.lock().unwrap().word(0) & 3, 2);
        frequent.advance(130_000_000);
        assert_eq!(frequent_pins.lock().unwrap().word(0) & 3, 0); // S1+S2
        frequent.advance(200_000_000);
        assert_eq!(frequent_pins.lock().unwrap().word(0) & 3, 2);
        frequent.advance(230_000_000);
        assert_eq!(frequent_pins.lock().unwrap().word(0) & 3, 3);
        for tick in [
            300_000_000,
            360_000_000,
            400_000_000,
            460_000_000,
            500_000_000,
            560_000_000,
            600_000_000,
            660_000_000,
            1_000_000_000,
        ] {
            frequent.advance(tick);
        }
        batched.advance(1_000_000_000);
        assert_eq!(frequent.save(), batched.save());
        assert_eq!(
            serde_json::to_value(&*frequent_pins.lock().unwrap()).unwrap(),
            serde_json::to_value(&*batched_pins.lock().unwrap()).unwrap()
        );
        let saved = batched.save();
        assert!(batched.restore(&Value::Null).is_err());
        assert_eq!(batched.save(), saved);
    }
}
