//! Generated settings-bank layout for one firmware.
//!
//! Stock firmware loads its settings from a bank of flash blocks at startup.
//! The document locates that bank, the stock defaults table in the update, the
//! fields every valid bank carries, and the named settings a run may request.
//! It holds addresses, sizes and hashes; bank bytes are built from the update.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SETTINGS_SCHEMA_URI: &str = "https://fujimod.com/schemas/settings.generated.schema.json";

/// Located settings bank and named settings for one firmware.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct FirmwareSettings {
    /// Schema identifying the generated document.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Document version.
    pub schema: u32,
    /// Stable document identity.
    pub id: String,
    /// Firmware whose stock loader reads this bank.
    pub firmware_id: String,
    /// The decoded update section holding the bank descriptor and defaults table.
    pub source: SettingsSource,
    /// Where the bank lives in flash.
    pub bank: SettingsBank,
    /// Stock default values applied to an otherwise zero bank, when the
    /// firmware has a defaults table.
    pub defaults: Option<UpdateRange>,
    /// Bytes every valid bank carries: signature, identity, body hardware values.
    pub fields: Vec<SettingsField>,
    /// Settings a run may request by name, each with its named values.
    pub settings: BTreeMap<String, NamedSetting>,
}

/// A method-4 encoded section of the installed flash image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct SettingsSource {
    /// Offset of the encoded stream in the installed flash image.
    pub flash_offset: u64,
    /// Firmware address of the first decoded byte.
    pub address: u64,
    /// Decoded length in bytes.
    pub size: u64,
    /// Lowercase hexadecimal SHA-256 of the decoded section.
    pub sha256: String,
}

/// The flash blocks holding the settings bank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct SettingsBank {
    /// The stock bank descriptor `(kind, blocks, first block)` in the update.
    pub descriptor: UpdateRange,
    /// Descriptor kind word.
    pub kind: u32,
    /// Number of flash blocks in the bank.
    pub blocks: u32,
    /// First physical flash block.
    pub first_block: u32,
    /// Managed logical index of the first block.
    pub logical_first: u16,
    /// Bank content length in bytes.
    pub bytes: u64,
}

/// Bytes at a firmware address in the decoded update, identified by SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct UpdateRange {
    /// Firmware address.
    pub address: u64,
    /// Length in bytes.
    pub size: u64,
    /// Lowercase hexadecimal SHA-256 of the stock bytes.
    pub sha256: String,
}

/// Bytes written at a bank offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct SettingsField {
    /// Byte offset in the bank.
    pub offset: u64,
    /// Bytes written there; at most 128.
    pub bytes: Vec<u8>,
    /// What the bytes are and where they come from.
    pub basis: String,
}

/// A setting a run requests by name, such as `debug` or `language`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct NamedSetting {
    /// Byte offset in the bank.
    pub offset: u64,
    /// Encoded bytes for each value name; all the same length.
    pub values: BTreeMap<String, Vec<u8>>,
}

impl FirmwareSettings {
    /// Validate identity, bank extent and field bounds.
    pub fn validate(&self) -> Result<(), String> {
        crate::reference::validate_id(&self.id)?;
        crate::reference::validate_id(&self.firmware_id)?;
        let fail = |message: &str| Err(format!("settings {}: {message}", self.id));
        if self.schema_uri != SETTINGS_SCHEMA_URI || self.schema != 1 {
            return fail("unsupported schema");
        }
        let bank = &self.bank;
        if bank.blocks == 0 || bank.bytes == 0 || bank.descriptor.size != 12 {
            return fail("empty bank or descriptor");
        }
        let source_end = self.source.address.checked_add(self.source.size);
        for range in std::iter::once(&bank.descriptor).chain(&self.defaults) {
            if range.address < self.source.address
                || range
                    .address
                    .checked_add(range.size)
                    .zip(source_end)
                    .is_none_or(|(end, limit)| end > limit)
            {
                return fail("descriptor or defaults outside the source section");
            }
            if range.size == 0
                || range.address.checked_add(range.size).is_none()
                || range.sha256.len() != 64
                || !range
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return fail("invalid update range");
            }
        }
        if self.defaults.as_ref().is_some_and(|table| table.size % 4 != 0) {
            return fail("defaults table is not whole entries");
        }
        let inside = |offset: u64, size: usize| {
            size > 0
                && size <= 128
                && offset
                    .checked_add(size as u64)
                    .is_some_and(|end| end <= bank.bytes)
        };
        let mut offsets = BTreeSet::new();
        for field in &self.fields {
            if !inside(field.offset, field.bytes.len()) || !offsets.insert(field.offset) {
                return fail("field outside the bank or repeated");
            }
        }
        for (name, setting) in &self.settings {
            let mut sizes = setting.values.values().map(Vec::len);
            let size = sizes.next().unwrap_or(0);
            if name.is_empty()
                || !sizes.all(|other| other == size)
                || !inside(setting.offset, size)
                || !offsets.insert(setting.offset)
            {
                return fail("named setting is empty, uneven or overlaps a field");
            }
        }
        Ok(())
    }
}
