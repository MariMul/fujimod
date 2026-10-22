//! Identity checks and declared nonvolatile input construction for a run.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const RUN_FIRMWARE_INPUTS_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/emulator-firmware-inputs.generated.schema.json";

/// A caller-supplied recipe with no embedded firmware contents or input paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunFirmwareInputs {
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    pub schema: u32,
    pub id: String,
    pub firmware_id: String,
    pub source_sha256: String,
    pub firmware: UsbFirmwareChecks,
    pub nand_inputs: Vec<NandByteInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_input: Option<UsbConfigurationInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<UsbManifestSelection>,
    pub limits: Vec<String>,
}

/// The USB firmware entry points whose immutable bytes qualify the input recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsbFirmwareChecks {
    pub usb_irq_wiring: UsbIrqChecks,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsbIrqChecks {
    pub views: Vec<RunSourceCheck>,
}

/// A named and hashed immutable source range; contains no source bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunSourceCheck {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub sha256: String,
    pub sampling: RunSourceSampling,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunSourceSampling {
    ImmutableSource,
}

/// A checked synthetic NAND byte input, never a RAM patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NandByteInput {
    pub offset: u64,
    pub expected: u8,
    pub value: u8,
    pub basis: String,
}

/// A firmware-qualified configuration-bank construction recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsbConfigurationInput {
    pub nand: String,
    pub bank_descriptor: RunSourceCheck,
    pub bank: [u32; 3],
    pub logical_first: u32,
    pub configuration_bytes: u64,
    pub fields: Vec<UsbConfigurationField>,
    pub limits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults_table: Option<RunSourceCheck>,
    pub code_checks: Vec<RunSourceCheck>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsbConfigurationField {
    pub offset: u64,
    /// Explicit small external input, if supplied instead of stock defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    pub basis: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UsbManifestSelection {
    Exr2 { node: String },
}

impl RunFirmwareInputs {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != RUN_FIRMWARE_INPUTS_SCHEMA_URI {
            return Err("run firmware inputs schema differs".into());
        }
        crate::emulator::validate_id(&self.id)?;
        crate::emulator::validate_id(&self.firmware_id)?;
        let digest =
            |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !digest(&self.source_sha256) {
            return Err("run firmware source identity is invalid".into());
        }
        let mut checks: Vec<_> = self.firmware.usb_irq_wiring.views.iter().collect();
        if let Some(input) = &self.configuration_input {
            crate::emulator::validate_id(&input.nand)?;
            if input.configuration_bytes == 0 || input.bank[0] == 0 || input.bank[2] == 0 {
                return Err("run configuration bank has zero extent".into());
            }
            let mut offsets = std::collections::BTreeSet::new();
            for field in &input.fields {
                let size = field.bytes.as_ref().map_or(1, Vec::len);
                if size == 0
                    || size > 128
                    || field
                        .offset
                        .checked_add(size as u64)
                        .is_none_or(|end| end > input.configuration_bytes)
                    || !offsets.insert(field.offset)
                {
                    return Err(
                        "run configuration input is repeated, too large or outside its bank".into(),
                    );
                }
            }
            checks.push(&input.bank_descriptor);
            checks.extend(input.defaults_table.iter());
            checks.extend(&input.code_checks);
        }
        for check in checks {
            if check.size == 0
                || !digest(&check.sha256)
                || check.address.checked_add(check.size).is_none()
            {
                return Err("run source check is empty or has an invalid identity".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_recipes_refuse_unbounded_configuration_bytes_and_bad_source_identities() {
        let value = serde_json::json!({
            "$schema": RUN_FIRMWARE_INPUTS_SCHEMA_URI, "schema": 1, "id": "test-usb-inputs",
            "firmware_id": "test-100", "source_sha256": "0".repeat(64),
            "firmware": {"usb_irq_wiring": {"views": []}}, "nand_inputs": [], "limits": [],
            "configuration_input": {
                "nand": "nand", "bank_descriptor": {"name": "bank", "address": 4, "size": 12,
                    "sha256": "1".repeat(64), "sampling": "immutable-source"},
                "bank": [2, 40, 937], "logical_first": 41, "configuration_bytes": 4096,
                "fields": [{"offset": 247, "bytes": [1], "basis": "Synthetic input"}],
                "limits": [], "code_checks": []
            }
        });
        let mut inputs: RunFirmwareInputs = serde_json::from_value(value).unwrap();
        assert!(inputs.validate().is_ok());
        inputs.configuration_input.as_mut().unwrap().fields[0].bytes = Some(vec![0; 129]);
        assert!(inputs.validate().is_err());
        inputs.configuration_input.as_mut().unwrap().fields[0].bytes = Some(vec![1]);
        inputs.source_sha256 = "invalid".into();
        assert!(inputs.validate().is_err());
    }
}

/// Caller-owned locations for optional factory inputs, kept outside run policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunFactoryInputs {
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    pub schema: u32,
    pub id: String,
    pub firmware_id: String,
    pub source_sha256: String,
    pub producer_data_fixture: String,
}

pub const RUN_FACTORY_INPUTS_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/emulator-factory-inputs.generated.schema.json";
