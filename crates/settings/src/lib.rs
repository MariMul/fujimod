//! Camera settings banks built from a firmware's catalog layout and its update file.
//!
//! A bank starts as zero bytes, takes the stock defaults table read from the
//! update, then the layout's fields and the requested named settings. Stock
//! firmware reads the result from flash as it would a camera's own settings.
#![forbid(unsafe_code)]
use fujimod_catalog::settings::{FirmwareSettings, UpdateRange};
use std::collections::BTreeMap;

/// A settings bank ready to place in flash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    pub bytes: Vec<u8>,
    pub first_block: u32,
    pub blocks: u32,
    pub logical_first: u16,
}

/// The installed flash image of an update file: its bytes after the flash
/// offset, inverted.
pub fn flash_image(update: &[u8], flash_offset: u64) -> Result<Vec<u8>, String> {
    let offset = usize::try_from(flash_offset).map_err(|_| "flash offset overflow")?;
    let installed = update.get(offset..).ok_or("update is shorter than its flash offset")?;
    Ok(fujimod_firmware::invert(installed))
}

/// The layout's source section, decoded from the installed flash image and checked.
fn source(layout: &FirmwareSettings, flash: &[u8]) -> Result<Vec<u8>, String> {
    let section = &layout.source;
    let stream = usize::try_from(section.flash_offset)
        .ok()
        .and_then(|offset| flash.get(offset..))
        .ok_or("settings source section is outside the flash image")?;
    let (mut bytes, _) = fujimod_firmware::decompress(stream).map_err(|error| error.to_string())?;
    bytes.truncate(section.size as usize);
    if fujimod_firmware::sha256(&bytes) != section.sha256 {
        return Err("settings source section differs from the layout".into());
    }
    Ok(bytes)
}

fn read(section: &[u8], layout: &FirmwareSettings, range: &UpdateRange) -> Result<Vec<u8>, String> {
    let start = (range.address - layout.source.address) as usize;
    let bytes = section[start..start + range.size as usize].to_vec();
    if fujimod_firmware::sha256(&bytes) != range.sha256 {
        return Err(format!("stock bytes at {:#x} differ from the layout", range.address));
    }
    Ok(bytes)
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

/// Build the bank for `requested` settings (`name -> value name`) from the
/// installed flash image.
pub fn build(
    layout: &FirmwareSettings,
    flash: &[u8],
    requested: &BTreeMap<String, String>,
) -> Result<Bank, String> {
    layout.validate()?;
    let section = source(layout, flash)?;
    let bank = &layout.bank;
    let descriptor = read(&section, layout, &bank.descriptor)?;
    if [word(&descriptor, 0), word(&descriptor, 4), word(&descriptor, 8)]
        != [bank.kind, bank.blocks, bank.first_block]
    {
        return Err("stock bank descriptor differs from the layout".into());
    }
    let size = usize::try_from(bank.bytes).map_err(|_| "bank too large")?;
    let mut bytes = vec![0; size];
    if let Some(table) = &layout.defaults {
        let table = read(&section, layout, table)?;
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
pub fn value(layout: &FirmwareSettings, bank: &[u8], name: &str) -> Option<String> {
    let setting = layout.settings.get(name)?;
    setting.values.iter().find_map(|(value, encoded)| {
        let offset = setting.offset as usize;
        (bank.get(offset..offset + encoded.len()) == Some(encoded.as_slice()))
            .then(|| value.clone())
    })
}

#[cfg(test)]
mod tests;
