//! Owned, observational XP4 execution values for host transports.
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Call {
    pub core: usize,
    pub pc: u32,
    pub instructions: u64,
    pub lr: u32,
    pub sp: u32,
    pub args: Vec<u32>,
    pub result: Option<u32>,
}

#[derive(Serialize)]
pub struct State {
    pub instructions: u64,
    pub virtual_ticks: u64,
    pub per_core: Vec<u64>,
    pub waiting: Vec<bool>,
    pub irq_entries: Vec<u64>,
    pub undefined_entries: Vec<u64>,
    pub prefetch_aborts: Vec<u64>,
    pub data_aborts: Vec<u64>,
    pub next_device_tick: Option<u64>,
    pub eligible_irqs: Vec<Option<usize>>,
    pub failure: Option<String>,
    pub cbar_reads: u64,
    pub history: Option<History>,
}

#[derive(Serialize)]
pub struct History {
    pub recent: Vec<Vec<(u32, u32)>>,
    pub milestones: Vec<(usize, u32, u64)>,
    pub calls: Vec<Call>,
}

#[derive(Serialize)]
pub struct Frame {
    pub output: usize,
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub state_json: Option<String>,
}

#[derive(Serialize)]
pub struct InterruptState {
    pub distributor_control: Vec<u32>,
    pub cpu_control: Vec<u32>,
    pub enabled: Vec<u32>,
    pub pending: Vec<u32>,
    pub active: Vec<u32>,
}
