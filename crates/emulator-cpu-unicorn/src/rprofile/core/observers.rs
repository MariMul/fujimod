//! Passive callbacks indexed by kind and disjoint address spans. Mutations
//! happen only while stopped; execution looks up one span instead of scanning
//! every installed observer. Entries retain hook installation order.
use super::*;

#[derive(Default)]
pub(super) struct Ranges(BTreeMap<u64, Vec<(usize, Hook)>>);

impl Ranges {
    pub fn at(&self, address: u64) -> &[(usize, Hook)] {
        self.0
            .range(..=address)
            .next_back()
            .map_or(&[], |(_, hooks)| hooks.as_slice())
    }

    fn split(&mut self, address: u64) {
        if !self.0.contains_key(&address) {
            let hooks = self.at(address).to_vec();
            self.0.insert(address, hooks);
        }
    }

    fn bounds(range: HookRange) -> Option<(u64, u64)> {
        match range {
            HookRange::All => Some((0, u64::MAX)),
            HookRange::Inclusive { begin, end } => (begin <= end).then_some((begin, end)),
        }
    }

    fn insert(&mut self, key: usize, hook: Hook, range: HookRange) {
        let Some((begin, end)) = Self::bounds(range) else {
            return;
        };
        self.split(begin);
        if let Some(after) = end.checked_add(1) {
            self.split(after);
        }
        for (_, hooks) in self.0.range_mut(begin..=end) {
            hooks.push((key, hook));
        }
    }

    fn remove(&mut self, key: usize, range: HookRange) {
        let Some((begin, end)) = Self::bounds(range) else {
            return;
        };
        for (_, hooks) in self.0.range_mut(begin..=end) {
            hooks.retain(|(id, _)| *id != key);
        }
        // Coalesce spans after removal, including an empty leading span.
        let mut previous = Vec::new();
        self.0.retain(|_, hooks| {
            let same = previous.len() == hooks.len()
                && previous.iter().zip(hooks.iter()).all(|(a, (b, _))| a == b);
            if !same {
                previous = hooks.iter().map(|(key, _)| *key).collect();
            }
            !same
        });
    }
}

#[derive(Default)]
pub(super) struct Index {
    pub instructions: Ranges,
    pub blocks: Ranges,
    pub stores: Ranges,
    pub reads: Ranges,
    pub writes: Ranges,
    pub exceptions: Vec<(usize, Hook)>,
    pub synchronization: Vec<(usize, Hook)>,
}

impl Index {
    pub fn insert(&mut self, key: usize, hook: Hook) {
        match hook {
            Hook::Instruction { range, .. } => self.instructions.insert(key, hook, range),
            Hook::Block { range, .. } => self.blocks.insert(key, hook, range),
            Hook::CompletedStore { range, .. } => self.stores.insert(key, hook, range),
            Hook::Exception { .. } => self.exceptions.push((key, hook)),
            Hook::Synchronization { .. } => self.synchronization.push((key, hook)),
            Hook::Memory {
                read, write, range, ..
            } => {
                if read {
                    self.reads.insert(key, hook, range);
                }
                if write {
                    self.writes.insert(key, hook, range);
                }
            }
            Hook::Fault { .. } => (),
        }
    }

    pub fn remove(&mut self, key: usize, hook: Hook) {
        match hook {
            Hook::Instruction { range, .. } => self.instructions.remove(key, range),
            Hook::Block { range, .. } => self.blocks.remove(key, range),
            Hook::CompletedStore { range, .. } => self.stores.remove(key, range),
            Hook::Exception { .. } => self.exceptions.retain(|(id, _)| *id != key),
            Hook::Synchronization { .. } => self.synchronization.retain(|(id, _)| *id != key),
            Hook::Memory {
                read, write, range, ..
            } => {
                if read {
                    self.reads.remove(key, range);
                }
                if write {
                    self.writes.remove(key, range);
                }
            }
            Hook::Fault { .. } => (),
        }
    }
}

#[cfg(test)]
mod tests;
