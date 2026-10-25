//! SDH30 register selection and UHS-I command/data/clock port selection.
//! The fixed 1:1 base-clock path is supported. Other clock routes refuse.
use fujimod_emulator_devices::{Cx, Device};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    selection: bool,
    ports: u32,
}
pub struct Interface {
    base: u64,
    state: State,
}
impl Interface {
    pub fn new(base: u64) -> Result<Self, String> {
        if base & 0xfff != 0x1e0 {
            return Err("Invalid SD interface register base".into());
        }
        Ok(Self {
            base,
            state: State::default(),
        })
    }
}
impl Device for Interface {
    fn name(&self) -> &'static str {
        "sd-interface"
    }
    fn read(&mut self, cx: &mut Cx<'_>, address: u64, size: u8) -> u64 {
        match (address.wrapping_sub(self.base), size) {
            (0, 4) => u64::from(self.state.selection) << 16,
            (4, 4) => self.state.ports.into(),
            _ => {
                cx.unmodeled();
                0
            }
        }
    }
    fn write(&mut self, cx: &mut Cx<'_>, address: u64, size: u8, value: u64) {
        match (address.wrapping_sub(self.base), size) {
            (0, 4) if value & !0x10000 == 0 => self.state.selection = value != 0,
            (4, 4) if self.state.selection && value & !0xff0101 == 0 => {
                self.state.ports = value as u32
            }
            _ => cx.unmodeled(),
        }
    }
    fn save(&self) -> Value {
        serde_json::to_value(&self.state).unwrap()
    }
    fn restore(&mut self, value: &Value) -> Result<(), String> {
        let state: State = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if state.ports & !0xff0101 != 0 {
            return Err("Invalid SD interface port checkpoint".into());
        }
        self.state = state;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use fujimod_emulator_devices::test_host::recording::RecordingHost;
    #[test]
    fn register_selection_keeps_port_state_and_checkpoint() {
        let mut device = Interface::new(0xff9b01e0).unwrap();
        let mut host = RecordingHost::default();
        let mut cx = Cx::new(&mut host, 0, 0, 0);
        device.write(&mut cx, 0xff9b01e0, 4, 0x10000);
        device.write(&mut cx, 0xff9b01e4, 4, 0xff0101);
        device.write(&mut cx, 0xff9b01e0, 4, 0);
        let saved = device.save();
        assert_eq!(device.read(&mut cx, 0xff9b01e0, 4), 0);
        assert_eq!(device.read(&mut cx, 0xff9b01e4, 4), 0xff0101);
        device.restore(&saved).unwrap();
        assert_eq!(device.save(), saved);
    }
}
