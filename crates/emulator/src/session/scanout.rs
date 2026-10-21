//! Observational scanout through the native register model and physical RAM.
use super::Session;
use fujimod_emulator_devices::exr2::display::{Scanout, device::Observation, scanout::Frame};
impl Session {
    pub fn scanout_state(&self, name: &str) -> Result<serde_json::Value, String> {
        self.board
            .inspect_device::<Scanout, _>(self.node(name)?, |scanout| Ok(scanout.metadata()))
    }

    pub fn scanout(
        &self,
        name: &str,
        instance: usize,
        pixels: bool,
        diagnostic_hashes: bool,
    ) -> Result<Option<Frame>, String> {
        self.board
            .inspect_device::<Scanout, _>(self.node(name)?, |scanout| {
                let mut read = |address, count| {
                    let mut bytes = vec![0; count];
                    self.board.read_physical(address, &mut bytes)?;
                    Ok(bytes)
                };
                scanout.frame(
                    instance,
                    pixels,
                    &mut Observation {
                        read: &mut read,
                        diagnostic_hashes,
                    },
                )
            })
    }
    pub fn scanout_main(
        &self,
        name: &str,
        instance: usize,
        pixels: bool,
        diagnostic_hashes: bool,
    ) -> Result<Option<Frame>, String> {
        self.board
            .inspect_device::<Scanout, _>(self.node(name)?, |scanout| {
                let mut read = |address, count| {
                    let mut bytes = vec![0; count];
                    self.board.read_physical(address, &mut bytes)?;
                    Ok(bytes)
                };
                scanout.main_frame(
                    instance,
                    pixels,
                    &mut Observation {
                        read: &mut read,
                        diagnostic_hashes,
                    },
                )
            })
    }
}
