//! The physical focus-axis position and the declared optical blur law that reads it.
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Position(Arc<Mutex<i64>>);
impl Position {
    pub fn value(&self) -> i64 {
        *self.0.lock().unwrap()
    }
    pub fn publish(&self, value: i64) {
        *self.0.lock().unwrap() = value;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub target_position: i64,
    pub pulses_per_radius: u32,
    pub maximum_radius: u32,
}
