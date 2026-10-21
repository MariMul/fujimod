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

include!(concat!(env!("OUT_DIR"), "/bundled_configurations.rs"));

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
        let document = BUNDLED_RUN_CONFIGURATIONS
            .iter()
            .find_map(|(candidate, document)| (*candidate == id).then_some(*document))
            .ok_or_else(|| format!("unsupported run configuration: {id}"))?;
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
        for encoder in &self.inputs.encoders {
            let input = connection(&encoder.connection_id)
                .filter(|item| item.kind == crate::reference::ConnectionKind::Control)
                .ok_or_else(|| {
                    format!(
                        "run encoder input refers to an absent or non-control connection {}",
                        encoder.connection_id
                    )
                })?;
            let gpio_signals = input
                .signals
                .iter()
                .filter(|signal| matches!(signal, crate::reference::ConnectionSignal::Gpio { .. }))
                .count();
            if gpio_signals != 2 {
                return Err(format!(
                    "run encoder input {} must resolve to two GPIO signals",
                    encoder.connection_id
                ));
            }
        }
        for output in &self.inputs.gpio.outputs {
            let target = connection(&output.connection_id).ok_or_else(|| {
                format!(
                    "run GPIO output refers to an absent body connection {}",
                    output.connection_id
                )
            })?;
            if !matches!(
                target.signals.get(usize::from(output.signal_index)),
                Some(crate::reference::ConnectionSignal::Gpio { .. })
            ) {
                return Err(format!(
                    "run GPIO output {} signal {} is absent or non-GPIO",
                    output.connection_id, output.signal_index
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
            .filter(|item| item.connection_id == self.inputs.card.connection_id)
            .ok_or("run card input does not target the body's card connection")?;
        if self.inputs.card.present && card.controller_id.is_none() {
            return Err("selected body has no supported removable-card controller".into());
        }
        if self.inputs.gpio.connections.iter().any(|state| {
            state.connection_id == self.inputs.usb.connection_id
                || state.connection_id == card.detect_connection_id
                || state.connection_id == card.write_protect_connection_id
        }) {
            return Err("run connection states overlap USB or card-slot inputs".into());
        }

        let mcu_part = part(&mcu.part_id).ok_or("body MCU part is unresolved")?;
        let (mcu_registers, packet_registers, clock_registers) = match &mcu_part.model {
            crate::reference::PartModel::Microcontroller {
                registers,
                packet_registers,
                clock_registers,
                ..
            } => (registers, packet_registers, clock_registers),
            _ => return Err("body MCU attachment refers to a non-MCU part".into()),
        };
        if mcu_registers.width_bytes != 1
            || mcu_registers.count == 0
            || packet_registers.width_bytes != 1
            || packet_registers.count != 16
            || clock_registers.width_bytes != 1
            || clock_registers.count != 4
        {
            return Err("body MCU part has an unsupported packet or clock register layout".into());
        }
        let in_range = |range: &crate::reference::RegisterRange, register: u32| {
            register >= range.first && register < range.first.saturating_add(range.count)
        };
        let packet_overlaps_clock = (0..packet_registers.count)
            .map(|offset| packet_registers.first.saturating_add(offset))
            .any(|register| in_range(clock_registers, register));
        let packet_overlaps_extent = packet_registers.first < mcu_registers.first
            || packet_registers
                .first
                .saturating_add(packet_registers.count)
                > mcu_registers.first.saturating_add(mcu_registers.count);
        let clock_overlaps_extent = clock_registers.first < mcu_registers.first
            || clock_registers.first.saturating_add(clock_registers.count)
                > mcu_registers.first.saturating_add(mcu_registers.count);
        if packet_overlaps_clock || packet_overlaps_extent || clock_overlaps_extent {
            return Err("body MCU sub-register layout overlaps or exceeds its extent".into());
        }
        let power_part = part(&power.part_id).ok_or("body power-controller part is unresolved")?;
        let power_registers = match &power_part.model {
            crate::reference::PartModel::PowerControl { status_registers } => status_registers,
            _ => return Err("body power attachment refers to a non-power part".into()),
        };
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
        for serial_input in &self.inputs.serial_registers {
            let target = part(&serial_input.part_id).ok_or_else(|| {
                format!(
                    "run input refers to an absent body part {}",
                    serial_input.part_id
                )
            })?;
            if !body
                .parts
                .iter()
                .any(|part_id| part_id == &serial_input.part_id)
            {
                return Err(format!(
                    "run input refers to an uninstalled body part {}",
                    serial_input.part_id
                ));
            }
            let range = match &target.model {
                crate::reference::PartModel::Microcontroller {
                    registers: register_range,
                    packet_registers,
                    clock_registers,
                    status_register,
                    initial_registers,
                } => {
                    for value in &serial_input.values {
                        let address = u32::from(value.register);
                        if in_range(packet_registers, address)
                            || in_range(clock_registers, address)
                            || (*status_register != Some(address)
                                && !initial_registers.contains(&address))
                        {
                            return Err(format!(
                                "run serial input register {} has no declared initialization role",
                                value.register
                            ));
                        }
                    }
                    Some(register_range)
                }
                crate::reference::PartModel::SerialOutput {
                    endpoint: crate::reference::SerialOutputKind::Registers,
                } => None,
                _ => {
                    return Err(format!(
                        "run serial-register input targets a non-register part {}",
                        serial_input.part_id
                    ));
                }
            };
            if let Some(range) = range {
                for value in &serial_input.values {
                    if !register_in_range(value.register, range) {
                        return Err(format!(
                            "run serial input register {} is outside part {}'s register range",
                            value.register, serial_input.part_id
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// A run configuration and the hardware and firmware identities it resolves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ResolvedConfiguration {
    /// Validated run configuration selected by identity.
    pub configuration: RunConfiguration,
    /// Resolved SoC, body, firmware and installed parts.
    pub reference: crate::reference::ResolvedSelection,
}

impl ResolvedConfiguration {
    /// Load a bundled run configuration and resolve all selected catalog IDs.
    pub fn bundled(id: &str) -> Result<Self, String> {
        let configuration = RunConfiguration::bundled(id)?;
        let overlays = configuration.overlays.clone();
        Self::resolve_and_apply(configuration, &overlays)
    }

    /// Resolve a base run configuration and apply its named input overlays.
    pub fn with_overlays(configuration_id: &str, overlay_ids: &[String]) -> Result<Self, String> {
        let configuration = RunConfiguration::bundled(configuration_id)?;
        let mut selected = configuration.overlays.clone();
        selected.extend(overlay_ids.iter().cloned());
        Self::resolve_and_apply(configuration, &selected)
    }

    fn resolve_and_apply(
        mut configuration: RunConfiguration,
        overlay_ids: &[String],
    ) -> Result<Self, String> {
        let catalog = crate::reference::ReferenceCatalog::bundled()?;
        let reference = catalog.resolve(&configuration.body_id, &configuration.firmware_id)?;
        configuration.validate_selection(&reference)?;
        let mut applied = BTreeSet::new();
        let mut changed = BTreeSet::new();
        configuration.overlays.clear();
        for overlay_id in overlay_ids {
            if !applied.insert(overlay_id.as_str()) {
                return Err(format!(
                    "run overlay {overlay_id} is selected more than once"
                ));
            }
            let overlay = RunOverlay::bundled(overlay_id)?;
            overlay.validate(overlay_id, &configuration.id)?;
            match overlay.change {
                RunOverlayChange::UsbConnection { connected } => {
                    if !changed.insert("usb") {
                        return Err("run overlays change USB connection more than once".into());
                    }
                    configuration.inputs.usb.connected = connected;
                }
                RunOverlayChange::Start { start } => {
                    if !changed.insert("start") {
                        return Err("run overlays change the start entry more than once".into());
                    }
                    configuration.start = start;
                }
            }
            configuration.overlays.push(overlay.id);
        }
        configuration.validate_selection(&reference)?;
        Ok(Self {
            configuration,
            reference,
        })
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
    /// Initial phases for physical quadrature encoders.
    pub encoders: Vec<RunEncoderInput>,
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
    /// Policy for accepting a firmware first-start prompt, when needed.
    pub first_start: Option<RunFirstStart>,
}

impl RunInputs {
    fn validate(&self) -> Result<(), String> {
        crate::emulator::validate_id(&self.gpio.block_id)?;
        if self.gpio.keys.len() > 64
            || self.gpio.connections.len() > 64
            || self.gpio.outputs.len() > 256
        {
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
        let mut outputs = BTreeSet::new();
        for output in &self.gpio.outputs {
            crate::emulator::validate_id(&output.connection_id)?;
            if !outputs.insert((output.connection_id.as_str(), output.signal_index)) {
                return Err("run GPIO repeats an output signal identity".into());
            }
        }
        if self.encoders.len() > 64 {
            return Err("run encoder input count exceeds its bound".into());
        }
        let mut encoders = BTreeSet::new();
        for encoder in &self.encoders {
            crate::emulator::validate_id(&encoder.connection_id)?;
            if encoder.phase > 3 || !encoders.insert(encoder.connection_id.as_str()) {
                return Err("run encoder input has a repeated identity or invalid phase".into());
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
        crate::emulator::validate_id(&self.card.connection_id)?;
        if !self.card.present && self.card.write_protected {
            return Err("an absent run card cannot be write protected".into());
        }
        if let Some(first_start) = &self.first_start {
            first_start.validate()?;
        }
        Ok(())
    }
}

/// Run policy for one logical key used to accept the first-start prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunFirstStart {
    /// Firmware logical key identity, such as `EXE_M`.
    pub key_id: String,
    /// Virtual ticks to hold the input active.
    pub hold_ticks: u64,
    /// Virtual ticks to wait after releasing the input.
    pub settle_ticks: u64,
    /// Maximum input presses accepted for this startup policy.
    pub max_presses: u8,
}

impl RunFirstStart {
    fn validate(&self) -> Result<(), String> {
        crate::emulator::validate_id(&self.key_id)?;
        if self.hold_ticks == 0 || self.settle_ticks == 0 || self.max_presses == 0 {
            return Err("run first-start timing and press limit must be positive".into());
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
    /// Startup input level supplied for pads without a named assignment.
    /// This is a run assumption, not a measured hardware fact.
    pub unassigned_input_level: bool,
    /// Explicit key/control states; released controls are listed as false.
    pub keys: Vec<RunKeyState>,
    /// External connection states resolved through body wiring.
    pub connections: Vec<RunConnectionState>,
    /// Known GPIO output-latch states addressed through body wiring.
    pub outputs: Vec<RunGpioOutputState>,
}

/// Initial output-latch value for one GPIO signal in a body connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunGpioOutputState {
    /// Body connection identity for the output signal.
    pub connection_id: String,
    /// Zero-based signal index in the body connection.
    pub signal_index: u8,
    /// Output level latched at reset.
    pub high: bool,
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

/// Initial phase for a body-backed quadrature encoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct RunEncoderInput {
    /// Body connection identity carrying both encoder signals.
    pub connection_id: String,
    /// Initial two-bit encoder phase in the range zero through three.
    pub phase: u8,
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
    /// Body connection identity for the removable-card slot.
    pub connection_id: String,
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
    /// Resolve a bundled run overlay by its stable identity.
    pub fn bundled(id: &str) -> Result<Self, String> {
        crate::emulator::validate_id(id)?;
        let document = BUNDLED_RUN_OVERLAYS
            .iter()
            .find_map(|(candidate, document)| (*candidate == id).then_some(*document))
            .ok_or_else(|| format!("unsupported run overlay: {id}"))?;
        let overlay: Self =
            serde_json::from_str(document).map_err(|error| format!("run overlay {id}: {error}"))?;
        overlay.validate(id, &overlay.configuration_id)?;
        Ok(overlay)
    }

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
        assert!(!config.inputs.gpio.unassigned_input_level);
        assert!(config.inputs.gpio.keys.iter().all(|key| !key.pressed));
        assert_eq!(config.inputs.gpio.outputs.len(), 3);
        assert!(config.inputs.gpio.outputs.iter().all(|output| output.high));
        assert_eq!(config.inputs.encoders.len(), 1);
        assert_eq!(
            config.inputs.encoders[0].connection_id,
            "controls-encoder-3-6-3-7"
        );
        assert_eq!(config.inputs.encoders[0].phase, 0);
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

        let mut invalid = config.clone();
        invalid.inputs.encoders[0].connection_id = "s1".into();
        assert!(invalid.validate_selection(&selection).is_err());

        let mut invalid = config.clone();
        invalid.inputs.gpio.outputs[0].signal_index = 2;
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

        let bundled = RunOverlay::bundled("x70-110-rec-usb").unwrap();
        assert_eq!(bundled.configuration_id, "x70-110-rec");
        assert_eq!(
            bundled.change,
            RunOverlayChange::UsbConnection { connected: true }
        );
        let resolved =
            ResolvedConfiguration::with_overlays("x70-110-rec", &["x70-110-rec-usb".into()])
                .unwrap();
        assert!(resolved.configuration.inputs.usb.connected);
        assert_eq!(resolved.configuration.overlays, ["x70-110-rec-usb"]);
    }

    #[test]
    fn statistics_offsets_are_destination_offsets() {
        let mut config = RunConfiguration::bundled("x70-110-rec").unwrap();
        config.scene.statistics.live_statistics[0].offset = u32::MAX;
        assert!(config.validate("x70-110-rec").is_ok());
    }
}
