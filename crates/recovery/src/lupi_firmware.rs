//! Checked installation of the ROM-file-backed 16 KiB firmware namespace.
use crate::{
    Proof,
    image::{self, Plan, Result, extent, hash, put, require},
};
use serde::{Deserialize, Serialize};
mod template {
    include!("lupi_firmware.generated.rs");
}
const BASE: usize = 0x5000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transport {
    pub replaced_loader_text: Proof,
    pub indexed_reader: Proof,
    pub dat_offset: usize,
    pub suppressed_loader_calls: Vec<usize>,
}
impl Transport {
    pub(super) fn install(
        &self,
        api: &super::lupi::Lupi,
        inverted: &[u8],
        plan: &Plan,
        ram: &mut [u8],
    ) -> Result<Vec<u8>> {
        let slot = &self.replaced_loader_text;
        require(
            slot.address == BASE && slot.size == 0x1000 && template::PROGRAM.len() <= slot.size,
            "Lupi firmware reader reservation differs",
        )?;
        hash(extent(ram, slot.address, slot.size)?, &slot.sha256)?;
        hash(
            extent(ram, self.indexed_reader.address, self.indexed_reader.size)?,
            &self.indexed_reader.sha256,
        )?;
        require(
            self.indexed_reader.size >= 4 && self.indexed_reader.address & 3 == 0,
            "Invalid Lupi indexed reader proof",
        )?;
        require(
            self.suppressed_loader_calls.len() == 8
                && self
                    .suppressed_loader_calls
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == 8
                && self.suppressed_loader_calls.iter().all(|at| {
                    plan.edits.iter().any(|edit| {
                        edit.address == *at
                            && edit.new == 0xe3a00000
                            && edit.old >> 24 == 0xeb
                            && *at < 0x40000
                            && image::word(ram, *at).ok() == Some(0xe3a00000)
                    })
                }),
            "Lupi NAND reader text still has an unsuppressed loader dependency",
        )?;
        require(
            plan.regions.iter().any(|region| {
                region.address == 0 && region.source.decoded_size >= BASE + slot.size
            }) && plan.bss.iter().all(|region| {
                region.address + region.size <= BASE || region.address >= BASE + slot.size
            }),
            "Lupi firmware reader overlaps loader BSS or escapes loaded text",
        )?;
        let source = inverted
            .get(self.dat_offset..)
            .ok_or_else(|| image::Error::Invalid("Lupi firmware DAT bias escapes source".into()))?;
        require(
            !source.is_empty() && source.len() <= 0x4000000,
            "Lupi firmware namespace exceeds its 16 KiB index",
        )?;
        let capacity = (source.len() + 0x3fff) & !0x3fff;
        let mut program = template::PROGRAM.to_vec();
        for (index, value) in [
            api.open as usize,
            api.read as usize,
            api.filesystem as usize,
            source.len(),
            capacity,
        ]
        .into_iter()
        .enumerate()
        {
            put(
                &mut program,
                template::LUPI_FIRMWARE_CONFIG - BASE + index * 4,
                u32::try_from(value).map_err(|_| {
                    image::Error::Invalid("Lupi firmware config exceeds ARM32".into())
                })?,
            )?;
        }
        let delta = template::LUPI_FIRMWARE_INDEXED as i64 - self.indexed_reader.address as i64 - 8;
        require(
            delta % 4 == 0 && (-0x2000000..0x2000000).contains(&delta),
            "Lupi indexed hook exceeds ARM branch range",
        )?;
        ram[BASE..BASE + program.len()].copy_from_slice(&program);
        put(
            ram,
            self.indexed_reader.address,
            0xea000000 | ((delta >> 2) as u32 & 0xffffff),
        )?;
        Ok(source.to_vec())
    }
}
