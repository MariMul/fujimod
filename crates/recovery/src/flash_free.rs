//! Recorded qualification of the exact card files with destroyed internal storage.
use crate::{
    Recipe,
    image::{self, File, Result, require},
};
use serde::{Deserialize, Serialize};

pub const MISSING: &str =
    "flash-free SD updater proof (zeroed and scrambled update-covered storage)";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub name: String,
    pub size: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoragePattern {
    Zeroed,
    Scrambled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageRange {
    pub offset: u64,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub storage: StoragePattern,
    pub destroyed_ranges: Vec<StorageRange>,
    pub entire_internal_storage: bool,
    pub receipt_sha256: String,
    pub instructions: u64,
    pub from_rom_reset: bool,
    pub sd_boot_and_disp_back: bool,
    pub updater_ready_for_dat: bool,
    pub internal_code_bytes_read: u64,
    pub internal_update_data_bytes_read: u64,
    pub unmodeled_accesses: u64,
    pub interventions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlashFreeProof {
    pub construction_sha256: String,
    pub boot_rom_sha256: String,
    pub emulator_sha256: String,
    pub configuration_sha256: String,
    pub complete_internal_storage_audit: bool,
    pub update_covered_ranges: Vec<StorageRange>,
    pub files: Vec<FileIdentity>,
    pub runs: Vec<Run>,
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl FlashFreeProof {
    pub fn verify(&self, recipe: &Recipe) -> Result<()> {
        require(
            self.complete_internal_storage_audit,
            "Incomplete internal-storage audit",
        )?;
        require(
            !self.update_covered_ranges.is_empty()
                && self
                    .update_covered_ranges
                    .iter()
                    .all(|r| r.size > 0 && r.offset.checked_add(r.size).is_some()),
            "Missing update-covered storage extents",
        )?;
        require(
            [
                &self.construction_sha256,
                &self.boot_rom_sha256,
                &self.emulator_sha256,
                &self.configuration_sha256,
            ]
            .into_iter()
            .all(|s| digest(s)),
            "Invalid flash-free proof identity",
        )?;
        require(
            self.construction_sha256 == construction_sha256(recipe)?,
            "Stale flash-free construction proof",
        )?;
        require(
            self.runs.len() == 2
                && self.runs[0].storage == StoragePattern::Zeroed
                && self.runs[1].storage == StoragePattern::Scrambled,
            "Flash-free proof requires both storage patterns",
        )?;
        for run in &self.runs {
            require(
                run.destroyed_ranges
                    .iter()
                    .all(|r| r.size > 0 && r.offset.checked_add(r.size).is_some())
                    && self.update_covered_ranges.iter().all(|covered| {
                        run.destroyed_ranges.iter().any(|destroyed| {
                            destroyed.offset <= covered.offset
                                && destroyed.offset + destroyed.size
                                    >= covered.offset + covered.size
                        })
                    }),
                "Flash-free execution leaves update-covered storage intact",
            )?;
            require(
                digest(&run.receipt_sha256)
                    && run.instructions > 0
                    && run.from_rom_reset
                    && run.sd_boot_and_disp_back
                    && run.updater_ready_for_dat
                    && run.internal_code_bytes_read == 0
                    && run.internal_update_data_bytes_read == 0
                    && run.unmodeled_accesses == 0
                    && run.interventions.is_empty(),
                "Unqualified flash-free updater execution",
            )?;
        }
        require(!self.files.is_empty(), "Missing qualified card files")?;
        let mut names = std::collections::BTreeSet::new();
        for file in &self.files {
            File::new(&file.name, vec![])?;
            require(
                file.size > 0
                    && digest(&file.sha256)
                    && names.insert(file.name.to_ascii_lowercase()),
                "Invalid qualified card file",
            )?;
        }
        require(
            self.files.iter().any(|f| {
                f.name == recipe.update_filename
                    && f.size == recipe.source_size
                    && f.sha256 == recipe.source_sha256
            }),
            "Flash-free proof omits the official DAT",
        )?;
        Ok(())
    }

    pub fn verify_files(&self, files: &[File]) -> Result<()> {
        require(
            files.len() == self.files.len(),
            "Qualified card file count differs",
        )?;
        for file in files {
            let identity = self
                .files
                .iter()
                .find(|f| f.name == file.name)
                .ok_or_else(|| image::Error::Invalid("Unqualified card filename".into()))?;
            require(
                identity.size == file.bytes.len(),
                "Qualified card file size differs",
            )?;
            image::hash(&file.bytes, &identity.sha256)?;
        }
        Ok(())
    }
}

/// Hash all construction and input facts, excluding qualification and diagnostics.
pub fn construction_sha256(recipe: &Recipe) -> Result<String> {
    let mut value =
        serde_json::to_value(recipe).map_err(|e| image::Error::Invalid(e.to_string()))?;
    let object = value.as_object_mut().unwrap();
    object.remove("flash_free");
    object.remove("missing");
    let bytes = serde_json::to_vec(&value).map_err(|e| image::Error::Invalid(e.to_string()))?;
    Ok(fujimod_firmware::sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof(recipe: &Recipe) -> FlashFreeProof {
        FlashFreeProof {
            construction_sha256: construction_sha256(recipe).unwrap(),
            boot_rom_sha256: "1".repeat(64),
            emulator_sha256: "2".repeat(64),
            configuration_sha256: "3".repeat(64),
            complete_internal_storage_audit: true,
            update_covered_ranges: vec![StorageRange {
                offset: 0,
                size: 4096,
            }],
            files: vec![FileIdentity {
                name: recipe.update_filename.clone(),
                size: recipe.source_size,
                sha256: recipe.source_sha256.clone(),
            }],
            runs: [StoragePattern::Zeroed, StoragePattern::Scrambled]
                .into_iter()
                .map(|storage| Run {
                    storage,
                    destroyed_ranges: vec![StorageRange {
                        offset: 0,
                        size: 4096,
                    }],
                    entire_internal_storage: false,
                    receipt_sha256: "4".repeat(64),
                    instructions: 100,
                    from_rom_reset: true,
                    sd_boot_and_disp_back: true,
                    updater_ready_for_dat: true,
                    internal_code_bytes_read: 0,
                    internal_update_data_bytes_read: 0,
                    unmodeled_accesses: 0,
                    interventions: vec![],
                })
                .collect(),
        }
    }

    #[test]
    fn every_generation_refuses_recovery_without_its_own_proof() {
        for id in [
            "xf1-101",
            "xpro1-382",
            "x70-110",
            "xpro2-511",
            "xt3-511",
            "xm5-130",
        ] {
            let mut recipe = Recipe::bundled(id).unwrap().unwrap();
            recipe.missing.clear();
            recipe.flash_free = None;
            assert!(recipe.missing_facts().iter().any(|s| s == MISSING), "{id}");
            assert!(
                recipe.build(&[]).unwrap_err().to_string().contains(MISSING),
                "{id}"
            );
            assert!(!crate::availability(id).unwrap().can_build, "{id}");
        }
    }

    #[test]
    fn stale_partial_or_intervened_execution_cannot_qualify() {
        let mut recipe = Recipe::bundled("x70-110").unwrap().unwrap();
        let qualified = proof(&recipe);
        qualified.verify(&recipe).unwrap();
        for field in 0..12 {
            let mut bad = qualified.clone();
            match field {
                0 => {
                    bad.runs.pop();
                }
                1 => bad.runs[1].storage = StoragePattern::Zeroed,
                2 => bad.runs[0].from_rom_reset = false,
                3 => bad.runs[0].sd_boot_and_disp_back = false,
                4 => bad.runs[0].updater_ready_for_dat = false,
                5 => bad.runs[0].internal_code_bytes_read = 4,
                6 => bad.runs[0].internal_update_data_bytes_read = 4,
                7 => bad.runs[0].unmodeled_accesses = 1,
                8 => bad.runs[0].interventions.push("callee substitution".into()),
                9 => bad.runs[0].receipt_sha256.clear(),
                10 => bad.complete_internal_storage_audit = false,
                11 => bad.runs[0].destroyed_ranges[0].size -= 1,
                _ => unreachable!(),
            }
            assert!(bad.verify(&recipe).is_err(), "field {field}");
        }
        recipe.plan.as_mut().unwrap().edits[0].new ^= 1;
        assert!(qualified.verify(&recipe).is_err());
        recipe.flash_free = Some(qualified);
        assert!(recipe.missing_facts().iter().any(|s| s == MISSING));
    }

    #[test]
    fn qualification_binds_every_emitted_byte_and_filename() {
        let recipe = Recipe::bundled("x70-110").unwrap().unwrap();
        let mut proof = proof(&recipe);
        let file = File::new("ffboot.bin", vec![1, 2, 3, 4]).unwrap();
        proof.files = vec![FileIdentity {
            name: file.name.clone(),
            size: file.bytes.len(),
            sha256: fujimod_firmware::sha256(&file.bytes),
        }];
        proof.verify_files(std::slice::from_ref(&file)).unwrap();
        assert!(proof.verify_files(&[]).is_err());
        assert!(
            proof
                .verify_files(&[File::new("ffboot.bin", vec![1, 2, 3, 5]).unwrap()])
                .is_err()
        );
        assert!(
            proof
                .verify_files(&[File::new("ffboot.bin", vec![1]).unwrap()])
                .is_err()
        );
        assert!(
            proof
                .verify_files(&[File::new("other.bin", file.bytes).unwrap()])
                .is_err()
        );
    }
}
