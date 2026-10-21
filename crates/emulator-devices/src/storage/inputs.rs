//! Host preparation of declared persistent inputs. Asset identity, placement,
//! byte edits and journal layout come from firmware JSON. No guest execution,
//! camera transport or capture-pipeline module participates in preparation.
use crate::parts::nand::{Nand, Options as Geometry, Overlay, managed_tag};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub offset: usize,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub bytes: usize,
    pub sha256: String,
    pub normalized_sha256: String,
    pub patches: Vec<Patch>,
}
impl Asset {
    /// The loader supplies only this declared window. Both raw captures and
    /// already-normalized browser bundle windows are accepted by identity.
    pub fn prepare(&self, input: &[u8]) -> Result<Vec<u8>, String> {
        if input.len() != self.bytes {
            return Err("Persistent input window length differs".into());
        }
        let digest = format!("{:x}", Sha256::digest(input));
        let mut bytes = input.to_vec();
        if digest == self.sha256 {
            for patch in &self.patches {
                let end = patch
                    .offset
                    .checked_add(patch.bytes.len())
                    .ok_or("Persistent patch extent overflow")?;
                bytes
                    .get_mut(patch.offset..end)
                    .ok_or("Persistent patch exceeds input")?
                    .copy_from_slice(&patch.bytes);
            }
        } else if digest != self.normalized_sha256 {
            return Err("Persistent input SHA-256 differs".into());
        }
        if format!("{:x}", Sha256::digest(&bytes)) != self.normalized_sha256 {
            return Err("Normalized persistent input SHA-256 differs".into());
        }
        Ok(bytes)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub asset: String,
    pub source_offset: usize,
    pub bytes: usize,
    pub physical_offset: u64,
    pub managed_index: Option<u16>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalAppend {
    pub record_asset: String,
    pub payload_asset: String,
    pub payload_offset: usize,
    pub payload_bytes: usize,
    pub record_payload_offset: usize,
    pub parity_offset: usize,
    pub checksum_offset: usize,
    pub physical_offset: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSet {
    pub placements: Vec<Placement>,
    pub append: Option<JournalAppend>,
    pub mcu_packet: Option<Packet>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Packet {
    Bytes {
        bytes: Vec<u8>,
    },
    Window {
        asset: String,
        offset: usize,
        bytes: usize,
    },
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub geometry: Geometry,
    pub assets: BTreeMap<String, Asset>,
    pub sets: BTreeMap<String, InputSet>,
}
impl Catalog {
    pub fn mcu_packet(
        &self,
        set: &str,
        loaded: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Option<[u8; 16]>, String> {
        let set = self.sets.get(set).ok_or("Unknown persistent input set")?;
        let bytes = match &set.mcu_packet {
            None => return Ok(None),
            Some(Packet::Bytes { bytes }) => bytes.clone(),
            Some(Packet::Window {
                asset,
                offset,
                bytes,
            }) => {
                let prepared = self
                    .assets
                    .get(asset)
                    .ok_or("Unknown MCU asset")?
                    .prepare(loaded.get(asset).ok_or("Missing MCU asset")?)?;
                prepared
                    .get(*offset..offset.checked_add(*bytes).ok_or("MCU packet overflow")?)
                    .ok_or("MCU packet exceeds input")?
                    .to_vec()
            }
        };
        let packet: [u8; 16] = bytes
            .try_into()
            .map_err(|_| "MCU packet must contain sixteen bytes")?;
        if packet[14] != 0xaa || packet.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != 0 {
            return Err("MCU packet framing differs".into());
        }
        Ok(Some(packet))
    }
    /// Loaded values are bounded source windows, not whole capture files.
    pub fn prepare(
        &self,
        set: &str,
        loaded: &BTreeMap<String, Vec<u8>>,
        base: Arc<[u8]>,
    ) -> Result<Nand, String> {
        let set = self.sets.get(set).ok_or("Unknown persistent input set")?;
        Nand::new(
            self.geometry.clone(),
            base.clone(),
            Vec::new(),
            BTreeMap::new(),
        )?;
        let mut assets = BTreeMap::new();
        for name in set.placements.iter().map(|p| p.asset.as_str()).chain(
            set.append
                .iter()
                .flat_map(|a| [a.record_asset.as_str(), a.payload_asset.as_str()]),
        ) {
            if !assets.contains_key(name) {
                assets.insert(
                    name.to_owned(),
                    self.assets
                        .get(name)
                        .ok_or("Unknown persistent asset")?
                        .prepare(loaded.get(name).ok_or("Missing persistent asset")?)?,
                );
            }
        }
        let mut overlays = Vec::new();
        let mut spare = BTreeMap::new();
        let block_bytes = self.geometry.page_bytes as u64 * self.geometry.pages_per_block as u64;
        for p in &set.placements {
            if p.physical_offset
                .checked_add(p.bytes as u64)
                .is_none_or(|end| end > self.geometry.bytes)
            {
                return Err("Persistent placement exceeds NAND geometry".into());
            }
            let data = assets[&p.asset]
                .get(
                    p.source_offset
                        ..p.source_offset
                            .checked_add(p.bytes)
                            .ok_or("Placement extent overflow")?,
                )
                .ok_or("Placement exceeds input window")?;
            overlays.push(Overlay {
                offset: p.physical_offset,
                bytes: Arc::from(data),
            });
            if let Some(index) = p.managed_index {
                if p.physical_offset % block_bytes != 0
                    || !(p.bytes as u64).is_multiple_of(block_bytes)
                {
                    return Err("Managed NAND input must cover whole blocks".into());
                }
                for n in 0..p.bytes as u64 / block_bytes {
                    let logical = index
                        .checked_add(n.try_into().map_err(|_| "Managed tag count overflow")?)
                        .ok_or("Managed tag index overflow")?;
                    let tag = managed_tag(logical)?.to_vec();
                    let first = ((p.physical_offset / block_bytes + n)
                        * self.geometry.pages_per_block as u64)
                        as u32;
                    spare.insert(first, tag.clone());
                    spare.insert(first + self.geometry.pages_per_block - 1, tag);
                }
            }
        }
        if let Some(a) = &set.append {
            let payload = assets[&a.payload_asset]
                .get(
                    a.payload_offset
                        ..a.payload_offset
                            .checked_add(a.payload_bytes)
                            .ok_or("Journal payload overflow")?,
                )
                .ok_or("Journal payload exceeds input")?;
            let record = append_record(&assets[&a.record_asset], payload, a)?;
            let record_end = a
                .physical_offset
                .checked_add(record.len() as u64)
                .ok_or("Journal placement overflow")?;
            let mut found = false;
            for overlay in &mut overlays {
                if a.physical_offset >= overlay.offset
                    && record_end <= overlay.offset + overlay.bytes.len() as u64
                {
                    let at = (a.physical_offset - overlay.offset) as usize;
                    let bytes = Arc::make_mut(&mut overlay.bytes);
                    if bytes[at..at + record.len()].iter().any(|b| *b != 255) {
                        return Err("Journal append slot is not erased".into());
                    }
                    bytes[at..at + record.len()].copy_from_slice(&record);
                    found = true;
                    break;
                }
            }
            if !found {
                return Err("Journal append does not fit declared placement".into());
            }
        }
        Nand::new(self.geometry.clone(), base, overlays, spare)
    }
}
/// Stock journal format: increment LE16 sequence; one parity bit per LE32
/// data word plus an XOR checksum. Layout is supplied by the firmware data.
fn append_record(record: &[u8], payload: &[u8], layout: &JournalAppend) -> Result<Vec<u8>, String> {
    let parity_bytes = (layout.parity_offset / 4).div_ceil(8);
    if record.len() < 2
        || layout.record_payload_offset < 2
        || !layout.parity_offset.is_multiple_of(4)
        || layout.parity_offset > record.len()
        || layout
            .record_payload_offset
            .checked_add(payload.len())
            .is_none_or(|e| e > layout.parity_offset)
        || layout
            .parity_offset
            .checked_add(parity_bytes)
            .is_none_or(|e| e > layout.checksum_offset)
        || layout.checksum_offset.checked_add(4) != Some(record.len())
    {
        return Err("Invalid journal record layout".into());
    }
    let mut result = record.to_vec();
    let sequence = u16::from_le_bytes(result[..2].try_into().unwrap()).wrapping_add(1);
    result[..2].copy_from_slice(&sequence.to_le_bytes());
    result[layout.record_payload_offset..layout.record_payload_offset + payload.len()]
        .copy_from_slice(payload);
    result[layout.parity_offset..].fill(0);
    let mut checksum = 0u32;
    for word in 0..layout.parity_offset / 4 {
        let value = u32::from_le_bytes(result[word * 4..word * 4 + 4].try_into().unwrap());
        checksum ^= value;
        result[layout.parity_offset + word / 8] |= (value.count_ones() as u8 & 1) << (word % 8);
    }
    result[layout.checksum_offset..].copy_from_slice(&checksum.to_le_bytes());
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_catalog_decodes_and_erased_inputs_need_no_assets() {
        let mut packet = vec![0; 16];
        packet[14] = 0xaa;
        packet[15] = 0x56;
        let catalog: Catalog = serde_json::from_value(serde_json::json!({
            "geometry":{"bytes":16384,"page_bytes":512,"pages_per_block":32,"spare_bytes":16},
            "assets":{},
            "sets":{"erased":{"placements":[],"append":null,
                "mcu_packet":{"kind":"bytes","bytes":packet}}}
        }))
        .unwrap();
        let nand = catalog
            .prepare("erased", &BTreeMap::new(), Arc::from([]))
            .unwrap();
        let mut bytes = [0; 16];
        nand.read(0, &mut bytes).unwrap();
        assert_eq!(bytes, [255; 16]);
        let packet = catalog
            .mcu_packet("erased", &BTreeMap::new())
            .unwrap()
            .unwrap();
        assert_eq!(packet[14], 0xaa);
        assert!(
            catalog
                .prepare("absent", &BTreeMap::new(), Arc::from([]))
                .is_err()
        );
    }
    #[test]
    fn patches_are_identity_checked_and_journal_parity_is_recomputed() {
        let source = [1u8, 2, 3];
        let normalized = [1u8, 5, 3];
        let asset = Asset {
            bytes: 3,
            sha256: format!("{:x}", Sha256::digest(source)),
            normalized_sha256: format!("{:x}", Sha256::digest(normalized)),
            patches: vec![Patch {
                offset: 1,
                bytes: vec![5],
            }],
        };
        assert_eq!(asset.prepare(&source).unwrap(), normalized);
        assert_eq!(asset.prepare(&normalized).unwrap(), normalized);
        assert!(asset.prepare(&[1, 2, 4]).is_err());
        let layout = JournalAppend {
            record_asset: "r".into(),
            payload_asset: "p".into(),
            payload_offset: 0,
            payload_bytes: 4,
            record_payload_offset: 4,
            parity_offset: 16,
            checksum_offset: 20,
            physical_offset: 0,
        };
        let mut record = [0; 24];
        record[..2].copy_from_slice(&65535u16.to_le_bytes());
        let updated = append_record(&record, &[1, 0, 0, 0], &layout).unwrap();
        assert_eq!(&updated[..2], &[0, 0]);
        assert_eq!(updated[16], 2);
        assert_eq!(&updated[20..], &1u32.to_le_bytes());
    }
}
