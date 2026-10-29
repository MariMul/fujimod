//! Loader footer validation against the update supplied on the SD card.
use crate::{
    Proof,
    image::{self, Plan, Result, Source, extent, put, require, word},
    rom_sd::call,
};
use serde::{Deserialize, Serialize};

const PROGRAM: &[u8] = include_bytes!("../../assets/footer-reader.generated.bin");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoaderFooter {
    pub check: Proof,
    pub read_call: usize,
    pub read_target: usize,
    pub flash_offset: u32,
    pub source: Source,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    schema: usize,
    address: usize,
    size: usize,
    entry: usize,
    config: usize,
}

impl LoaderFooter {
    pub(crate) fn install(&self, inverted: &[u8], plan: &Plan, ram: &mut [u8]) -> Result<()> {
        let layout: Layout =
            serde_json::from_str(include_str!("../../assets/footer-reader.generated.json"))
                .map_err(|e| image::Error::Invalid(e.to_string()))?;
        require(
            layout.schema == 1
                && layout.address == 0x102000
                && layout.size == PROGRAM.len()
                && layout.config + 36 == PROGRAM.len()
                && layout.entry < layout.config,
            "Invalid loader footer reader layout",
        )?;
        image::hash(
            extent(ram, self.check.address, self.check.size)?,
            &self.check.sha256,
        )?;
        require(
            self.read_call >= self.check.address + 12
                && self
                    .read_call
                    .checked_add(4)
                    .is_some_and(|end| end <= self.check.address + self.check.size)
                && call(ram, self.read_call)? == self.read_target,
            "Loader footer read call differs",
        )?;
        let mov = |base, value| base | (value & 0xfff) | ((value & 0xf000) << 4);
        require(
            word(ram, self.read_call - 12)? == mov(0xe3000000, self.flash_offset & 0xffff)
                && word(ram, self.read_call - 8)? == mov(0xe3400000, self.flash_offset >> 16),
            "Loader footer offset differs",
        )?;
        let header = self.source.read(inverted)?;
        require(
            self.flash_offset != 0
                && header.len() == 32
                && word(&header, 0)? == self.flash_offset
                && word(&header, 4)? == self.flash_offset
                && word(&header, 8)? == 32
                && word(&header, 28)? == 0xaaaaffff,
            "Update loader footer header differs",
        )?;
        let end = layout.address + PROGRAM.len();
        require(
            plan.regions
                .iter()
                .all(|r| end <= r.address || r.address + r.source.decoded_size <= layout.address)
                && plan
                    .bss
                    .iter()
                    .all(|r| end <= r.address || r.address + r.size <= layout.address)
                && extent(ram, layout.address, PROGRAM.len())?
                    .iter()
                    .all(|b| *b == 0),
            "Loader footer reader reservation is occupied",
        )?;
        let delta = (layout.address + layout.entry) as i64 - self.read_call as i64 - 8;
        require(
            delta % 4 == 0 && (-0x2000000..0x2000000).contains(&delta),
            "Loader footer reader BL exceeds ARM range",
        )?;
        let mut program = PROGRAM.to_vec();
        put(&mut program, layout.config, self.flash_offset)?;
        program[layout.config + 4..layout.config + 36].copy_from_slice(&header);
        ram[layout.address..end].copy_from_slice(&program);
        put(
            ram,
            self.read_call,
            0xeb000000 | ((delta >> 2) as u32 & 0xffffff),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Compression, Region};

    fn fixture() -> (LoaderFooter, Plan, Vec<u8>, Vec<u8>) {
        let mut header = vec![0; 32];
        for (at, value) in [(0, 0x28a0000), (4, 0x28a0000), (8, 32), (28, 0xaaaaffff)] {
            put(&mut header, at, value).unwrap();
        }
        let mut ram = vec![0; 0x104000];
        for (at, value) in [
            (0x1004, 0xe3000000),
            (0x1008, 0xe340028a),
            (0x1010, 0xeb0003fa),
        ] {
            put(&mut ram, at, value).unwrap();
        }
        let source = Source {
            dat_offset: 0,
            size: 32,
            compression: Compression::Raw,
            decoded_size: 32,
            sha256: fujimod_firmware::sha256(&header),
            initialized_prefix: None,
        };
        let footer = LoaderFooter {
            check: Proof {
                address: 0x1000,
                size: 32,
                sha256: fujimod_firmware::sha256(&ram[0x1000..0x1020]),
            },
            read_call: 0x1010,
            read_target: 0x2000,
            flash_offset: 0x28a0000,
            source: source.clone(),
        };
        let plan = Plan {
            size: ram.len(),
            entry: 0,
            bss: vec![],
            edits: vec![],
            regions: vec![Region {
                name: "loader".into(),
                address: 0x1000,
                source,
            }],
        };
        (footer, plan, header, ram)
    }

    #[test]
    fn installs_only_the_checked_read_call_and_rejects_changed_inputs_atomically() {
        let (footer, plan, header, ram) = fixture();
        let mut result = ram.clone();
        footer.install(&header, &plan, &mut result).unwrap();
        assert_eq!(&result[0x1000..0x1010], &ram[0x1000..0x1010]);
        assert_eq!(&result[0x1014..0x1020], &ram[0x1014..0x1020]);
        assert_ne!(word(&result, 0x1010).unwrap(), word(&ram, 0x1010).unwrap());
        for at in [0x1004, 0x1008, 0x1010, 0x102000] {
            let mut changed = ram.clone();
            changed[at] ^= 1;
            let before = changed.clone();
            assert!(footer.install(&header, &plan, &mut changed).is_err());
            assert_eq!(changed, before);
        }
        let mut altered = footer.clone();
        altered.flash_offset += 1;
        assert!(altered.install(&header, &plan, &mut ram.clone()).is_err());
        let mut bad = header.clone();
        bad[30] = 0;
        let mut altered = footer.clone();
        altered.source.sha256 = fujimod_firmware::sha256(&bad);
        assert!(altered.install(&bad, &plan, &mut ram.clone()).is_err());
        let mut altered = footer.clone();
        altered.read_target += 4;
        assert!(altered.install(&header, &plan, &mut ram.clone()).is_err());
    }
}
