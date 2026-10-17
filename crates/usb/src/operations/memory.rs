use crate::{
    Error, Result,
    ff80::{
        Session,
        commands::memory::{Address, DebugMemoryLimits, MemoryRange, ReadMemory},
    },
};
use std::num::NonZeroU32;

/// Pull-based range download, one bounded command per `next_chunk().await`.
///
/// Retains progress without accumulating the full range. If a chunk fails,
/// previously returned chunks remain valid and no retry is issued automatically.
pub struct MemoryReader<'a> {
    session: &'a mut Session,
    range: MemoryRange,
    limits: DebugMemoryLimits,
    offset: u32,
}
impl<'a> MemoryReader<'a> {
    pub fn new(session: &'a mut Session, range: MemoryRange, limits: DebugMemoryLimits) -> Self {
        Self {
            session,
            range,
            limits,
            offset: 0,
        }
    }
    pub fn bytes_completed(&self) -> u32 {
        self.offset
    }
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>> {
        if self.offset == self.range.length().get() {
            return Ok(None);
        }
        let budget = self.session.max_reply_bytes().get();
        let maximum = self
            .limits
            .read
            .get()
            .min(self.session.info().advertised_packet_bytes.get())
            .min(budget.saturating_sub(36));
        if maximum == 0 {
            return Err(Error::LengthLimit {
                requested: 37,
                limit: budget,
            });
        }
        let Some(range) = chunk(self.range, self.offset, maximum) else {
            return Ok(None);
        };
        let data = self
            .session
            .read_memory(ReadMemory::new(range, self.limits)?)
            .await?;
        self.offset += range.length().get();
        Ok(Some(data))
    }
}

fn chunk(range: MemoryRange, offset: u32, maximum: u32) -> Option<MemoryRange> {
    let remaining = range.length().get().checked_sub(offset)?;
    let length = NonZeroU32::new(remaining.min(maximum))?;
    Some(
        MemoryRange::new(Address(range.address().0.checked_add(offset)?), length)
            .expect("subrange of a validated range"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_chunks_include_short_tail_and_final_address() {
        let range = MemoryRange::new(Address(u32::MAX - 5), NonZeroU32::new(6).unwrap()).unwrap();
        assert_eq!(chunk(range, 0, 4).unwrap().length().get(), 4);
        assert_eq!(chunk(range, 4, 4).unwrap().address(), Address(u32::MAX - 1));
        assert_eq!(chunk(range, 4, 4).unwrap().length().get(), 2);
        assert!(chunk(range, 6, 4).is_none());
    }
}
