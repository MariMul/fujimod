//! Camera settings banks built from a firmware's catalog layout and its update file.
//!
//! A bank starts as zero bytes, takes the stock defaults table read from the
//! update, then the layout's fields and the requested named settings. Stock
//! firmware reads the result from flash as it would a camera's own settings.
#![forbid(unsafe_code)]
use fujimod_catalog::{
    reference::FirmwareInstallation,
    settings::{FirmwareSettings, UpdateRange},
};
use std::collections::{BTreeMap, HashMap};

/// A settings bank ready to place in flash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    pub bytes: Vec<u8>,
    pub first_block: u32,
    pub blocks: u32,
    pub logical_first: u16,
}

/// Reads firmware addresses from an update by decoding the mapped section that
/// holds them, once per section.
pub struct Update<'a> {
    source: &'a [u8],
    installation: &'a FirmwareInstallation,
    inverted: Option<Vec<u8>>,
    sections: HashMap<String, Vec<u8>>,
}
impl<'a> Update<'a> {
    pub fn new(source: &'a [u8], installation: &'a FirmwareInstallation) -> Self {
        Self {
            source,
            installation,
            inverted: None,
            sections: HashMap::new(),
        }
    }
    /// The stock bytes of `range`, checked against its SHA-256.
    pub fn read(&mut self, range: &UpdateRange) -> Result<Vec<u8>, String> {
        let end = range.address + range.size;
        let section = self
            .installation
            .image_sections
            .iter()
            .find(|s| s.address <= range.address && end <= s.address + s.size)
            .ok_or_else(|| format!("{:#x} is in no mapped update section", range.address))?;
        if !self.sections.contains_key(&section.name) {
            let native = section
                .native_flash_offset
                .ok_or_else(|| format!("section {} has no flash offset", section.name))?;
            let offset = usize::try_from(self.installation.flash_offset + native)
                .map_err(|_| "update offset overflow")?;
            let inverted = self
                .inverted
                .get_or_insert_with(|| fujimod_firmware::invert(self.source));
            let stream = inverted
                .get(offset..)
                .ok_or_else(|| format!("section {} is outside the update", section.name))?;
            let (mut bytes, _) =
                fujimod_firmware::decompress(stream).map_err(|error| error.to_string())?;
            bytes.truncate(section.size as usize);
            if fujimod_firmware::sha256(&bytes) != section.sha256 {
                return Err(format!("update section {} differs from the catalog", section.name));
            }
            self.sections.insert(section.name.clone(), bytes);
        }
        let start = (range.address - section.address) as usize;
        let bytes = self.sections[&section.name][start..start + range.size as usize].to_vec();
        if fujimod_firmware::sha256(&bytes) != range.sha256 {
            return Err(format!("stock bytes at {:#x} differ from the layout", range.address));
        }
        Ok(bytes)
    }
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

/// Build the bank for `requested` settings (`name -> value name`).
pub fn build(
    layout: &FirmwareSettings,
    update: &mut Update<'_>,
    requested: &BTreeMap<String, String>,
) -> Result<Bank, String> {
    layout.validate()?;
    let bank = &layout.bank;
    let descriptor = update.read(&bank.descriptor)?;
    if [word(&descriptor, 0), word(&descriptor, 4), word(&descriptor, 8)]
        != [bank.kind, bank.blocks, bank.first_block]
    {
        return Err("stock bank descriptor differs from the layout".into());
    }
    let size = usize::try_from(bank.bytes).map_err(|_| "bank too large")?;
    let mut bytes = vec![0; size];
    if let Some(table) = &layout.defaults {
        let table = update.read(table)?;
        if table[table.len() - 4..] != [0xff, 0xff, 0, 0] {
            return Err("defaults table has no terminator".into());
        }
        for entry in table[..table.len() - 4].chunks_exact(4) {
            let offset = usize::from(u16::from_le_bytes([entry[0], entry[1]]));
            if offset >= size || entry[3] != 0 {
                return Err("defaults entry outside the bank".into());
            }
            bytes[offset] = entry[2];
        }
    }
    let mut put = |offset: u64, value: &[u8]| {
        let offset = offset as usize;
        bytes[offset..offset + value.len()].copy_from_slice(value);
    };
    for field in &layout.fields {
        put(field.offset, &field.bytes);
    }
    for (name, value) in requested {
        let setting = layout
            .settings
            .get(name)
            .ok_or_else(|| format!("{} has no located setting {name}", layout.firmware_id))?;
        let encoded = setting
            .values
            .get(value)
            .ok_or_else(|| format!("setting {name} has no value {value}"))?;
        put(setting.offset, encoded);
    }
    Ok(Bank {
        bytes,
        first_block: bank.first_block,
        blocks: bank.blocks,
        logical_first: bank.logical_first,
    })
}

/// The value name a bank holds for `name`, when it matches one.
pub fn read(layout: &FirmwareSettings, bank: &[u8], name: &str) -> Option<String> {
    let setting = layout.settings.get(name)?;
    setting.values.iter().find_map(|(value, encoded)| {
        let offset = setting.offset as usize;
        (bank.get(offset..offset + encoded.len()) == Some(encoded.as_slice()))
            .then(|| value.clone())
    })
}

#[cfg(test)]
mod tests;
