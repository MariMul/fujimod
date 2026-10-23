//! Register banks with a Python dict's insertion order.

use std::collections::HashMap;

/// Register index -> value, iterating in first-insertion order.
/// Device state and fingerprints preserve that order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registers {
    order: Vec<u32>,
    values: HashMap<u32, u64>,
}

impl Registers {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, key: u32) -> Option<u64> {
        self.values.get(&key).copied()
    }
    pub fn contains(&self, key: u32) -> bool {
        self.values.contains_key(&key)
    }
    pub fn set(&mut self, key: u32, value: u64) {
        if self.values.insert(key, value).is_none() {
            self.order.push(key);
        }
    }
    /// `dict.update(other)`.
    pub fn update(&mut self, other: &Registers) {
        for (key, value) in other.iter() {
            self.set(key, value);
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (u32, u64)> + '_ {
        self.order.iter().map(|k| (*k, self.values[k]))
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    /// The value at `key`, which the caller has already required.
    pub fn at(&self, key: u32) -> u64 {
        self.values[&key]
    }
}
