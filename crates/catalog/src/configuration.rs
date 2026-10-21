//! Closed run-configuration documents kept outside the reference catalog.
//!
//! Configurations select body and firmware identities, a boot entry, execution
//! policy, synthetic reset inputs, and generated external scene inputs. They
//! do not own hardware maps, reference-document paths, firmware bytes, or
//! caller input paths.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RUN_CONFIGURATION_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/emulator-run-configuration.schema.json";
pub const RUN_OVERLAY_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/emulator-run-overlay.schema.json";

/// A validated, named run configuration for one resolved body and firmware.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunConfiguration {
    /// Stable schema URI for this run-configuration document.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Run-configuration document version.
    pub schema: u32,
    /// Stable run-configuration identity.
    pub id: String,
    /// Selected body identity.
    pub body_id: String,
    /// Selected firmware identity.
    pub firmware_id: String,
    /// Firmware entry mode selected for this run.
    pub start: RunStart,
    /// Execution policy for this run.
    pub execution: RunExecution,
    /// Declared synthetic reset inputs, addressed by catalog identity.
    pub inputs: RunInputs,
    /// External scene inputs generated for this run.
    pub scene: RunScene,
    /// Named overlays to apply in declaration order.
    pub overlays: Vec<String>,
}

impl RunConfiguration {
    /// Resolve a bundled run-configuration by its stable identity.
    pub fn bundled(id: &str) -> Result<Self, String> {
        crate::emulator::validate_id(id)?;
        let document = match id {
            "x70-110-rec" => include_str!("../../../configurations/emulator/x70-110-rec/run.json"),
            _ => return Err(format!("unsupported run configuration: {id}")),
        };
        let configuration: Self = serde_json::from_str(document)
            .map_err(|error| format!("run configuration {id}: {error}"))?;
        configuration.validate(id)?;
        Ok(configuration)
    }

    /// Check document identity and scene asset references before use.
    pub fn validate(&self, selected_id: &str) -> Result<(), String> {
        if self.schema != 1
            || self.schema_uri != RUN_CONFIGURATION_SCHEMA_URI
            || self.id != selected_id
        {
            return Err("run configuration schema or identity differs".into());
        }
        crate::emulator::validate_id(&self.id)?;
        crate::emulator::validate_id(&self.body_id)?;
        crate::emulator::validate_id(&self.firmware_id)?;
        let mut overlays = BTreeSet::new();
        for overlay in &self.overlays {
            crate::emulator::validate_id(overlay)?;
            if !overlays.insert(overlay) {
                return Err("run configuration repeats an overlay identity".into());
            }
        }
        if self.execution.quantum == 0 {
            return Err("run execution quantum must be positive".into());
        }
        self.execution.instructions_per_tick.validate()?;
        self.inputs.validate()?;
        self.scene.validate()
    }

    /// Check that every run-input identity exists in the selected hardware.
    pub fn validate_selection(
        &self,
        selection: &crate::reference::ResolvedSelection,
    ) -> Result<(), String> {
        if selection.body.id != self.body_id
            || selection.soc.id != selection.body.soc_id
            || selection.firmware.id != self.firmware_id
            || selection.firmware.body_id != self.body_id
        {
            return Err("run configuration identities differ from resolved hardware".into());
        }

        let body = &selection.body;
        let soc = &selection.soc;
        let block = |id: &str, kind| {
            soc.blocks
                .iter()
                .any(|candidate| candidate.id == id && candidate.kind == kind)
        };
        if !block(
            &self.inputs.gpio.block_id,
            crate::reference::SocBlockKind::Gpio,
        ) {
            return Err(format!(
                "run GPIO input refers to an absent or non-GPIO SoC block {}",
                self.inputs.gpio.block_id
            ));
        }
        if !block(
            &self.inputs.adc.block_id,
            crate::reference::SocBlockKind::Adc,
        ) {
            return Err(format!(
                "run ADC input refers to an absent or non-ADC SoC block {}",
                self.inputs.adc.block_id
            ));
        }

        let connection = |id: &str| body.connections.iter().find(|item| item.id == id);
        for key in &self.inputs.gpio.keys {
            if !connection(&key.connection_id)
                .is_some_and(|item| item.kind == crate::reference::ConnectionKind::Control)
            {
                return Err(format!(
                    "run key input refers to an absent or non-control body connection {}",
                    key.connection_id
                ));
            }
        }
        for state in &self.inputs.gpio.connections {
            if connection(&state.connection_id).is_none() {
                return Err(format!(
                    "run connection input refers to an absent body connection {}",
                    state.connection_id
                ));
            }
        }
        if !connection(&self.inputs.usb.connection_id)
            .is_some_and(|item| item.kind == crate::reference::ConnectionKind::Usb)
        {
            return Err(format!(
                "run USB input refers to an absent or non-USB body connection {}",
                self.inputs.usb.connection_id
            ));
        }

        let part = |id: &str| selection.parts.iter().find(|item| item.id == id);
        let mcu = body
            .microcontroller
            .as_ref()
            .filter(|item| item.part_id == self.inputs.mcu.part_id)
            .ok_or("run MCU input does not target the installed body MCU")?;
        let power = body
            .power_control
            .as_ref()
            .filter(|item| item.part_id == self.inputs.power.part_id)
            .ok_or("run power input does not target the installed body power controller")?;
        let card = body
            .card_slot
            .as_ref()
            .filter(|item| item.part_id == self.inputs.card.part_id)
            .ok_or("run card input does not target the installed body card slot")?;
        if self.inputs.gpio.connections.iter().any(|state| {
            state.connection_id == self.inputs.usb.connection_id
                || state.connection_id == card.detect_connection_id
                || state.connection_id == card.write_protect_connection_id
        }) {
            return Err("run connection states overlap USB or card-slot inputs".into());
        }

        let mcu_part = part(&mcu.part_id).ok_or("body MCU part is unresolved")?;
        let mcu_registers = match &mcu_part.model {
            crate::reference::PartModel::Microcontroller { registers } => registers,
            _ => return Err("body MCU attachment refers to a non-MCU part".into()),
        };
        if mcu_registers.width_bytes != 1 || mcu_registers.count < 16 {
            return Err(
                "body MCU part does not expose a sixteen-byte packet register range".into(),
            );
        }
        let power_part = part(&power.part_id).ok_or("body power-controller part is unresolved")?;
        let power_registers = match &power_part.model {
            crate::reference::PartModel::PowerControl { status_registers } => status_registers,
            _ => return Err("body power attachment refers to a non-power part".into()),
        };
        let card_part = part(&card.part_id).ok_or("body card-slot part is unresolved")?;
        if !matches!(&card_part.model, crate::reference::PartModel::SdCardSlot) {
            return Err("body card-slot attachment refers to a non-card part".into());
        }

        let register_in_range = |register: u8, range: &crate::reference::RegisterRange| {
            let first = u64::from(range.first);
            let end = first + u64::from(range.count);
            u64::from(register) >= first && u64::from(register) < end
        };
        for input in &self.inputs.power.spi_status {
            if !power_registers
                .iter()
                .any(|register| register.address == input.register)
            {
                return Err(format!(
                    "run power input targets an undeclared status register {}",
                    input.register
                ));
            }
        }
        for registers in &self.inputs.serial_registers {
            let target = part(&registers.part_id).ok_or_else(|| {
                format!(
                    "run input refers to an absent body part {}",
                    registers.part_id
                )
            })?;
            if !body
                .parts
                .iter()
                .any(|part_id| part_id == &registers.part_id)
            {
                return Err(format!(
                    "run input refers to an uninstalled body part {}",
                    registers.part_id
                ));
            }
            let range = match &target.model {
                crate::reference::PartModel::Microcontroller { registers } => Some(registers),
                crate::reference::PartModel::SerialOutput {
                    endpoint: crate::reference::SerialOutputKind::Registers,
                } => None,
                _ => {
                    return Err(format!(
                        "run serial-register input targets a non-register part {}",
                        registers.part_id
                    ));
                }
            };
            if let Some(range) = range {
                for value in &registers.values {
                    if !register_in_range(value.register, range) {
                        return Err(format!(
                            "run serial input register {} is outside part {}'s register range",
                            value.register, registers.part_id
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// A supported firmware start entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum RunStart {
    /// Reset through the selected firmware's real boot ROM.
    Rom,
    /// Enter through the selected firmware's loader entry.
    Loader,
}

/// Closed instruction-derived timebase ratio for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunInstructionRate {
    /// Instruction entries consumed per board tick.
    pub numerator: u64,
    /// Board ticks advanced per instruction entry.
    pub denominator: u64,
}

impl RunInstructionRate {
    fn validate(&self) -> Result<(), String> {
        if self.numerator == 0
            || self.denominator == 0
            || self.numerator > 1 << 32
            || self.denominator > 1 << 32
        {
            return Err("instruction-per-tick ratio must be positive and bounded".into());
        }
        Ok(())
    }
}

/// Synthetic starting input values applied to resolved blocks and parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunInputs {
    /// GPIO reset state and explicitly released controls.
    pub gpio: RunGpioInputs,
    /// ADC reset samples.
    pub adc: RunAdcInputs,
    /// Power-controller reset input.
    pub power: RunPowerInput,
    /// MCU persistent input and RTC start state.
    pub mcu: RunMcuInput,
    /// Other declared reset values for serial-backed parts.
    pub serial_registers: Vec<RunSerialRegisters>,
    /// Initial USB cable state.
    pub usb: RunUsbInput,
    /// Initial SD card insertion and protection state.
    pub card: RunCardInput,
}

impl RunInputs {
    fn validate(&self) -> Result<(), String> {
        crate::emulator::validate_id(&self.gpio.block_id)?;
        if self.gpio.keys.len() > 64 || self.gpio.connections.len() > 64 {
            return Err("run GPIO input count exceeds its bound".into());
        }
        let mut keys = BTreeSet::new();
        for key in &self.gpio.keys {
            crate::emulator::validate_id(&key.connection_id)?;
            if !keys.insert(key.connection_id.as_str()) {
                return Err("run GPIO repeats a control identity".into());
            }
        }
        let mut connections = BTreeSet::new();
        for connection in &self.gpio.connections {
            crate::emulator::validate_id(&connection.connection_id)?;
            if !connections.insert(connection.connection_id.as_str()) {
                return Err("run GPIO repeats a connection identity".into());
            }
        }
        crate::emulator::validate_id(&self.adc.block_id)?;
        if self.adc.samples.is_empty() || self.adc.samples.len() > 64 {
            return Err("run ADC sample count is outside its bound".into());
        }
        crate::emulator::validate_id(&self.power.part_id)?;
        if self.power.spi_status.len() > 128 {
            return Err("run power status input count exceeds its bound".into());
        }
        validate_register_values(&self.power.spi_status)?;
        crate::emulator::validate_id(&self.mcu.part_id)?;
        if self.mcu.packet[14] != 0xaa
            || self
                .mcu
                .packet
                .iter()
                .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
                != 0
        {
            return Err("run MCU packet has an invalid marker or checksum".into());
        }
        self.mcu.rtc.validate()?;
        if self.serial_registers.len() > 16 {
            return Err("run serial part count exceeds its bound".into());
        }
        let mut serial_parts = BTreeSet::new();
        for registers in &self.serial_registers {
            crate::emulator::validate_id(&registers.part_id)?;
            if !serial_parts.insert(registers.part_id.as_str()) || registers.values.len() > 128 {
                return Err("run serial register target is repeated or too large".into());
            }
            validate_register_values(&registers.values)?;
        }
        crate::emulator::validate_id(&self.usb.connection_id)?;
        crate::emulator::validate_id(&self.card.part_id)?;
        if !self.card.present && self.card.write_protected {
            return Err("an absent run card cannot be write protected".into());
        }
        Ok(())
    }
}

/// GPIO input block and semantic external states for the base run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunGpioInputs {
    /// SoC GPIO block identity.
    pub block_id: String,
    /// Explicit key/control states; released controls are listed as false.
    pub keys: Vec<RunKeyState>,
    /// External connection states resolved through body wiring.
    pub connections: Vec<RunConnectionState>,
}

/// Initial state of a physical key or body control connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunKeyState {
    /// Body connection identity for the control.
    pub connection_id: String,
    /// Whether the control is pressed at reset.
    pub pressed: bool,
}

/// Initial state of a physical body connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunConnectionState {
    /// Body connection identity.
    pub connection_id: String,
    /// Whether the external connection is present or electrically closed.
    pub connected: bool,
}

/// ADC block and reset sample vector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunAdcInputs {
    /// SoC ADC block identity.
    pub block_id: String,
    /// Per-channel samples at reset.
    pub samples: Vec<u16>,
}

/// Power-controller reset values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunPowerInput {
    /// Body power-controller part identity.
    pub part_id: String,
    /// Whether the camera power input is asserted.
    pub powered_on: bool,
    /// Synthetic boot reply byte.
    pub boot_reply: u8,
    /// Synthetic serial status values presented by the power controller.
    pub spi_status: Vec<RunRegisterValue>,
}

/// MCU packet, serial clock and RTC start state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunMcuInput {
    /// Body MCU-interface part identity.
    pub part_id: String,
    /// Synthetic sixteen-byte persistent settings packet.
    pub packet: [u8; 16],
    /// Synthetic RTC state at reset.
    pub rtc: RunRtcInput,
}

/// RTC values supplied at the start of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunRtcInput {
    /// Initial seconds in the device clock's declared epoch.
    pub initial_seconds: u32,
    /// Virtual ticks per RTC second.
    pub ticks_per_second: u64,
    /// Maximum supported seconds in the device clock's epoch.
    pub maximum_seconds: u32,
    /// Starting virtual tick for RTC progression.
    pub started: u64,
}

impl RunRtcInput {
    fn validate(&self) -> Result<(), String> {
        if self.ticks_per_second == 0 || self.initial_seconds > self.maximum_seconds {
            return Err("run RTC input has an invalid rate or range".into());
        }
        Ok(())
    }
}

/// Initial serial-register values for one resolved physical part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunSerialRegisters {
    /// Body part identity whose external registers receive these values.
    pub part_id: String,
    /// Register values at reset.
    pub values: Vec<RunRegisterValue>,
}

/// One synthetic external serial register value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunRegisterValue {
    /// External part register number.
    pub register: u8,
    /// Initial register byte.
    pub value: u8,
}

fn validate_register_values(values: &[RunRegisterValue]) -> Result<(), String> {
    let mut registers = BTreeSet::new();
    for value in values {
        if !registers.insert(value.register) {
            return Err("run serial input repeats a register".into());
        }
    }
    Ok(())
}

/// Initial USB cable state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunUsbInput {
    /// Body USB connection identity.
    pub connection_id: String,
    /// Whether a USB cable is present.
    pub connected: bool,
}

/// Initial removable-card state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunCardInput {
    /// Body card part identity.
    pub part_id: String,
    /// Whether a card is inserted.
    pub present: bool,
    /// Whether an inserted card is write protected.
    pub write_protected: bool,
}

/// A named, typed change to one run configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunOverlay {
    /// Stable schema URI for this run-overlay document.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Run-overlay document version.
    pub schema: u32,
    /// Stable run-overlay identity.
    pub id: String,
    /// Run configuration this overlay applies to.
    pub configuration_id: String,
    /// One typed run input change.
    pub change: RunOverlayChange,
}

impl RunOverlay {
    /// Check the overlay identity and its selected configuration.
    pub fn validate(&self, selected_id: &str, configuration_id: &str) -> Result<(), String> {
        if self.schema != 1
            || self.schema_uri != RUN_OVERLAY_SCHEMA_URI
            || self.id != selected_id
            || self.configuration_id != configuration_id
        {
            return Err("run overlay schema or identity differs".into());
        }
        crate::emulator::validate_id(&self.id)?;
        crate::emulator::validate_id(&self.configuration_id)
    }
}

/// The closed set of changes supported by run overlays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub enum RunOverlayChange {
    /// Connect or disconnect the declared USB cable.
    UsbConnection { connected: bool },
    /// Select another named firmware entry.
    Start { start: RunStart },
}

/// Execution policy selected by the run configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunExecution {
    /// Strictly model unsupported hardware and bus behavior.
    pub mode: RunExecutionMode,
    /// CPU engine selected for the configured processors.
    pub engine: RunEngine,
    /// Maximum instruction count scheduled in one batch.
    pub quantum: usize,
    /// Instruction-derived virtual-time ratio.
    pub instructions_per_tick: RunInstructionRate,
}

/// A supported run execution mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunExecutionMode {
    /// Refuse unsupported hardware or bus behavior.
    Strict,
}

/// A supported processor execution backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunEngine {
    /// Run the configured processors through the Unicorn backend.
    Unicorn,
}

/// Generated scene assets and the devices that consume them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunScene {
    /// Deterministic external byte assets generated from fixed patterns.
    pub generated_assets: Vec<GeneratedSceneAsset>,
    /// Image input presented to live-view hardware.
    pub images: RunImages,
    /// Statistics input presented to live-view hardware.
    pub statistics: RunStatistics,
    /// Initial frame clock period in virtual ticks.
    pub frame_period_ticks: u64,
}

impl RunScene {
    fn validate(&self) -> Result<(), String> {
        if self.generated_assets.is_empty() || self.generated_assets.len() > 256 {
            return Err("run scene generated-asset count is outside its bound".into());
        }
        let mut assets = BTreeMap::new();
        for asset in &self.generated_assets {
            asset.validate()?;
            if assets.insert(asset.id.as_str(), asset.size).is_some() {
                return Err("run scene repeats a generated asset identity".into());
            }
        }
        if self.frame_period_ticks == 0 {
            return Err("run scene frame period must be positive".into());
        }
        if self.images.live_image.width == 0
            || self.images.live_image.height == 0
            || self.images.live_image.stride < self.images.live_image.width
        {
            return Err("run scene image raster is invalid".into());
        }
        let image_bytes = self
            .images
            .live_image
            .stride
            .checked_mul(self.images.live_image.height)
            .ok_or("run scene image extent overflows")?;
        if assets.get(self.images.live_image.asset.as_str()) != Some(&image_bytes) {
            return Err("run scene image asset does not match its raster extent".into());
        }
        if self.statistics.live_statistics.len() > 256 {
            return Err("run scene has too many statistics spans".into());
        }
        for span in &self.statistics.live_statistics {
            crate::emulator::validate_id(&span.asset)?;
            if !assets.contains_key(span.asset.as_str()) {
                return Err("run scene statistics refer to an undeclared asset".into());
            }
        }
        Ok(())
    }
}

/// A generated run-scene input asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GeneratedSceneAsset {
    /// Stable asset identity scoped to the configuration.
    pub id: String,
    /// Repeated bytes used to generate the asset.
    pub pattern: Vec<u8>,
    /// Generated byte length.
    pub size: usize,
}

impl GeneratedSceneAsset {
    fn validate(&self) -> Result<(), String> {
        crate::emulator::validate_id(&self.id)?;
        if self.pattern.is_empty()
            || self.size == 0
            || self.size > 64 << 20
            || !self.size.is_multiple_of(self.pattern.len())
        {
            return Err("run scene generated asset has an invalid extent or pattern".into());
        }
        Ok(())
    }
}

/// The named image sources used by the run scene.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunImages {
    /// Live-view image and raster layout.
    pub live_image: RunImage,
}

/// A live-view image source and its raster layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunImage {
    /// Raster width in pixels.
    pub width: usize,
    /// Raster height in pixels.
    pub height: usize,
    /// Raster row stride in bytes.
    pub stride: usize,
    /// Generated scene asset identity.
    pub asset: String,
}

/// The named statistics sources used by the run scene.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunStatistics {
    /// Statistics spans presented to live-view hardware.
    pub live_statistics: Vec<RunStatisticsSpan>,
}

/// One statistics asset span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunStatisticsSpan {
    /// Statistics destination selected by the device implementation.
    pub destination: usize,
    /// Byte offset within the destination.
    pub offset: u32,
    /// Generated scene asset identity.
    pub asset: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_configuration_is_identity_checked_and_has_explicit_reset_inputs() {
        let config = RunConfiguration::bundled("x70-110-rec").unwrap();
        assert_eq!(config.id, "x70-110-rec");
        assert_eq!(config.body_id, "x70");
        assert_eq!(config.firmware_id, "x70-110");
        assert_eq!(config.start, RunStart::Rom);
        assert_eq!(config.execution.mode, RunExecutionMode::Strict);
        assert_eq!(
            config.execution.instructions_per_tick,
            RunInstructionRate {
                numerator: 24,
                denominator: 1
            }
        );
        assert_eq!(config.scene.frame_period_ticks, 400000);
        assert!(config.inputs.gpio.keys.iter().all(|key| !key.pressed));
        assert_eq!(config.inputs.adc.block_id, "adc");
        assert!(config.overlays.is_empty());
        assert!(RunConfiguration::bundled("unknown").is_err());
    }

    #[test]
    fn scene_asset_references_are_closed_and_bounded() {
        let mut config = RunConfiguration::bundled("x70-110-rec").unwrap();
        config.scene.images.live_image.asset = "missing".into();
        assert!(config.validate("x70-110-rec").is_err());
        config = RunConfiguration::bundled("x70-110-rec").unwrap();
        config.scene.generated_assets[0].pattern.clear();
        assert!(config.validate("x70-110-rec").is_err());
        assert!(config.validate("different").is_err());
    }

    #[test]
    fn run_inputs_resolve_to_typed_blocks_connections_and_parts() {
        let config = RunConfiguration::bundled("x70-110-rec").unwrap();
        let catalog = crate::reference::ReferenceCatalog::bundled().unwrap();
        let selection = catalog.resolve("x70", "x70-110").unwrap();
        assert!(config.validate_selection(&selection).is_ok());

        let mut invalid = config.clone();
        invalid.inputs.adc.block_id = invalid.inputs.gpio.block_id.clone();
        assert!(invalid.validate_selection(&selection).is_err());

        let mut invalid = config.clone();
        invalid.inputs.usb.connection_id = "hdmi".into();
        assert!(invalid.validate_selection(&selection).is_err());

        let mut invalid = config;
        invalid.inputs.mcu.part_id = invalid.inputs.power.part_id.clone();
        assert!(invalid.validate_selection(&selection).is_err());
    }

    #[test]
    fn overlay_is_bound_to_its_configuration_and_has_a_typed_change() {
        let overlay = RunOverlay {
            schema_uri: RUN_OVERLAY_SCHEMA_URI.into(),
            schema: 1,
            id: "x70-110-rec-usb".into(),
            configuration_id: "x70-110-rec".into(),
            change: RunOverlayChange::UsbConnection { connected: true },
        };
        assert!(overlay.validate("x70-110-rec-usb", "x70-110-rec").is_ok());
        assert!(overlay.validate("x70-110-rec-usb", "different").is_err());
        assert!(overlay.validate("different", "x70-110-rec").is_err());
    }

    #[test]
    fn statistics_offsets_are_destination_offsets() {
        let mut config = RunConfiguration::bundled("x70-110-rec").unwrap();
        config.scene.statistics.live_statistics[0].offset = u32::MAX;
        assert!(config.validate("x70-110-rec").is_ok());
    }
}
