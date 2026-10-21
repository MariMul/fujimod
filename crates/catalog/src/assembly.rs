//! Physical attachments and reusable endpoint protocols for camera assemblies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Additional body-specific physical wiring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BodyHardware {
    /// Serial endpoints and their processor-side connections.
    pub serial_attachments: Vec<SerialAttachment>,
    /// Connections between GPIO output and input pads.
    pub gpio_feedback: Vec<GpioFeedback>,
    /// Named keypad positions and their declared analog sample levels.
    pub keypad_samples: BTreeMap<String, u16>,
    /// Body connections carrying thermal sensor voltages.
    pub thermal_connection_ids: Vec<String>,
}

/// A physical serial endpoint attached to one SoC controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SerialAttachment {
    /// Installed part identity.
    pub part_id: String,
    /// SoC serial-controller block identity.
    pub controller_id: String,
    /// Connection carrying endpoint data.
    pub connection_id: String,
    /// Connection selecting the endpoint, when declared.
    pub select_connection_id: Option<String>,
}

/// A declared electrical feedback connection between GPIO pads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GpioFeedback {
    /// Body connection exposing the output pad.
    pub output_connection_id: String,
    /// Body connection exposing the input pad.
    pub input_connection_id: String,
}

impl BodyHardware {
    /// Validate installed parts and all physical connection identities.
    pub fn validate(&self, body: &crate::reference::Body) -> Result<(), String> {
        let connection = |id: &str| {
            body.connections
                .iter()
                .any(|item| item.id == id)
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "body {} hardware refers to missing connection {id}",
                        body.id
                    )
                })
        };
        for attachment in &self.serial_attachments {
            if !body.parts.contains(&attachment.part_id) {
                return Err(format!(
                    "body {} serial attachment refers to an uninstalled part",
                    body.id
                ));
            }
            connection(&attachment.connection_id)?;
            if let Some(id) = &attachment.select_connection_id {
                connection(id)?;
            }
        }
        for feedback in &self.gpio_feedback {
            connection(&feedback.output_connection_id)?;
            connection(&feedback.input_connection_id)?;
        }
        for id in &self.thermal_connection_ids {
            connection(id)?;
        }
        Ok(())
    }
}

/// Reusable endpoint protocols with no processor addresses or startup state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum PeripheralPart {
    /// Focus actuator accepting serial commands.
    FocusActuator,
    /// Iris actuator accepting serial commands.
    IrisActuator,
    /// Audio serial output endpoint.
    AudioSerialOutput,
    /// Imaging serial output endpoint.
    ImagingSerialOutput,
    /// Image sensor whose serial register selects the readout gate.
    ImagingSensor {
        /// Sensor register controlling readout.
        read_gate_register: u16,
        /// Bits enabling the readout gate.
        read_gate_mask: u8,
    },
    /// Power-controller register protocol.
    PowerController,
    /// Serial calibration-register protocol.
    SerialCalibrationRegisters,
    /// BIS configuration-register protocol.
    BisConfigurationRegisters,
    /// LSM6DS configuration-register protocol.
    Lsm6dsConfigurationRegisters,
    /// Serial NOR flash geometry and device identification, when declared.
    SerialNorFlash {
        /// JEDEC identification value, when supplied by the part definition.
        jedec_id: Option<u32>,
        /// Erase-unit capacity in bytes, when declared.
        erase_bytes: Option<u64>,
        /// Program-page capacity in bytes, when declared.
        page_bytes: Option<u64>,
    },
}
