use fujimod_usb::{
    Error, Result,
    ff80::{
        commands::memory::{Address, DebugMemoryLimits, MemoryRange},
        sequence::SequenceCommand,
    },
};
use std::num::NonZeroU32;

/// Plan all bounded write/readback pairs before the first USB command. Each
/// expected read runs at the USB owner and discards later work on mismatch.
/// The caller must authorize the complete range before executing these batches.
pub fn verified_writes(
    address: Address,
    bytes: &[u8],
    limits: DebugMemoryLimits,
    packet_bytes: NonZeroU32,
) -> Result<Vec<Vec<SequenceCommand>>> {
    let length = u32::try_from(bytes.len()).map_err(|_| Error::InvalidLength)?;
    MemoryRange::new(
        address,
        NonZeroU32::new(length).ok_or(Error::InvalidLength)?,
    )?;
    let chunk = limits
        .read
        .get()
        .min(limits.write.get())
        .min(packet_bytes.get()) as usize;
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    for (index, piece) in bytes.chunks(chunk).enumerate() {
        let at = Address((u64::from(address.0) + (index * chunk) as u64) as u32);
        let range = MemoryRange::new(
            at,
            NonZeroU32::new(piece.len() as u32).expect("nonempty piece"),
        )?;
        batch.push(SequenceCommand::Write {
            address: at,
            bytes: piece.to_vec(),
        });
        batch.push(SequenceCommand::Read {
            range,
            expected: Some(piece.to_vec()),
        });
        if batch.len() == 512 {
            batches.push(std::mem::take(&mut batch));
        }
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    Ok(batches)
}
