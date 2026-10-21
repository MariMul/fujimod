//! Physical controls and generated firmware input/observation facts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Physical controls documented for a body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyControls {
    /// Published manual identifying the physical controls.
    pub manual: ManualReference,
    /// Number of analog sample levels exposed by the controls.
    pub analog_levels: u16,
    /// Physical controls keyed by stable control identity.
    pub controls: BTreeMap<String, BodyControl>,
}

/// A published manual and the pages used to identify controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManualReference {
    /// Public manual URL.
    pub url: String,
    /// Manual title.
    pub title: String,
    /// Referenced page numbers.
    pub pages: Vec<u32>,
}

/// A physical control's identity, positions, and body connections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyControl {
    /// Printed or documented control name.
    pub name: String,
    /// Mechanical control category.
    pub kind: ControlKind,
    /// Physical position on the camera body.
    pub location: ControlLocation,
    /// Documented mechanical positions.
    pub positions: Vec<ControlPosition>,
    /// Whether the control can be pressed.
    pub pushable: bool,
    /// Documented default function when available.
    pub default_function: Option<String>,
    /// Whether firmware allows its function to be assigned.
    pub assignable: bool,
    /// Relevant manual page when available.
    pub manual_page: Option<u32>,
    /// Physical body connections used by this control.
    pub connection_ids: Vec<String>,
}

/// Physical control categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    /// Rotary dial with named positions.
    Dial,
    /// Momentary button.
    Button,
    /// Mechanical lever.
    Lever,
    /// Selector switch.
    Switch,
    /// Lens or body ring.
    Ring,
    /// Two-stage shutter release.
    Shutter,
    /// Relative command wheel.
    Wheel,
    /// Directional selector.
    Joystick,
    /// Touch-sensitive display.
    Touchscreen,
}

/// Locations used to identify physical controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlLocation {
    /// Top surface.
    Top,
    /// Front surface.
    Front,
    /// Rear surface.
    Rear,
    /// Lens assembly.
    Lens,
    /// Bottom surface.
    Bottom,
}

/// A named mechanical position of a physical control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControlPosition {
    /// Stable position identity or printed value.
    pub id: String,
    /// Printed label when available.
    pub label: Option<String>,
    /// Meaning of an icon or otherwise unlabeled position.
    pub description: Option<String>,
}

/// Firmware key scanning and logical control mappings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmwareControls {
    /// Logical behavior keyed by physical body control identity.
    pub controls: BTreeMap<String, FirmwareControl>,
    /// Whether named debug key events are available.
    pub debug_keys_available: bool,
    /// Firmware calibration and scanner readers.
    pub input_readers: ControlReaders,
    /// Firmware scanner table identity and extent.
    pub scanner: ScannerTable,
    /// Firmware native event names and their table identity.
    pub native_key_names: NativeKeyNames,
}

/// A firmware mapping for one physical control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmwareControl {
    /// Logical key identities emitted by this control.
    pub keys: Vec<String>,
    /// Electrical predicates keyed by logical key identity.
    pub input: Option<BTreeMap<String, Vec<ControlInput>>>,
    /// Named logical actions and their required keys.
    pub actions: BTreeMap<String, ControlAction>,
    /// Firmware key combinations for named positions.
    pub positions: Vec<FirmwareControlPosition>,
    /// Firmware protocol interface for controls outside the key scanner.
    pub interface: Option<ControlInterface>,
    /// Reason no electronic mapping is established, when applicable.
    pub unavailable_reason: Option<String>,
}

/// Firmware interpretation of a control outside the key scanner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlInterface {
    /// Power switch sampled through a physical GPIO and serial status field.
    PowerSwitch {
        /// Body connection carrying the switch input.
        connection_id: String,
        /// Firmware address retaining sampled GPIO state.
        saved_inputs: u64,
        /// Firmware serial endpoint selector.
        endpoint: u32,
        /// Firmware-selected status register.
        register: u32,
        /// Mask of status bits interpreted by the firmware.
        mask: u32,
        /// Masked status value interpreted as on.
        on_value: u32,
        /// Masked status value interpreted as off.
        off_value: u32,
    },
    /// Touch reports read through a prefixed control-bus endpoint.
    PrefixedTouch {
        /// Body connection carrying control-bus traffic.
        connection_id: String,
        /// Firmware control-bus transaction mode.
        mode: u32,
        /// First report register read by the firmware.
        register: u32,
        /// Report byte count consumed by the firmware.
        report_bytes: u32,
        /// Maximum simultaneous contacts decoded by the firmware.
        contacts: u32,
        /// Number of encoded coordinate bits.
        coordinate_bits: u32,
        /// Firmware logical coordinate width.
        width: u32,
        /// Firmware logical coordinate height.
        height: u32,
        /// Configuration-byte offsets for logical coordinate correction.
        offset_tags: [u32; 2],
        /// Firmware interrupt line selected for reports.
        irq: u32,
        /// Firmware-selected interrupt priority.
        priority: u32,
        /// Firmware-selected interrupt mode.
        irq_mode: u32,
    },
}

/// A logical action expressed through named keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControlAction {
    /// Logical keys selected by the action.
    pub keys: Vec<String>,
}

/// Firmware key selection for a named control position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirmwareControlPosition {
    /// Position identity or printed value.
    pub position: String,
    /// Logical keys selecting this position.
    pub keys: Vec<String>,
    /// Whether this position asserts the associated binary control.
    pub active: Option<bool>,
}

/// An electrical predicate referencing physical body wiring by identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlInput {
    /// Analog input with bounds obtained from firmware configuration.
    Adc {
        /// Body connection carrying this analog input.
        connection_id: String,
        /// Calibration-table intervals accepted by this predicate.
        intervals: Vec<CalibrationInterval>,
        /// Firmware callback that evaluates this input, when present.
        callback: Option<u64>,
        /// Callback result accepted as active, when present.
        expected_result: Option<u32>,
        /// Scanner record index within the firmware scanner table.
        scanner_index: u32,
        /// Firmware key-event number associated with the record.
        event: u32,
    },
    /// Digital input compared with a firmware-selected level.
    Gpio {
        /// Body connection carrying this digital input.
        connection_id: String,
        /// Logic level interpreted as the selected key.
        expected_level: bool,
        /// Scanner predicate used to interpret the digital input.
        scanner: GpioScanner,
        /// Scanner record index within the firmware scanner table.
        scanner_index: u32,
        /// Firmware key-event number associated with the record.
        event: u32,
    },
    /// Firmware quadrature decoder attached to a physical encoder.
    GpioQuadrature {
        /// Body connection carrying the encoder phases.
        connection_id: String,
        /// Decoder phase mapping.
        phase_map: [u8; 4],
        /// Decoder transition values.
        transitions: [u8; 16],
        /// Decoder reset counters and phase state.
        reset: [u32; 4],
        /// Decoder event selected by this logical key.
        event: u32,
        /// Firmware decoder guard address.
        guard: u64,
        /// Decoder activation threshold.
        threshold: u32,
    },
}

/// Firmware scanner predicates used for digital keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GpioScanner {
    /// Direct GPIO comparison using the declared expected logic level.
    Direct,
    /// Combined scanner predicate with its firmware-selected source and bounds.
    Combined {
        /// Scanner source selector.
        source: u32,
        /// Lower comparison operand.
        lower: u16,
        /// Upper comparison operand.
        upper: u16,
    },
}

/// Bounds stored as little-endian 16-bit values in firmware configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalibrationInterval {
    /// Byte offset of the lower bound from the configuration base.
    pub lower: u32,
    /// Byte offset of the upper bound from the configuration base.
    pub upper: u32,
}

/// Firmware routines used to read calibration and stable scanner state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControlReaders {
    /// Routine reading 16-bit control calibration values.
    pub calibration: u64,
    /// Routine reading stable key scanner state.
    pub scanner: ScannerReader,
}

/// A firmware scanner state reader.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScannerReader {
    /// Stable-state field offset within a scanner record.
    pub stable_offset: u32,
    /// Reader routine identity.
    pub routine: AddressIdentity,
}

/// A firmware table used to scan physical input predicates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScannerTable {
    /// Table base address.
    pub base: u64,
    /// Number of scanner records.
    pub count: u32,
    /// Scanner record size in bytes.
    pub stride: u32,
    /// Routine that looks up scanner entries.
    pub lookup: u64,
    /// SHA-256 of the scanner table.
    pub sha256: String,
}

/// Native firmware event names and the identity of their source table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeKeyNames {
    /// Native event names and their numeric identities.
    pub rows: Vec<NativeKeyName>,
    /// Routine used to read the table.
    pub reader: u64,
    /// Table byte length.
    pub size: u64,
    /// SHA-256 of the table.
    pub sha256: String,
}

/// One named native firmware event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeKeyName {
    /// Numeric event identity.
    pub event: u32,
    /// Native event name.
    pub name: String,
    /// Address of the native name string.
    pub string_address: u64,
}

/// An identified firmware address range without its contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddressIdentity {
    /// Start address.
    pub address: u64,
    /// Byte length.
    pub size: u64,
    /// SHA-256 of the range.
    pub sha256: String,
}

/// A configuration block addressed directly or through a firmware pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfigurationLocation {
    /// Firmware address containing a configuration-block pointer.
    Pointer {
        /// Pointer address.
        address: u64,
    },
    /// Fixed configuration-block base address.
    Direct {
        /// Block address.
        address: u64,
    },
}

/// Firmware UI mode numbers beyond the REC and setup modes used by boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdditionalUiModes {
    /// Playback UI mode.
    pub play: u32,
    /// Playback setup UI mode.
    pub setup_play: u32,
}

/// Firmware key identities used to navigate menus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MenuKeys {
    /// Accept or open-menu key identity.
    pub menu: String,
    /// Back key identity.
    pub back: String,
    /// Up key identity.
    pub up: String,
    /// Down key identity.
    pub down: String,
}

impl BodyControls {
    /// Check physical control identities and their body connection references.
    pub fn validate(&self, connections: &BTreeSet<&str>) -> Result<(), String> {
        if self.analog_levels == 0 || self.manual.url.is_empty() || self.manual.title.is_empty() {
            return Err("body controls require analog levels and a manual identity".into());
        }
        for (id, control) in &self.controls {
            crate::emulator::validate_id(id)?;
            if control.name.is_empty() {
                return Err(format!("body control {id} has no name"));
            }
            let mut seen = BTreeSet::new();
            for connection in &control.connection_ids {
                if !connections.contains(connection.as_str()) || !seen.insert(connection) {
                    return Err(format!(
                        "body control {id} has an absent or repeated connection {connection}"
                    ));
                }
            }
        }
        Ok(())
    }
}

impl AddressIdentity {
    /// Validate the digest and nonempty extent of a firmware range identity.
    pub fn validate(&self) -> Result<(), String> {
        if self.size == 0 || self.address.checked_add(self.size).is_none() {
            return Err("firmware range identity has an empty or overflowing extent".into());
        }
        crate::emulator::Content {
            size: self.size,
            sha256: self.sha256.clone(),
        }
        .validate()
    }
}

impl FirmwareControls {
    /// Check scanner extents and references to the selected body's controls.
    pub fn validate(&self, body: &crate::reference::Body) -> Result<(), String> {
        use crate::reference::ConnectionSignal;
        let physical = body
            .controls
            .as_ref()
            .ok_or("firmware controls require body controls")?;
        if self.scanner.count == 0
            || self.scanner.stride == 0
            || self
                .scanner
                .base
                .checked_add(u64::from(self.scanner.count) * u64::from(self.scanner.stride))
                .is_none()
        {
            return Err("firmware scanner has an empty or overflowing extent".into());
        }
        crate::emulator::Content {
            size: u64::from(self.scanner.count) * u64::from(self.scanner.stride),
            sha256: self.scanner.sha256.clone(),
        }
        .validate()?;
        self.input_readers.scanner.routine.validate()?;
        crate::emulator::Content {
            size: self.native_key_names.size,
            sha256: self.native_key_names.sha256.clone(),
        }
        .validate()?;
        let mut events = BTreeSet::new();
        for row in &self.native_key_names.rows {
            if row.name.is_empty() || !events.insert(row.event) {
                return Err("firmware native key names are empty or repeat an event".into());
            }
        }
        for (id, control) in &self.controls {
            let installed = physical
                .controls
                .get(id)
                .ok_or_else(|| format!("firmware refers to missing body control {id}"))?;
            if let Some(interface) = &control.interface {
                let connection_id = match interface {
                    ControlInterface::PowerSwitch { connection_id, .. }
                    | ControlInterface::PrefixedTouch { connection_id, .. } => connection_id,
                };
                let connection = body
                    .connections
                    .iter()
                    .find(|candidate| candidate.id == *connection_id)
                    .filter(|_| installed.connection_ids.contains(connection_id))
                    .ok_or_else(|| {
                        format!("firmware interface {id} has no assigned body connection")
                    })?;
                match interface {
                    ControlInterface::PowerSwitch {
                        mask,
                        on_value,
                        off_value,
                        ..
                    } => {
                        if !connection
                            .signals
                            .iter()
                            .any(|signal| matches!(signal, ConnectionSignal::Gpio { .. }))
                            || *mask == 0
                            || on_value & !mask != 0
                            || off_value & !mask != 0
                            || on_value == off_value
                        {
                            return Err(
                                "firmware power switch has invalid wiring or status values".into(),
                            );
                        }
                    }
                    ControlInterface::PrefixedTouch {
                        width,
                        height,
                        contacts,
                        report_bytes,
                        coordinate_bits,
                        ..
                    } => {
                        if !connection
                            .signals
                            .iter()
                            .any(|signal| matches!(signal, ConnectionSignal::Serial { .. }))
                            || *width == 0
                            || *height == 0
                            || *contacts == 0
                            || *report_bytes == 0
                            || !(1..=32).contains(coordinate_bits)
                        {
                            return Err(
                                "firmware touch interface has invalid wiring or report geometry"
                                    .into(),
                            );
                        }
                    }
                }
            }
            for (key, predicates) in control.input.iter().flat_map(|inputs| inputs.iter()) {
                if !control.keys.contains(key) || predicates.is_empty() {
                    return Err(format!(
                        "firmware control {id} has an unknown or empty key predicate {key}"
                    ));
                }
                for predicate in predicates {
                    let (connection_id, scanner_index) = match predicate {
                        ControlInput::Adc {
                            connection_id,
                            scanner_index,
                            ..
                        }
                        | ControlInput::Gpio {
                            connection_id,
                            scanner_index,
                            ..
                        } => (connection_id, Some(*scanner_index)),
                        ControlInput::GpioQuadrature { connection_id, .. } => (connection_id, None),
                    };
                    if !installed.connection_ids.contains(connection_id) {
                        return Err(format!(
                            "firmware control {id} refers to an unassigned connection {connection_id}"
                        ));
                    }
                    let connection = body
                        .connections
                        .iter()
                        .find(|candidate| candidate.id == *connection_id)
                        .ok_or_else(|| {
                            format!("firmware input connection {connection_id} is absent")
                        })?;
                    if scanner_index.is_some_and(|index| index >= self.scanner.count) {
                        return Err(format!("firmware input {key} exceeds the scanner table"));
                    }
                    match predicate {
                        ControlInput::Adc {
                            intervals,
                            callback,
                            expected_result,
                            ..
                        } => {
                            if !connection
                                .signals
                                .iter()
                                .any(|signal| matches!(signal, ConnectionSignal::Adc { .. }))
                                || intervals.is_empty()
                                || intervals.iter().any(|interval| {
                                    interval.lower > u32::from(u16::MAX) - 1
                                        || interval.upper > u32::from(u16::MAX) - 1
                                })
                                || callback.is_some() != expected_result.is_some()
                            {
                                return Err(format!(
                                    "firmware ADC predicate {key} has invalid wiring, calibration, or callback"
                                ));
                            }
                        }
                        ControlInput::Gpio { .. } => {
                            if !connection
                                .signals
                                .iter()
                                .any(|signal| matches!(signal, ConnectionSignal::Gpio { .. }))
                            {
                                return Err(format!(
                                    "firmware GPIO predicate {key} has no GPIO wiring"
                                ));
                            }
                        }
                        ControlInput::GpioQuadrature { .. } => {
                            if connection
                                .signals
                                .iter()
                                .filter(|signal| matches!(signal, ConnectionSignal::Gpio { .. }))
                                .count()
                                != 2
                            {
                                return Err(format!(
                                    "firmware encoder predicate {key} requires two GPIO phases"
                                ));
                            }
                        }
                    }
                }
            }
            for keys in control
                .actions
                .values()
                .map(|action| &action.keys)
                .chain(control.positions.iter().map(|position| &position.keys))
            {
                if keys.iter().any(|key| !control.keys.contains(key)) {
                    return Err(format!("firmware control {id} selects an undeclared key"));
                }
            }
        }
        Ok(())
    }
}
