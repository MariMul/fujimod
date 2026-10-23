//! Generated settings layout for one firmware.
//!
//! Stock firmware loads its settings from a bank of flash blocks at startup,
//! and its user profile from a header the power controller keeps and from
//! journal records in flash. The document locates the bank, the stock defaults
//! tables in the update, the fields every valid bank and record carries, and
//! the named settings a run may request. It holds addresses, sizes and hashes;
//! bank and record bytes are built from the update.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SETTINGS_SCHEMA_URI: &str = "https://fujimod.com/schemas/settings.generated.schema.json";

/// Located settings bank and named settings for one firmware.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// The stock user profile, when located.
    pub profile: Option<SettingsProfile>,
    /// Settings a run may request by name, each with its named values.
    pub settings: BTreeMap<String, NamedSetting>,
}

/// The stock user profile: a sixteen-byte header kept by the power controller
/// and journal records in managed flash blocks. Firmware that finds no valid
/// header and records resets the profile to its defaults at startup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct SettingsProfile {
    /// Stock header defaults. Byte 14 is the marker `0xaa`; byte 15 holds the
    /// checksum that makes the sixteen bytes sum to zero.
    pub header: UpdateRange,
    /// Journal records the stock loader requires, by name.
    pub records: BTreeMap<String, ProfileRecord>,
}

/// One journal record: stock defaults and fixed bytes, sealed with one parity
/// bit per data word and an XOR checksum of the data words in its last four bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct ProfileRecord {
    /// Physical flash block holding the record at its start.
    pub block: u32,
    /// Managed logical index of that block.
    pub managed_index: u16,
    /// Record length in bytes.
    pub bytes: u64,
    /// Stock defaults copied into the record.
    pub defaults: UpdateRange,
    /// Record offset of the defaults.
    pub defaults_offset: u64,
    /// Record offset of the parity bitmap; the data words precede it.
    pub parity_offset: u64,
    /// Bytes every valid record carries outside the defaults.
    pub fields: Vec<SettingsField>,
}

/// A method-4 encoded section of the installed flash image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// Bytes written at a bank or record offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(tsify::Tsify))]
pub struct SettingsField {
    /// Byte offset in the bank or record.
    pub offset: u64,
    /// Bytes written there; at most 128.
    pub bytes: Vec<u8>,
    /// What the bytes are and where they come from.
    pub basis: String,
}

/// A setting a run requests by name, such as `debug` or `language`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct NamedSetting {
    /// Where the setting is stored: `bank`, `header`, or a profile record name.
    pub store: String,
    /// Byte offset in that store.
    pub offset: u64,
    /// Encoded bytes for each value name; all the same length.
    pub values: BTreeMap<String, Vec<u8>>,
}

impl FirmwareSettings {
    /// Validate identity, store extents and field bounds.
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
        let profile = self.profile.as_ref();
        let ranges = std::iter::once(&bank.descriptor)
            .chain(&self.defaults)
            .chain(profile.map(|profile| &profile.header))
            .chain(
                profile
                    .into_iter()
                    .flat_map(|p| p.records.values().map(|r| &r.defaults)),
            );
        for range in ranges {
            if range.address < self.source.address
                || range
                    .address
                    .checked_add(range.size)
                    .zip(source_end)
                    .is_none_or(|(end, limit)| end > limit)
            {
                return fail("stock range outside the source section");
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
        if self
            .defaults
            .as_ref()
            .is_some_and(|table| table.size % 4 != 0)
        {
            return fail("defaults table is not whole entries");
        }
        // Store extents, each with the offsets its fields and settings occupy.
        let mut stores: BTreeMap<&str, (u64, BTreeSet<u64>)> = BTreeMap::new();
        stores.insert("bank", (bank.bytes, BTreeSet::new()));
        if let Some(profile) = profile {
            if profile.header.size != 16 {
                return fail("profile header is not sixteen bytes");
            }
            stores.insert("header", (14, BTreeSet::new()));
            for (name, record) in &profile.records {
                let parity_bytes = (record.parity_offset / 4).div_ceil(8);
                if name.is_empty()
                    || stores.contains_key(name.as_str())
                    || record.parity_offset % 4 != 0
                    || record.defaults_offset + record.defaults.size > record.parity_offset
                    || record.parity_offset + parity_bytes + 4 > record.bytes
                    || record.bytes > 0x10_0000
                {
                    return fail("profile record name, defaults or parity outside the record");
                }
                stores.insert(name, (record.parity_offset, BTreeSet::new()));
            }
        }
        let place = |stores: &mut BTreeMap<&str, (u64, BTreeSet<u64>)>,
                     store: &str,
                     offset: u64,
                     size: usize| {
            let Some((extent, offsets)) = stores.get_mut(store) else {
                return false;
            };
            size > 0
                && size <= 128
                && offset
                    .checked_add(size as u64)
                    .is_some_and(|end| end <= *extent)
                && offsets.insert(offset)
        };
        for field in &self.fields {
            if !place(&mut stores, "bank", field.offset, field.bytes.len()) {
                return fail("field outside the bank or repeated");
            }
        }
        for (name, record) in profile.into_iter().flat_map(|p| &p.records) {
            for field in &record.fields {
                if !place(&mut stores, name, field.offset, field.bytes.len()) {
                    return fail("record field outside its record or repeated");
                }
            }
        }
        for (name, setting) in &self.settings {
            let mut sizes = setting.values.values().map(Vec::len);
            let size = sizes.next().unwrap_or(0);
            if name.is_empty()
                || !sizes.all(|other| other == size)
                || !place(&mut stores, &setting.store, setting.offset, size)
            {
                return fail("named setting is empty, uneven, unstored or overlaps a field");
            }
        }
        Ok(())
    }
}
