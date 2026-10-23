//! Body startup combinations and their documented purposes, independent of execution support.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const BODY_BOOT_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/body-boot.generated.schema.json";

/// Hand-maintained startup options for a body, including model-specific variants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BodyBootOptions {
    /// Stable schema identity.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Document format version.
    pub schema: u32,
    /// Body identity; a hardware definition is not required.
    pub body_id: String,
    /// Camera model names reviewed in this document.
    pub models: Vec<String>,
    /// Startup options keyed by identity, grouped for callers.
    pub boot_options: BTreeMap<String, BootOption>,
    /// Body-specific coverage or interpretation notes.
    pub notes: Vec<String>,
}

/// One startup option; describing it does not enable a camera operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootOption {
    /// Human-readable name.
    pub label: String,
    /// Related startup options shown together by a caller.
    pub group: BootGroup,
    /// Program or session selected by this procedure.
    pub purpose: BootPurpose,
    /// Exact model names covered; shared bodies do not imply sibling support.
    pub models: Vec<String>,
    /// Whether the instruction is documented, implemented, provisional, or superseded.
    pub status: BootStatus,
    /// Controls and connection/card conditions at power-on.
    pub entry: BootEntry,
    /// Actions the selected program can perform, not consequences of every key press.
    pub effects: Vec<BootEffect>,
    /// Details relevant to this option.
    pub notes: Vec<String>,
}

/// Top-level startup groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootGroup {
    Service,
    FirmwareUpdate,
    SdBoot,
}

/// Distinct purposes which may share the same startup controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootPurpose {
    ServiceSession,
    FirmwareUpdate,
    FirmwareDownload,
    CardProgram,
    EntryProbe,
}

/// Instruction status; candidates and superseded instructions are not active procedures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootStatus {
    Documented,
    Implemented,
    Candidate,
    Superseded,
}

/// Program capabilities or known lifecycle effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootEffect {
    FirmwareWrite,
    ServiceCommands,
    CardProgramDefined,
    SettingsResetOnExit,
}

/// Physical entry conditions; no host command sequence is encoded here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootEntry {
    /// USB condition at entry.
    pub usb: BootUsb,
    /// Card contents required by the procedure.
    pub card: BootCard,
    /// Card slot when specified, numbered from one.
    pub slot: Option<u8>,
    /// Simultaneously held controls when powering on.
    pub hold: Vec<BootKey>,
    /// Observable condition for releasing the held controls, when specified.
    pub release_when: Option<String>,
    /// Existing ordered guide text, when the published guide has additional steps.
    pub instructions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootUsb {
    Connected,
    NotUsed,
    Unspecified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootCard {
    ServiceBoot,
    FirmwareUpdate,
    NotRequired,
    Unspecified,
}

/// One held control with an explicit physical action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootKey {
    /// Human-readable control name.
    pub label: String,
    /// Canonical physical-control identity, when resolved unambiguously.
    pub control_id: Option<String>,
    /// Press or directional action; a label is never parsed to infer this.
    pub action: BootKeyAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BootKeyAction {
    Press,
    FullPress,
    Up,
}

impl BodyBootOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != BODY_BOOT_SCHEMA_URI {
            return Err("body boot schema differs".into());
        }
        crate::reference::validate_id(&self.body_id)?;
        if self.models.is_empty() || self.models.iter().any(|model| model.trim().is_empty()) {
            return Err("body boot document requires model names".into());
        }
        for (id, option) in &self.boot_options {
            crate::reference::validate_id(id)?;
            let models: BTreeSet<_> = option.models.iter().collect();
            if option
                .models
                .iter()
                .any(|model| !self.models.contains(model))
            {
                return Err(format!("boot option {id} refers to an unreviewed model"));
            }
            if option.label.trim().is_empty()
                || models.is_empty()
                || models.len() != option.models.len()
                || models.iter().any(|m| m.trim().is_empty())
                || option.entry.hold.is_empty()
                || option.entry.slot == Some(0)
            {
                return Err(format!("boot option {id} has incomplete entry"));
            }
            let expected_group = match option.purpose {
                BootPurpose::ServiceSession => BootGroup::Service,
                BootPurpose::FirmwareUpdate => BootGroup::FirmwareUpdate,
                _ => BootGroup::SdBoot,
            };
            if option.group != expected_group {
                return Err(format!("boot option {id} has the wrong purpose group"));
            }
            let mut held = BTreeSet::new();
            for key in &option.entry.hold {
                if key.label.trim().is_empty() || !held.insert((&key.control_id, &key.label)) {
                    return Err(format!(
                        "boot option {id} has an empty or duplicate held control"
                    ));
                }
            }
        }
        Ok(())
    }
}
