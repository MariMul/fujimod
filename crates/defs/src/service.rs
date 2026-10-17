//! App80 service definitions loaded from caller-selected JSON data.
use serde::Deserialize;
use std::num::NonZeroU32;

#[derive(Debug, thiserror::Error)]
pub enum DefinitionError {
    #[error("invalid service definition JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported service definition schema {0}")]
    Schema(u32),
    #[error("firmware identity fields must be nonempty")]
    EmptyIdentity,
    #[error("firmware digest must contain exactly 64 hexadecimal characters")]
    InvalidDigest,
    #[error("definition does not match the selected firmware id and digest")]
    FirmwareMismatch,
    #[error("service mode mask must be nonzero")]
    EmptyModes,
    #[error("App80 mailbox needs 33 bytes within its configuration limit and address space")]
    InvalidMailbox,
    #[error("command {code:#x} has an invalid {codec:?} argument layout")]
    InvalidLayout { code: u8, codec: Codec },
    #[error("source record count differs from catalogue entries")]
    SourceCount,
    #[error("command labels must be nonempty")]
    EmptyLabel,
}

/// SHA-256 identity, not an address or command selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Digest([u8; 32]);
impl Digest {
    pub fn parse(hex: &str) -> Result<Self, DefinitionError> {
        if hex.len() != 64 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(DefinitionError::InvalidDigest);
        }
        let mut bytes = [0; 32];
        for (index, value) in bytes.iter_mut().enumerate() {
            *value =
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("validated hex pair");
        }
        Ok(Self(bytes))
    }
    pub fn hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareIdentity {
    id: String,
    digest: Digest,
}
impl FirmwareIdentity {
    pub fn new(id: impl Into<String>, sha256: &str) -> Result<Self, DefinitionError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(DefinitionError::EmptyIdentity);
        }
        Ok(Self {
            id,
            digest: Digest::parse(sha256)?,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
}
/// Internal service-dispatch flags, distinct from camera UI and shooting modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceModes(NonZeroU32);
impl ServiceModes {
    pub fn new(mask: u32) -> Result<Self, DefinitionError> {
        NonZeroU32::new(mask)
            .map(Self)
            .ok_or(DefinitionError::EmptyModes)
    }
    pub fn mask(self) -> u32 {
        self.0.get()
    }
    pub fn intersects(self, other: Self) -> bool {
        self.mask() & other.mask() != 0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModeTransition {
    Keep,
    Set(ServiceModes),
}
/// Whether a row's service-dispatch eligibility has been established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModeAvailability {
    Modes(ServiceModes),
    Disabled,
    Unreviewed,
}
impl ModeAvailability {
    fn accepts(self, modes: ServiceModes) -> bool {
        matches!(self,Self::Modes(allowed) if allowed.intersects(modes))
    }
}
/// A reviewed codec implementation. `Research` makes no argument semantics claim.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Research,
    WritePins,
    ReadPins,
    ReadChannel,
    WriteChannel,
    ProfileByte,
    DownloadStatus,
    CopyConfiguration,
    FlushConfiguration,
    MemoryWord,
    RegisterControl,
    I2c,
    Accumulator,
    PeripheralReport,
}
impl Codec {
    pub fn argument_slots(self) -> usize {
        match self {
            Self::Research => 0,
            Self::WritePins => 5,
            Self::ReadPins | Self::ReadChannel | Self::WriteChannel | Self::ProfileByte => 3,
            Self::DownloadStatus | Self::CopyConfiguration | Self::FlushConfiguration => 1,
            Self::MemoryWord => 10,
            Self::RegisterControl | Self::Accumulator => 7,
            Self::I2c => 14,
            Self::PeripheralReport => 8,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Mailbox {
    page: u32,
    offset: u32,
    limit: NonZeroU32,
}
impl Mailbox {
    pub fn page(self) -> u32 {
        self.page
    }
    pub fn offset(self) -> u32 {
        self.offset
    }
    pub fn configuration_limit(self) -> NonZeroU32 {
        self.limit
    }
}
#[derive(Clone, Debug)]
pub struct Entry {
    code: u8,
    label: String,
    codec: Codec,
    slots: Vec<u8>,
    enabled_modes: ModeAvailability,
    transition: ModeTransition,
    firmware_kind: u32,
    handler: u64,
    parameter_table: u64,
}
impl Entry {
    pub fn code(&self) -> u8 {
        self.code
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn codec(&self) -> Codec {
        self.codec
    }
    /// Zero-based positions in App80's 32 argument/result bytes.
    pub fn slots(&self) -> &[u8] {
        &self.slots
    }
    pub fn enabled_modes(&self) -> ModeAvailability {
        self.enabled_modes
    }
    pub fn transition(&self) -> ModeTransition {
        self.transition
    }
    /// Exact firmware table data. Never a host pending/completed disposition.
    pub fn firmware_kind(&self) -> u32 {
        self.firmware_kind
    }
    pub fn handler(&self) -> u64 {
        self.handler
    }
    pub fn parameter_table(&self) -> u64 {
        self.parameter_table
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Dispatch24,
    Declarations12,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub format: SourceFormat,
    pub records: u32,
    pub table: u64,
    pub table_sha256: String,
    pub dispatcher: u64,
}
#[derive(Clone, Debug)]
pub struct Definition {
    identity: FirmwareIdentity,
    model: String,
    version: String,
    mailbox: Mailbox,
    entries: Vec<Entry>,
    source: Option<Source>,
}
impl Definition {
    /// Loading requires the selected registry identity; this does not identify
    /// a connected camera or grant authorization to execute its commands.
    pub fn from_json(json: &str, expected: &FirmwareIdentity) -> Result<Self, DefinitionError> {
        let raw: RawDefinition = serde_json::from_str(json)?;
        if raw.schema != 1 {
            return Err(DefinitionError::Schema(raw.schema));
        }
        let identity = FirmwareIdentity::new(raw.firmware.id, &raw.firmware.sha256)?;
        if &identity != expected {
            return Err(DefinitionError::FirmwareMismatch);
        }
        if raw.firmware.model.trim().is_empty() || raw.firmware.version.trim().is_empty() {
            return Err(DefinitionError::EmptyIdentity);
        }
        let limit = NonZeroU32::new(raw.mailbox.configuration_limit)
            .filter(|limit| limit.get() >= 33)
            .ok_or(DefinitionError::InvalidMailbox)?;
        raw.mailbox
            .offset
            .checked_add(33)
            .ok_or(DefinitionError::InvalidMailbox)?;
        if let Some(source) = &raw.source {
            Digest::parse(&source.table_sha256)?;
        }
        let mut entries = Vec::new();
        for e in raw.entries {
            if e.label.trim().is_empty() {
                return Err(DefinitionError::EmptyLabel);
            }
            let valid = e.slots.len() == e.codec.argument_slots()
                && e.slots.iter().all(|slot| *slot < 32)
                && e.slots
                    .iter()
                    .enumerate()
                    .all(|(index, slot)| !e.slots[..index].contains(slot));
            if !valid {
                return Err(DefinitionError::InvalidLayout {
                    code: e.code,
                    codec: e.codec,
                });
            }
            entries.push(Entry {
                code: e.code,
                label: e.label,
                codec: e.codec,
                slots: e.slots,
                enabled_modes: match e.enabled_modes {
                    None => ModeAvailability::Unreviewed,
                    Some(0) => ModeAvailability::Disabled,
                    Some(mask) => ModeAvailability::Modes(ServiceModes::new(mask)?),
                },
                transition: match NonZeroU32::new(e.next_mode) {
                    None => ModeTransition::Keep,
                    Some(mask) => ModeTransition::Set(ServiceModes(mask)),
                },
                firmware_kind: e.firmware_kind,
                handler: e.handler,
                parameter_table: e.parameter_table,
            });
        }
        if raw
            .source
            .as_ref()
            .is_some_and(|s| s.records as usize != entries.len())
        {
            return Err(DefinitionError::SourceCount);
        }
        Ok(Self {
            identity,
            model: raw.firmware.model,
            version: raw.firmware.version,
            mailbox: Mailbox {
                page: raw.mailbox.page,
                offset: raw.mailbox.offset,
                limit,
            },
            entries,
            source: raw.source,
        })
    }
    pub fn identity(&self) -> &FirmwareIdentity {
        &self.identity
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn mailbox(&self) -> Mailbox {
        self.mailbox
    }
    /// Firmware order is meaningful: duplicate command bytes are retained.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn source(&self) -> Option<&Source> {
        self.source.as_ref()
    }
    pub fn select(&self, code: u8, modes: ServiceModes) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.code == code && entry.enabled_modes.accepts(modes))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefinition {
    schema: u32,
    firmware: RawFirmware,
    mailbox: RawMailbox,
    entries: Vec<RawEntry>,
    source: Option<Source>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFirmware {
    id: String,
    model: String,
    version: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMailbox {
    page: u32,
    offset: u32,
    configuration_limit: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    code: u8,
    label: String,
    codec: Codec,
    slots: Vec<u8>,
    enabled_modes: Option<u32>,
    next_mode: u32,
    firmware_kind: u32,
    handler: u64,
    parameter_table: u64,
}

#[cfg(test)]
mod tests;
