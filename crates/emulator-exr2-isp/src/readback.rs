//! Channel4's explicit connection to the two output channels. Each output
//! owns its configuration, deadline, row writes, status and IRQ44/45. Input
//! publishes the sampled pixels here and emits EOF38 one tick after participating
//! output DMAs. The board declares input/output dependencies in both directions.
use fujimod_emulator_devices::exr2::isp::dma::{Raster, Transfer};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[derive(Default, Serialize, Deserialize)]
pub struct MemoryOutputs {
    /// FFF10004 bit0 selects RAW transport instead of direct image output.
    pub raw_input: bool,
    pub destinations: [Option<Raster>; 2],
    pub active_destinations: [Option<Raster>; 2],
    pub transfers: [Transfer; 2],
    pub active: bool,
}
pub type MemoryOutputHandle = Arc<Mutex<MemoryOutputs>>;
impl MemoryOutputs {
    pub fn fingerprint(&self) -> serde_json::Value {
        let Self {
            raw_input,
            destinations,
            active_destinations,
            transfers,
            active,
        } = self;
        serde_json::json!({"raw_input":raw_input,"destinations":destinations,
            "active_destinations":active_destinations,"active":active,
            "transfers":transfers.iter().map(Transfer::fingerprint).collect::<Vec<_>>()})
    }
    pub fn configure(&mut self, channel: usize, raster: Option<Raster>) -> Result<(), String> {
        if channel >= 2 {
            return Err("RAW readback output channel is invalid".into());
        }
        if self.destinations[channel] == raster {
            return Ok(());
        }
        if let Some(r) = raster {
            r.validate(r.width * 2)?;
        }
        self.destinations[channel] = raster;
        Ok(())
    }
    pub fn completed(&self) -> bool {
        self.active && self.transfers.iter().all(|t| t.deadline().is_none())
    }
}
