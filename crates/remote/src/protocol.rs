//! Four asynchronous primitives.
use crate::{Control, Reference};
use fujimod_recording::Frame;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, future::Future, pin::Pin};

pub type Result<T> = std::result::Result<T, String>;
/// Futures deliberately have no Send bound: browser and board owners are local.
pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadSet {
    /// Physical half-open store ranges for boards; transport addresses for USB.
    pub stores: Vec<(u64, u64)>,
    pub devices: BTreeSet<usize>,
}
impl ReadSet {
    pub fn extend(&mut self, other: Self) {
        self.stores.extend(other.stores);
        self.devices.extend(other.devices);
        self.stores.sort_unstable();
        let mut union: Vec<(u64, u64)> = Vec::new();
        for (lo, hi) in self.stores.drain(..) {
            if let Some(last) = union.last_mut()
                && lo <= last.1
            {
                last.1 = last.1.max(hi);
            } else {
                union.push((lo, hi));
            }
        }
        self.stores = union;
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum InputState {
    Pressed,
    Released,
    Turn(i32),
    Stage(String),
    Position(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub control: Control,
    pub state: InputState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub mode: u32,
    pub menu_active: bool,
    pub node: u64,
    pub cursor: u32,
    pub current_key: Option<u32>,
    pub ready: bool,
    pub pending: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSnapshot {
    pub node: u64,
    pub key: Option<u32>,
    pub index: Option<u32>,
    pub child: Option<u64>,
    pub label: String,
    pub exists: bool,
    pub visible: bool,
    pub selected: bool,
    pub enabled: bool,
    pub value: Option<String>,
    /// Opening row references, ordered from the root toward this item.
    pub ancestors: Vec<crate::Menu>,
    pub root: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlSnapshot {
    pub id: String,
    pub position: Option<String>,
    pub supported: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Menu(MenuSnapshot),
    Control(ControlSnapshot),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub state: State,
    pub value: Value,
    pub reads: ReadSet,
}
/// Bytes from one explicitly requested range and the backend watch set that
/// can wake a wait for changes to those bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRead {
    pub address: u64,
    pub bytes: Vec<u8>,
    pub reads: ReadSet,
}
impl Snapshot {
    pub fn menu(&self) -> Result<&MenuSnapshot> {
        match &self.value {
            Value::Menu(value) => Ok(value),
            _ => Err("Expected a menu reference".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    GuestTicks,
    WallNanoseconds,
}
#[derive(Clone)]
pub struct Capture {
    pub frame: Frame,
    pub time: u64,
    pub clock: Clock,
    pub output: String,
    pub sequence: u64,
    pub reads: ReadSet,
    pub completion_reads: ReadSet,
}
/// This view cannot supply inputs or advance time. Frame reads here are passive;
/// screenshot() is the operation that waits for a completed frame. Each read
/// inspects state now; wait_until repeats its predicate and tracks these reads.
pub trait Observation {
    fn now(&self) -> u64;
    fn read<'a>(&'a mut self, reference: &'a Reference) -> LocalFuture<'a, Snapshot>;
    /// Read caller-selected physical bytes without assigning them semantic meaning.
    fn read_memory<'a>(&'a mut self, address: u64, size: usize) -> LocalFuture<'a, MemoryRead>;
    fn frame(&mut self) -> LocalFuture<'_, Capture>;
}
pub trait Remote: Observation {
    fn set_input<'a>(&'a mut self, input: &'a Input, deadline: u64) -> LocalFuture<'a, ()>;
    fn wait<'a>(&'a mut self, reads: &'a ReadSet, deadline: u64) -> LocalFuture<'a, ()>;
}
