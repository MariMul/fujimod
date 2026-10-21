//! Channel4's explicit connection to the two output channels. Each output
//! owns its configuration, deadline, row writes, status and IRQ44/45. Input
//! publishes the sampled pixels here and emits EOF38 one tick after both
//! output DMAs. The board declares input/output dependencies in both directions.
use super::dma::{Raster, Transfer};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[derive(Default, Serialize, Deserialize)]
pub struct MemoryOutputs {
    pub destinations: [Option<Raster>; 2],
    pub transfers: [Transfer; 2],
    pub active: bool,
}
pub type MemoryOutputHandle = Arc<Mutex<MemoryOutputs>>;
impl MemoryOutputs {
    pub fn fingerprint(&self) -> serde_json::Value {
        let Self {
            destinations,
            transfers,
            active,
        } = self;
        serde_json::json!({"destinations":destinations,"active":active,
            "transfers":transfers.iter().map(Transfer::fingerprint).collect::<Vec<_>>()})
    }
    pub fn configure(&mut self, channel: usize, raster: Option<Raster>) -> Result<(), String> {
        if channel >= 2 || self.active && !self.completed() {
            return Err("RAW readback outputs are busy or invalid".into());
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
