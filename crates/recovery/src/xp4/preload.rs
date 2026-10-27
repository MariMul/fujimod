//! Preloaded executable sections and explicitly located resident data banks.
use super::sd_loader;
use crate::{
    Proof,
    image::{self, Edit, Plan, Result, Source, extent, hash, put, require, word},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resident {
    pub name: String,
    pub offset: usize,
    pub address: Option<usize>,
    pub source: Source,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preload {
    pub address_limit: usize,
    pub enabled_byte: usize,
    pub byte_limit: usize,
    pub byte_reader: Proof,
    pub indexed_reader: Proof,
    pub cache_maintenance: Proof,
    pub discards: Vec<Proof>,
    pub resources: Vec<Proof>,
    pub witnesses: Vec<Proof>,
    pub edits: Vec<Edit>,
    pub placements: Vec<image::Region>,
    pub ranges: Vec<Resident>,
    pub exclusions: Vec<image::Bss>,
}

fn branch(at: usize, target: usize, link: bool) -> Result<u32> {
    let delta = target as i64 - at as i64 - 8;
    require(
        delta % 4 == 0 && (-0x2000000..0x2000000).contains(&delta),
        "Preloaded hook is outside ARM branch range",
    )?;
    Ok((if link { 0xeb000000 } else { 0xea000000 }) | ((delta >> 2) as u32 & 0xffffff))
}

impl Preload {
    pub fn install(
        &self,
        inverted: &[u8],
        plan: &Plan,
        dat_offset: usize,
        ram: &mut Vec<u8>,
    ) -> Result<()> {
        let layout = &sd_loader::LAYOUT.preloaded;
        let program_base = layout.address;
        let program_bytes = extent(sd_loader::IMAGE, layout.offset, layout.size)?;
        require(
            self.ranges.len() <= 16
                && !self.ranges.is_empty()
                && matches!(self.discards.len(), 1 | 3)
                && self.resources.len() == 2,
            "Incomplete preloaded data/transfer map",
        )?;
        require(
            self.byte_limit > 0
                && self.byte_limit <= 0x8000000
                && self.address_limit <= u32::MAX as usize,
            "Invalid resident address bounds",
        )?;
        for view in &self.exclusions {
            require(
                view.address
                    .checked_add(view.size)
                    .is_some_and(|end| end <= self.address_limit),
                "Startup working view exceeds declared RAM",
            )?;
        }
        // Verify every original routine/table before applying any modifications.
        for proof in self
            .witnesses
            .iter()
            .chain([
                &self.byte_reader,
                &self.indexed_reader,
                &self.cache_maintenance,
            ])
            .chain(self.discards.iter())
            .chain(self.resources.iter())
        {
            hash(extent(ram, proof.address, proof.size)?, &proof.sha256)?;
        }
        let mut placed = plan.clone();
        placed.regions.extend(self.placements.clone());
        let mut cursor = ram.len();
        let mut ranges = Vec::new();
        for resident in &self.ranges {
            let size = resident.source.decoded_size;
            require(
                resident.source.size == size
                    && resident.source.initialized_prefix.is_none()
                    && matches!(resident.source.compression, image::Compression::Raw)
                    && dat_offset.checked_add(resident.offset) == Some(resident.source.dat_offset),
                "Resident source does not match its firmware offset",
            )?;
            require(
                !ranges.iter().any(|&(offset, _, amount)| {
                    resident.offset < offset + amount
                        && offset < resident.offset.saturating_add(size)
                }),
                "Resident firmware offsets overlap",
            )?;
            let address = if let Some(address) = resident.address {
                let end = address
                    .checked_add(size)
                    .ok_or_else(|| image::Error::Invalid("Resident placement overflow".into()))?;
                cursor = cursor.max(end);
                address
            } else {
                cursor = (cursor + 0xfff) & !0xfff;
                loop {
                    let end = cursor.checked_add(size).ok_or_else(|| {
                        image::Error::Invalid("Resident placement overflow".into())
                    })?;
                    if let Some(blocked) = self
                        .exclusions
                        .iter()
                        .find(|b| cursor < b.address + b.size && b.address < end)
                    {
                        cursor = (blocked.address + blocked.size + 0xfff) & !0xfff;
                    } else {
                        break;
                    }
                }
                let address = cursor;
                cursor += size;
                address
            };
            require(
                size > 0
                    && address
                        .checked_add(size)
                        .is_some_and(|end| end <= self.address_limit)
                    && resident
                        .offset
                        .checked_add(size)
                        .is_some_and(|end| end <= self.byte_limit + 0x20000),
                "Resident range exceeds declared bounds",
            )?;
            require(
                !self
                    .exclusions
                    .iter()
                    .any(|b| address < b.address + b.size && b.address < address + size),
                "Resident input overlaps a startup working view",
            )?;
            placed.regions.push(image::Region {
                name: resident.name.clone(),
                address,
                source: resident.source.clone(),
            });
            placed.size = placed.size.max(address + size);
            ranges.push((resident.offset, address, size));
        }
        // The image transform checks every source hash and all initialized/BSS
        // intersections, including fixed destinations and resident inputs.
        let mut result = image::transform(inverted, &placed)?;
        require(
            extent(&result, program_base, layout.size)?
                .iter()
                .all(|b| *b == 0),
            "Preloaded program reservation is occupied",
        )?;
        require(
            !placed.bss.iter().any(|b| {
                program_base < b.address + b.size && b.address < program_base + layout.size
            }),
            "Preloaded program overlaps BSS",
        )?;
        let mut program = program_bytes.to_vec();
        let config = layout.symbols["preloaded_config"];
        for (index, value) in [
            self.enabled_byte,
            self.byte_limit,
            ranges.len(),
            self.cache_maintenance.address,
        ]
        .into_iter()
        .enumerate()
        {
            put(
                &mut program,
                config + index * 4,
                u32::try_from(value)
                    .map_err(|_| image::Error::Invalid("Resident config overflows ARM32".into()))?,
            )?;
        }
        for (index, &(offset, address, size)) in ranges.iter().enumerate() {
            for (column, value) in [offset, address, size].into_iter().enumerate() {
                put(
                    &mut program,
                    config + 16 + index * 12 + column * 4,
                    value as u32,
                )?;
            }
        }
        result[program_base..program_base + layout.size].copy_from_slice(&program);
        for site in &self.discards {
            let old = word(&result, site.address)?;
            require(
                site.size == 4 && old >> 24 == 0xeb,
                "Discarded transfer is not a direct call",
            )?;
            let target = site.address as i64 + 8 + (((old & 0xffffff) as i32) << 8 >> 6) as i64;
            require(
                target == self.indexed_reader.address as i64,
                "Discarded transfer calls another reader",
            )?;
            put(
                &mut result,
                site.address,
                branch(
                    site.address,
                    program_base + layout.symbols["preloaded_discard"],
                    true,
                )?,
            )?;
        }
        for proof in &self.resources {
            put(
                &mut result,
                proof.address,
                branch(
                    proof.address,
                    program_base + layout.symbols["preloaded_discard"],
                    false,
                )?,
            )?;
        }
        for (proof, symbol) in [
            (&self.byte_reader, "preloaded_bytes"),
            (&self.indexed_reader, "preloaded_indexed"),
        ] {
            put(
                &mut result,
                proof.address,
                branch(proof.address, program_base + layout.symbols[symbol], false)?,
            )?;
        }
        for edit in &self.edits {
            require(
                word(&result, edit.address)? == edit.old,
                "Resident selection old word differs",
            )?;
            put(&mut result, edit.address, edit.new)?;
        }
        *ram = result;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Preload, Vec<u8>, Plan, Vec<u8>) {
        let mut input = vec![0x5a; 0x21000];
        let mut regions = Vec::new();
        for (index, address) in [0x100, 0x104, 0x108, 0x200, 0x300, 0x400, 0x500, 0x600]
            .into_iter()
            .enumerate()
        {
            let at = index * 4;
            let instruction = if address < 0x200 {
                branch(address, 0x500, true).unwrap()
            } else {
                0xe12fff1e
            };
            put(&mut input, at, instruction).unwrap();
            regions.push(image::Region {
                name: format!("code-{address:x}"),
                address,
                source: Source {
                    dat_offset: at,
                    size: 4,
                    decoded_size: 4,
                    compression: image::Compression::Raw,
                    sha256: fujimod_firmware::sha256(&input[at..at + 4]),
                    initialized_prefix: None,
                },
            });
        }
        let plan = Plan {
            size: 0x200000,
            entry: 0x100,
            regions,
            bss: vec![],
            edits: vec![],
        };
        let ram = image::transform(&input, &plan).unwrap();
        let proof = |address| Proof {
            address,
            size: 4,
            sha256: fujimod_firmware::sha256(&ram[address..address + 4]),
        };
        let preload = Preload {
            address_limit: 0x1000000,
            enabled_byte: 0x800,
            byte_limit: 0x300000,
            byte_reader: proof(0x400),
            indexed_reader: proof(0x500),
            cache_maintenance: proof(0x600),
            discards: [0x100, 0x104, 0x108].into_iter().map(proof).collect(),
            resources: [0x200, 0x300].into_iter().map(proof).collect(),
            witnesses: vec![],
            edits: vec![],
            placements: vec![],
            ranges: vec![Resident {
                name: "selected bank".into(),
                offset: 0x1000,
                address: None,
                source: Source {
                    dat_offset: 0x1000,
                    size: 0x20000,
                    decoded_size: 0x20000,
                    compression: image::Compression::Raw,
                    sha256: fujimod_firmware::sha256(&input[0x1000..]),
                    initialized_prefix: None,
                },
            }],
            exclusions: vec![image::Bss {
                name: "working view".into(),
                address: 0x200000,
                size: 0x10000,
            }],
        };
        (preload, input, plan, ram)
    }

    #[test]
    fn places_only_declared_banks_and_excludes_startup_working_views() {
        let (preload, input, plan, mut ram) = fixture();
        preload.install(&input, &plan, 0, &mut ram).unwrap();
        let config = sd_loader::LAYOUT.preloaded.address
            + sd_loader::LAYOUT.preloaded.symbols["preloaded_config"];
        assert_eq!(word(&ram, config + 8).unwrap(), 1);
        assert_eq!(word(&ram, config + 20).unwrap(), 0x210000);
        assert_eq!(ram.len(), 0x230000);
        assert_eq!(&ram[0x210000..], &input[0x1000..]);
        assert_eq!(word(&ram, 0x100).unwrap() >> 24, 0xeb);
        assert_eq!(word(&ram, 0x200).unwrap() >> 24, 0xea);
        assert_eq!(word(&ram, 0x154).unwrap(), 0);
    }

    #[test]
    fn changed_calls_and_sources_refuse_without_mutating_the_image() {
        let (preload, mut input, plan, mut ram) = fixture();
        input[0x1000] ^= 1;
        let original = ram.clone();
        assert!(preload.install(&input, &plan, 0, &mut ram).is_err());
        assert_eq!(ram, original);
        let (preload, input, plan, mut ram) = fixture();
        ram[0x100] ^= 1;
        let original = ram.clone();
        assert!(preload.install(&input, &plan, 0, &mut ram).is_err());
        assert_eq!(ram, original);
    }

    #[test]
    fn wrong_namespace_bias_and_overlapping_or_escaping_ranges_refuse() {
        let (preload, input, plan, ram) = fixture();
        assert!(preload.install(&input, &plan, 4, &mut ram.clone()).is_err());
        for address in [0x100, 0x200000, usize::MAX] {
            let mut changed = preload.clone();
            changed.ranges[0].address = Some(address);
            assert!(changed.install(&input, &plan, 0, &mut ram.clone()).is_err());
        }
        let mut changed = preload.clone();
        changed.ranges.push(changed.ranges[0].clone());
        assert!(changed.install(&input, &plan, 0, &mut ram.clone()).is_err());
    }
}
