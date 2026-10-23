//! Declarative external NAND inputs shared by native and browser frontends.
//! Preparation verifies declared asset windows before constructing a medium.
use super::inputs::{PreparePersistentInputs, tag_managed_blocks};
use crate::parts::nand::{Nand, Options, Overlay};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NandInput {
    Committed {
        options: Options,
        base: String,
        image: String,
    },
    Raw {
        options: Options,
        base: String,
        overlays: Vec<(u64, String)>,
        spare: Vec<(u32, String)>,
    },
    Catalog {
        options: Options,
        definition: fujimod_catalog::persistent_inputs::PersistentInputs,
        set: String,
        base: String,
        assets: BTreeMap<String, String>,
    },
    /// A settings bank built from the update file, with the requested named
    /// settings, placed in its stock flash blocks before cold boot.
    Settings {
        options: Options,
        base: String,
        update: String,
        installation: fujimod_catalog::reference::FirmwareInstallation,
        layout: fujimod_catalog::settings::FirmwareSettings,
        requested: BTreeMap<String, String>,
    },
}
impl NandInput {
    pub fn build(self, loaded: &BTreeMap<String, Vec<u8>>) -> Result<Nand, String> {
        let asset = |name: &str| {
            loaded
                .get(name)
                .ok_or_else(|| format!("Missing NAND asset {name}"))
        };
        match self {
            Self::Committed {
                options,
                base,
                image,
            } => Nand::new(
                options,
                Arc::from(asset(&base)?.as_slice()),
                vec![],
                BTreeMap::new(),
            )?
            .restore_committed(asset(&image)?),
            Self::Raw {
                options,
                base,
                overlays,
                spare,
            } => Nand::new(
                options,
                Arc::from(asset(&base)?.as_slice()),
                overlays
                    .into_iter()
                    .map(|(offset, name)| {
                        Ok(Overlay {
                            offset,
                            bytes: Arc::from(asset(&name)?.as_slice()),
                        })
                    })
                    .collect::<Result<_, String>>()?,
                spare
                    .into_iter()
                    .try_fold(BTreeMap::new(), |mut tags, (row, name)| {
                        if tags.insert(row, asset(&name)?.clone()).is_some() {
                            return Err("Duplicate initial NAND spare row".to_owned());
                        }
                        Ok(tags)
                    })?,
            ),
            Self::Catalog {
                options,
                definition,
                set,
                base,
                assets,
            } => {
                let windows = assets
                    .into_iter()
                    .map(|(name, source)| Ok((name, asset(&source)?.clone())))
                    .collect::<Result<_, String>>()?;
                definition.prepare(options, &set, &windows, Arc::from(asset(&base)?.as_slice()))
            }
            Self::Settings {
                options,
                base,
                update,
                installation,
                layout,
                requested,
            } => {
                let mut source = fujimod_settings::Update::new(asset(&update)?, &installation);
                let bank = fujimod_settings::build(&layout, &mut source, &requested)?;
                let block_bytes = options.page_bytes as u64 * options.pages_per_block as u64;
                let mut bytes = vec![0xff; (u64::from(bank.blocks) * block_bytes) as usize];
                let content = bytes
                    .get_mut(..bank.bytes.len())
                    .ok_or("Settings bank exceeds its flash blocks")?;
                content.copy_from_slice(&bank.bytes);
                let offset = u64::from(bank.first_block) * block_bytes;
                if offset + bytes.len() as u64 > options.bytes {
                    return Err("Settings bank exceeds NAND geometry".into());
                }
                let mut spare = BTreeMap::new();
                tag_managed_blocks(
                    &options,
                    offset,
                    bytes.len() as u64,
                    bank.logical_first,
                    &mut spare,
                )?;
                Nand::new(
                    options,
                    Arc::from(asset(&base)?.as_slice()),
                    vec![Overlay {
                        offset,
                        bytes: Arc::from(bytes),
                    }],
                    spare,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn committed_nand_input_keeps_program_erase_and_base_identity() {
        let options = Options {
            bytes: 262144,
            page_bytes: 2048,
            pages_per_block: 64,
            spare_bytes: 64,
        };
        let base = vec![0xa5; 2048];
        let mut medium = Nand::new(
            options.clone(),
            Arc::from(base.as_slice()),
            vec![],
            BTreeMap::new(),
        )
        .unwrap();
        medium.erase(0).unwrap();
        medium
            .program(64, &vec![0x85; 2048], &vec![0x12; 64])
            .unwrap();
        let mut assets = BTreeMap::from([
            ("base".into(), base),
            ("saved".into(), medium.committed_image().unwrap()),
        ]);
        let spec =
            serde_json::json!({"kind":"committed","options":options,"base":"base","image":"saved"});
        let restored = serde_json::from_value::<NandInput>(spec.clone())
            .unwrap()
            .build(&assets)
            .unwrap();
        assert_eq!(restored.fingerprint(), medium.fingerprint());
        let mut bytes = [0; 2048];
        restored.read(0, &mut bytes).unwrap();
        assert_eq!(bytes, [255; 2048]);
        assets.insert("base".into(), vec![0; 2048]);
        assert!(
            serde_json::from_value::<NandInput>(spec)
                .unwrap()
                .build(&assets)
                .err()
                .unwrap()
                .contains("identity")
        );
    }

    #[test]
    fn declared_asset_names_build_the_same_owned_medium_without_a_frontend_model() {
        let spec = serde_json::json!({"kind":"raw", "options":{
            "bytes":262144,"page_bytes":2048,"pages_per_block":64,"spare_bytes":64},
            "base":"update","overlays":[[131072,"settings"]],"spare":[[64,"tag"]]});
        let inputs = BTreeMap::from([
            ("update".into(), vec![255; 2048]),
            ("settings".into(), vec![0x85; 2048]),
            ("tag".into(), vec![0x12, 0x34]),
        ]);
        let medium = serde_json::from_value::<NandInput>(spec.clone())
            .unwrap()
            .build(&inputs)
            .unwrap();
        let mut data = [0; 2048];
        medium.read(131072, &mut data).unwrap();
        assert_eq!(data, [0x85; 2048]);
        let mut tag = [0; 64];
        medium.read_spare(64, &mut tag).unwrap();
        assert_eq!(&tag[..2], &[0x12, 0x34]);
        assert!(tag[2..].iter().all(|b| *b == 255));
        let mut duplicate = spec.clone();
        duplicate["spare"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!([64, "tag"]));
        assert!(
            serde_json::from_value::<NandInput>(duplicate)
                .unwrap()
                .build(&inputs)
                .is_err()
        );
        let mut missing = inputs.clone();
        missing.remove("settings");
        assert!(
            serde_json::from_value::<NandInput>(spec)
                .unwrap()
                .build(&missing)
                .err()
                .unwrap()
                .contains("settings")
        );
    }
}

/// Sparse sectors supplied as external media, never synthesized file results.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CardInput {
    Sparse {
        options: crate::parts::sd_card::Options,
        sectors: Vec<(u32, String)>,
    },
    Committed {
        image: String,
    },
}
impl CardInput {
    pub fn build(
        self,
        loaded: &BTreeMap<String, Vec<u8>>,
    ) -> Result<crate::parts::sd_card::Card, String> {
        let (options, sectors) = match self {
            Self::Committed { image } => {
                let bytes = loaded.get(&image).ok_or("Missing committed card asset")?;
                let card: crate::parts::sd_card::Card =
                    serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                card.validate()?;
                return Ok(card);
            }
            Self::Sparse { options, sectors } => (options, sectors),
        };
        let mut card = crate::parts::sd_card::Card::new(options)?;
        let mut seen = std::collections::BTreeSet::new();
        for (lba, source) in sectors {
            if !seen.insert(lba) {
                return Err("Duplicate initial card sector".into());
            }
            let bytes = loaded.get(&source).ok_or("Missing initial card sector")?;
            if bytes.len() != 512 {
                return Err("Initial card input must contain whole individual sectors".into());
            }
            card.write_bytes(lba as u64 * 512, bytes)?;
        }
        Ok(card)
    }
}

#[cfg(test)]
mod card_tests {
    use super::*;
    #[test]
    fn committed_card_input_preserves_revision_and_sparse_allocation_bound() {
        let mut card = crate::parts::sd_card::Card::new(crate::parts::sd_card::Options {
            sectors: 0x800000,
            max_dirty_sectors: 2,
        })
        .unwrap();
        card.write_bytes(512, &[0x85; 512]).unwrap();
        card.write_bytes(512, &[0x42; 512]).unwrap();
        let mut assets = BTreeMap::from([("saved".into(), serde_json::to_vec(&card).unwrap())]);
        let spec = serde_json::json!({"kind":"committed","image":"saved"});
        let restored = serde_json::from_value::<CardInput>(spec.clone())
            .unwrap()
            .build(&assets)
            .unwrap();
        assert_eq!(restored.fingerprint(), card.fingerprint());
        let mut invalid = serde_json::to_value(&card).unwrap();
        invalid["max_dirty"] = serde_json::json!(0);
        assets.insert("saved".into(), serde_json::to_vec(&invalid).unwrap());
        assert!(
            serde_json::from_value::<CardInput>(spec)
                .unwrap()
                .build(&assets)
                .is_err()
        );
    }

    #[test]
    fn sparse_card_assets_require_whole_distinct_sectors() {
        let spec = serde_json::json!({"kind":"sparse","options":{"sectors":0x800000,"max_dirty_sectors":8},
            "sectors":[[7,"payload"]]});
        let assets = BTreeMap::from([("payload".into(), vec![0x85; 512])]);
        let card = serde_json::from_value::<CardInput>(spec.clone())
            .unwrap()
            .build(&assets)
            .unwrap();
        assert_eq!(card.sector(7).unwrap(), [0x85; 512]);
        let mut duplicate = spec.clone();
        duplicate["sectors"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!([7, "payload"]));
        assert!(
            serde_json::from_value::<CardInput>(duplicate)
                .unwrap()
                .build(&assets)
                .is_err()
        );
        let bad = BTreeMap::from([("payload".into(), vec![0x85; 511])]);
        assert!(
            serde_json::from_value::<CardInput>(spec)
                .unwrap()
                .build(&bad)
                .is_err()
        );
    }
}
