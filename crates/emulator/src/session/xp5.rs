//! XP5 host controls shared by standard emulator sessions.
use fujimod_emulator_devices::parts::resistor_keys::{Key, ResistorKeys};
use fujimod_emulator_xp5::gpio::SharedPins;
use std::time::Duration;

/// External XP5 controls retained by a session after device construction.
/// Their modeled state is checkpointed by the devices that consume them.
pub struct State {
    pub(crate) pins: SharedPins,
    pub(crate) controls: std::sync::Arc<ResistorKeys>,
    pub(crate) gpio_devices: Vec<usize>,
    pub(crate) control_source: usize,
}

impl State {
    pub fn new(
        pins: SharedPins,
        controls: std::sync::Arc<ResistorKeys>,
        gpio_devices: Vec<usize>,
        control_source: usize,
    ) -> Self {
        Self {
            pins,
            controls,
            gpio_devices,
            control_source,
        }
    }
}

impl super::Session {
    /// Queue a physical XP5 key edge at an absolute guest nanosecond.
    pub fn xp5_key(&mut self, at_ns: u64, key: Key, pressed: bool) -> Result<(), String> {
        let state = self.xp5.as_ref().ok_or("session has no XP5 controls")?;
        state
            .controls
            .enqueue(Duration::from_nanos(at_ns), key, pressed)?;
        self.board.inputs_changed(state.control_source)
    }

    /// Drive one external XP5 GPIO input level and wake its registered devices.
    pub fn xp5_gpio(&mut self, port: usize, pin: usize, high: bool) -> Result<(), String> {
        if port >= 20 || pin >= 16 {
            return Err("XP5 GPIO input is outside the declared pin range".into());
        }
        let state = self.xp5.as_ref().ok_or("session has no XP5 GPIO inputs")?;
        state
            .pins
            .lock()
            .map_err(|_| "XP5 GPIO pins poisoned")?
            .set_input(port, pin, high);
        for &device in &state.gpio_devices {
            self.board.inputs_changed(device)?;
        }
        Ok(())
    }

    pub fn xp5_gpio_level(&self, port: usize, pin: usize) -> Result<bool, String> {
        if port >= 20 || pin >= 16 {
            return Err("XP5 GPIO input is outside the declared pin range".into());
        }
        let state = self.xp5.as_ref().ok_or("session has no XP5 GPIO inputs")?;
        Ok(state
            .pins
            .lock()
            .map_err(|_| "XP5 GPIO pins poisoned")?
            .level(port, pin)
            != 0)
    }

    pub fn xp5_gpio_input_levels(&self, port: usize) -> Result<u32, String> {
        if port >= 20 {
            return Err("XP5 GPIO port is outside the declared pin range".into());
        }
        let state = self.xp5.as_ref().ok_or("session has no XP5 GPIO inputs")?;
        Ok(state
            .pins
            .lock()
            .map_err(|_| "XP5 GPIO pins poisoned")?
            .word(port))
    }

    pub fn xp5_key_name(name: &str) -> Result<Key, String> {
        Key::parse(name)
    }
}
