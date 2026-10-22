//! Physical control declarations and firmware-located input identities.
//!
//! A binding names one vocabulary explicitly. Numeric event identities establish
//! the checked relationship; spelling is never used to translate vocabularies.

use crate::facts::{
    CalibrationInterval, ControlInterface, GpioScanner, ManualReference, NativeKeyNames,
    ScannerReader, ScannerRecordWitness, ScannerTable,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PHYSICAL_CONTROLS_SCHEMA_URI: &str =
    "https://fujimod.com/schemas/physical-controls.schema.json";
pub const CONTROL_IDS_SCHEMA_URI: &str = "https://fujimod.com/schemas/control-ids.schema.json";

/// One explicitly located command in the firmware's debug transport vocabulary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct DebugKey {
    /// Exact command token accepted by that transport.
    pub name: String,
    /// UI event identity delivered by the command.
    pub event: u32,
}

/// Hand-maintained physical controls for one body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct PhysicalControls {
    /// Schema identifying this document's closed Rust type.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Document format version.
    pub schema: u32,
    /// Stable document identity referenced by the body.
    pub id: String,
    /// Body owning the documented controls.
    pub body_id: String,
    /// Published manual identifying the controls.
    pub manual: ManualReference,
    /// Analog sample resolution used by this body's contacts.
    pub analog_levels: u16,
    /// Controls keyed by their canonical physical identities.
    pub controls: BTreeMap<String, PhysicalControl>,
}

/// A control's documented appearance and declared physical actions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct PhysicalControl {
    /// Manual name of the control.
    pub name: String,
    /// Surface on which the control is located.
    pub location: ControlSurface,
    /// Manual page identifying the control, when available.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub manual_page: Option<u32>,
    /// Text printed on the control, when present.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub face: Option<String>,
    /// Meaning of a printed icon, when present.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub icon: Option<String>,
    /// Body connections carrying the control's electrical signals.
    pub connection_ids: Vec<String>,
    /// Whether the control's function can be assigned in the camera.
    pub assignable: bool,
    /// Documented initial function, when available.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub default_function: Option<String>,
    /// Physical role required by a declared cross-body operation.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub role: Option<ControlRole>,
    /// Whether this control has no electronic action.
    pub mechanical: bool,
    /// Actions defined by this control's mechanism; absent for mechanical controls.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub behavior: Option<ControlBehavior>,
}

/// Physical roles that an operation may require from a body.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum ControlRole {
    /// Relative command dial used to change the selected value.
    CommandControl,
}

/// Body surfaces used to locate a physical control in a manual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum ControlSurface {
    /// Upper body surface.
    Top,
    /// Rear body surface.
    Rear,
    /// Front surface, including controls on the lens.
    Front,
    /// Lower body surface.
    Bottom,
}

/// An explicit binding in one of two distinct firmware input vocabularies.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum InputBinding {
    /// Command name accepted by the firmware's debug transport.
    DebugKey {
        /// Exact command token, without conversion from a native event name.
        name: String,
    },
    /// Scanned native input which need not have any debug transport command.
    NativeInput {
        /// Exact name in the firmware's located native input vocabulary.
        name: String,
    },
}

/// The actions offered by a physical control's mechanism.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum ControlBehavior {
    /// A momentary button.
    Button {
        /// Input asserted by a press and deasserted by release.
        press: InputBinding,
    },
    /// A shutter release with named stages.
    Shutter {
        /// Inputs asserted at each named shutter stage.
        stages: BTreeMap<String, Vec<InputBinding>>,
    },
    /// A directional pad with individually addressable direction identities.
    Pad {
        /// Inputs asserted by the four directions.
        #[cfg_attr(
            feature = "typescript",
            tsify(type = "Partial<Record<PadDirection, InputBinding>>")
        )]
        directions: BTreeMap<PadDirection, InputBinding>,
        /// Center press input, when present.
        #[cfg_attr(feature = "typescript", serde(default))]
        press: Option<InputBinding>,
    },
    /// A dial with positions or relative steps.
    Dial {
        /// Documented positions or relative step directions.
        motion: ControlMotion,
        /// Press input, when the dial is pushable.
        #[cfg_attr(feature = "typescript", serde(default))]
        press: Option<InputBinding>,
        /// Whether the mechanism rotates without an end stop.
        endless: bool,
    },
    /// A ring with positions or relative steps.
    Ring {
        /// Documented positions or relative step directions.
        motion: ControlMotion,
        /// Press input, when the ring is pushable.
        #[cfg_attr(feature = "typescript", serde(default))]
        press: Option<InputBinding>,
        /// Whether the mechanism rotates without an end stop.
        endless: bool,
    },
    /// A switch with named positions.
    Switch {
        /// Inputs selecting each documented position.
        positions: Vec<BoundPosition>,
    },
    /// A lever with named positions.
    Lever {
        /// Inputs selecting each documented position.
        positions: Vec<BoundPosition>,
    },
    /// A touch surface whose protocol is located in the firmware input document.
    Touch,
}

/// A rotary mechanism's mutually exclusive position or step declarations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum ControlMotion {
    /// Finite, named mechanical detents.
    Positions {
        /// Input and asserted level for each detent.
        positions: Vec<BoundPosition>,
    },
    /// Relative movement in either direction.
    Steps {
        /// Input generated by one decreasing step.
        decrease: InputBinding,
        /// Input generated by one increasing step.
        increase: InputBinding,
    },
}

/// A position's printed identity and explicit input selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct BoundPosition {
    /// Exact position identity accepted by the physical panel.
    pub id: String,
    /// Printed label, when present.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub label: Option<String>,
    /// Meaning of an unlabeled position or icon.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub description: Option<String>,
    /// Explicit firmware input binding.
    pub key: InputBinding,
    /// Whether this position asserts that input.
    pub asserted: bool,
}

/// Directions addressable as `<pad-id>.<direction>`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum PadDirection {
    /// Upper direction.
    Up,
    /// Lower direction.
    Down,
    /// Left direction.
    Left,
    /// Right direction.
    Right,
}

impl PadDirection {
    /// Canonical final component of a direction's control identity.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// Generated firmware input facts, separate from the body's declarations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct FirmwareControlIds {
    /// Schema identifying the generated document's closed Rust type.
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    /// Document format version.
    pub schema: u32,
    /// Stable document identity referenced by the firmware.
    pub id: String,
    /// Firmware from which these inputs were located.
    pub firmware_id: String,
    /// Scanner table identity and extent.
    pub scanner: ScannerTable,
    /// Calibration and stable-state readers located in that firmware.
    pub input_readers: InputReaders,
    /// Native names and their source table identity, when the table was located.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub native_key_names: Option<NativeKeyNames>,
    /// Exact debug command vocabulary, independent of the native names.
    pub debug_keys: Vec<DebugKey>,
    /// Every located scanner or protocol input, including unclaimed connections.
    pub inputs: Vec<LocatedInput>,
    /// Protocols outside the key scanner, keyed by physical body connection ID.
    pub interfaces: BTreeMap<String, ControlInterface>,
}

/// Input routines located in one firmware; a routine the locators did not find is absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct InputReaders {
    /// Routine reading 16-bit control calibration values.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub calibration: Option<u64>,
    /// Routine reading stable key scanner state.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub scanner: Option<ScannerReader>,
}

/// One located input's identity, ownership, and checked electrical predicates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct LocatedInput {
    /// Stable identity of the located input record.
    pub id: String,
    /// UI/native event identity established by the locator.
    pub event: u32,
    /// Exact debug command token for this input, absent for native-only inputs.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub debug_key: Option<String>,
    /// Physical control, connection detector, or unclaimed firmware input.
    pub target: InputTarget,
    /// Scanner row and its source witness, absent for inputs outside that table.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub scanner: Option<LocatedScannerRecord>,
    /// Located electrical interpretation, absent when the hardware is not modeled.
    #[cfg_attr(feature = "typescript", serde(default))]
    pub electrical: Option<ElectricalInput>,
}

/// One scanner row's identity and located predicate operands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub struct LocatedScannerRecord {
    /// Zero-based index within the identified scanner table.
    pub index: u32,
    /// Event identity, predicate kind, source selector, and bounds stored in the row.
    pub witness: ScannerRecordWitness,
}

/// Located electrical meaning, separate from scanner table operands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum ElectricalInput {
    /// Analog input interpreted using located calibration intervals.
    Adc {
        /// Body connection carrying this analog input.
        connection_id: String,
        /// Configuration offsets for the accepted analog intervals.
        intervals: Vec<CalibrationInterval>,
        /// Firmware callback evaluating the input, when present.
        #[cfg_attr(feature = "typescript", serde(default))]
        callback: Option<u64>,
        /// Callback result interpreted as active, when present.
        #[cfg_attr(feature = "typescript", serde(default))]
        expected_result: Option<u32>,
    },
    /// Digital contact interpreted by a located scanner predicate.
    Gpio {
        /// Body connection carrying this digital input.
        connection_id: String,
        /// Logic level interpreted as active.
        expected_level: bool,
        /// Firmware interpretation of the contact.
        scanner: GpioScanner,
    },
    /// Physical encoder interpreted by a located quadrature decoder.
    GpioQuadrature {
        /// Body connection carrying the encoder phases.
        connection_id: String,
        /// Decoder phase mapping.
        phase_map: [u8; 4],
        /// Decoder transition values.
        transitions: [u8; 16],
        /// Decoder reset counters and phase state.
        reset: [u32; 4],
        /// Decoder-local direction event, distinct from the UI event.
        decoder_event: u32,
        /// Located decoder guard address.
        guard: u64,
        /// Located decoder activation threshold.
        threshold: u32,
    },
    /// Physical encoder whose firmware counts whole phase cycles, one event per cycle.
    GpioFixedCycle {
        /// Body connection carrying the encoder phases.
        connection_id: String,
        /// Sign of the physical phase cycle that produces this input's event.
        cycle_sign: i8,
    },
}

/// Ownership of a generated input record without a control alias map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(
    feature = "typescript",
    derive(tsify::Tsify),
    tsify(hashmap_as_object, missing_as_null)
)]
pub enum InputTarget {
    /// Input claimed by a canonical physical control identity.
    Control {
        /// Exact control identity, including a pad direction where applicable.
        control_id: String,
    },
    /// Card, cable, door, or other connection detector.
    Connection {
        /// Exact connection identity declared by the body.
        connection_id: String,
    },
    /// Located firmware input which no documented physical control claims.
    Unclaimed,
}

/// A checked reference to one exact physical control or pad direction.
#[derive(Clone, Copy, Debug)]
pub struct ControlRef<'a> {
    /// Canonical control metadata.
    pub control: &'a PhysicalControl,
    /// Addressed pad direction, absent for a top-level control identity.
    pub direction: Option<PadDirection>,
}

/// An operation requested using the same identity as the installed physical panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlOperation {
    /// Assert a momentary input.
    Press,
    /// Release a momentary input or all shutter stages.
    Release,
    /// Select a named shutter stage.
    Stage {
        /// Exact stage identity.
        id: String,
    },
    /// Select a named detent or switch position.
    Position {
        /// Exact position identity.
        id: String,
    },
    /// Take a signed number of relative steps.
    Step {
        /// Negative decreases; positive increases; zero is refused.
        delta: i32,
    },
}

/// A declared physical action resolved against one firmware's located inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedControlAction {
    /// Exact identity supplied to the checked physical panel.
    pub control_id: String,
    /// Requested physical operation.
    pub operation: ControlOperation,
    /// Number of times to apply the selected movement.
    pub repetitions: u32,
    /// Asserted level for the selected input bindings.
    pub asserted: bool,
    /// Located inputs named by the declaration; no event names are inferred.
    pub inputs: Vec<LocatedInput>,
}

impl PhysicalControls {
    fn bindings(&self) -> Vec<(String, &InputBinding)> {
        let mut bindings = Vec::new();
        for (id, control) in &self.controls {
            let Some(behavior) = &control.behavior else {
                continue;
            };
            match behavior {
                ControlBehavior::Pad { directions, press } => {
                    for (direction, binding) in directions {
                        bindings.push((format!("{id}.{}", direction.id()), binding));
                    }
                    if let Some(binding) = press {
                        bindings.push((id.clone(), binding));
                    }
                }
                ControlBehavior::Button { press } => bindings.push((id.clone(), press)),
                ControlBehavior::Shutter { stages } => {
                    for binding in stages.values().flatten() {
                        bindings.push((id.clone(), binding));
                    }
                }
                ControlBehavior::Dial { motion, press, .. }
                | ControlBehavior::Ring { motion, press, .. } => {
                    if let Some(binding) = press {
                        bindings.push((id.clone(), binding));
                    }
                    match motion {
                        ControlMotion::Positions { positions } => {
                            for position in positions {
                                bindings.push((id.clone(), &position.key));
                            }
                        }
                        ControlMotion::Steps { decrease, increase } => {
                            bindings.push((id.clone(), decrease));
                            bindings.push((id.clone(), increase));
                        }
                    }
                }
                ControlBehavior::Switch { positions } | ControlBehavior::Lever { positions } => {
                    for position in positions {
                        bindings.push((id.clone(), &position.key));
                    }
                }
                ControlBehavior::Touch => {}
            }
        }
        bindings
    }

    /// Look up a canonical identity, including a declared pad direction.
    pub fn control(&self, id: &str) -> Result<ControlRef<'_>, String> {
        if let Some(control) = self.controls.get(id) {
            return Ok(ControlRef {
                control,
                direction: None,
            });
        }
        if let Some((parent, member)) = id.split_once('.')
            && let Some(control) = self.controls.get(parent)
            && let Some(ControlBehavior::Pad { directions, .. }) = &control.behavior
            && let Some(direction) = directions.keys().find(|direction| direction.id() == member)
        {
            return Ok(ControlRef {
                control,
                direction: Some(*direction),
            });
        }
        Err(format!("unknown physical control identity {id}"))
    }

    /// Validate identity, mechanism declarations, and unique physical roles.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.schema_uri != PHYSICAL_CONTROLS_SCHEMA_URI {
            return Err("physical controls schema differs".into());
        }
        crate::reference::validate_id(&self.id)?;
        crate::reference::validate_id(&self.body_id)?;
        if self.analog_levels < 2
            || self.controls.is_empty()
            || self.manual.url.is_empty()
            || self.manual.title.is_empty()
            || self.manual.pages.is_empty()
        {
            return Err(
                "physical controls require a manual, controls, and analog resolution".into(),
            );
        }
        let mut roles = BTreeSet::new();
        for (id, control) in &self.controls {
            if id.is_empty()
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            {
                return Err(format!("invalid physical control identity {id}"));
            }
            if control.name.trim().is_empty()
                || control.manual_page == Some(0)
                || (control.mechanical && control.behavior.is_some())
            {
                return Err(format!("physical control {id} has an invalid declaration"));
            }
            if let Some(role) = control.role
                && !roles.insert(role)
            {
                return Err(format!("physical role {role:?} is assigned more than once"));
            }
            let mut connection_ids = BTreeSet::new();
            if control
                .connection_ids
                .iter()
                .any(|id| !connection_ids.insert(id))
            {
                return Err(format!("physical control {id} repeats a body connection"));
            }
            if let Some(behavior) = &control.behavior {
                behavior.validate()?;
            }
        }
        Ok(())
    }

    /// Resolve a declared action against checked firmware input facts.
    pub fn action(
        &self,
        firmware: &FirmwareControlIds,
        id: &str,
        operation: ControlOperation,
    ) -> Result<ResolvedControlAction, String> {
        let selected = self.control(id)?;
        let behavior = selected
            .control
            .behavior
            .as_ref()
            .ok_or_else(|| format!("physical control {id} has no bound actions"))?;
        let (bindings, asserted, repetitions) =
            behavior.bindings(selected.direction, &operation)?;
        let inputs = bindings
            .into_iter()
            .map(|binding| firmware.input(id, binding).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedControlAction {
            control_id: id.into(),
            operation,
            repetitions,
            asserted,
            inputs,
        })
    }
}

impl ControlBehavior {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Shutter { stages } => {
                if stages.is_empty()
                    || stages
                        .iter()
                        .any(|(id, inputs)| id.is_empty() || inputs.is_empty())
                {
                    return Err("shutter requires named stages with explicit inputs".into());
                }
            }
            Self::Pad { directions, .. } => {
                if directions.len() != 4 {
                    return Err("pad requires all four declared directions".into());
                }
            }
            Self::Switch { positions }
            | Self::Lever { positions }
            | Self::Dial {
                motion: ControlMotion::Positions { positions },
                ..
            }
            | Self::Ring {
                motion: ControlMotion::Positions { positions },
                ..
            } => {
                let mut ids = BTreeSet::new();
                if positions.is_empty()
                    || positions
                        .iter()
                        .any(|p| p.id.is_empty() || !ids.insert(&p.id))
                {
                    return Err("control positions must have distinct nonempty identities".into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn bindings<'a>(
        &'a self,
        direction: Option<PadDirection>,
        operation: &ControlOperation,
    ) -> Result<(Vec<&'a InputBinding>, bool, u32), String> {
        let press = match self {
            Self::Button { press } => Some(press),
            Self::Pad { directions, press } => direction
                .and_then(|d| directions.get(&d))
                .or(press.as_ref()),
            Self::Dial { press, .. } | Self::Ring { press, .. } => press.as_ref(),
            _ => None,
        };
        match operation {
            ControlOperation::Press | ControlOperation::Release if press.is_some() => Ok((
                vec![press.expect("checked press")],
                matches!(operation, ControlOperation::Press),
                1,
            )),
            ControlOperation::Release => match self {
                Self::Shutter { stages } => {
                    let inputs: BTreeSet<_> = stages.values().flatten().collect();
                    Ok((inputs.into_iter().collect(), false, 1))
                }
                _ => Err("control has no momentary release action".into()),
            },
            ControlOperation::Stage { id } => match self {
                Self::Shutter { stages } => stages
                    .get(id)
                    .map(|inputs| (inputs.iter().collect(), true, 1))
                    .ok_or_else(|| format!("unknown shutter stage {id}")),
                _ => Err("control is not a staged shutter".into()),
            },
            ControlOperation::Position { id } => {
                let positions = match self {
                    Self::Switch { positions }
                    | Self::Lever { positions }
                    | Self::Dial {
                        motion: ControlMotion::Positions { positions },
                        ..
                    }
                    | Self::Ring {
                        motion: ControlMotion::Positions { positions },
                        ..
                    } => positions,
                    _ => return Err("control has no named positions".into()),
                };
                let position = positions
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| format!("unknown control position {id}"))?;
                Ok((vec![&position.key], position.asserted, 1))
            }
            ControlOperation::Step { delta } if *delta != 0 => match self {
                Self::Dial {
                    motion: ControlMotion::Steps { decrease, increase },
                    ..
                }
                | Self::Ring {
                    motion: ControlMotion::Steps { decrease, increase },
                    ..
                } => Ok((
                    vec![if *delta < 0 { decrease } else { increase }],
                    true,
                    delta.unsigned_abs(),
                )),
                _ => Err("control has no relative step action".into()),
            },
            ControlOperation::Step { .. } => Err("control step must be nonzero".into()),
            _ => Err("control has no press action".into()),
        }
    }
}

impl FirmwareControlIds {
    /// Validate every scanner row, vocabulary reference, and physical input claim.
    pub fn validate(
        &self,
        physical: &PhysicalControls,
        connections: &BTreeSet<&str>,
    ) -> Result<(), String> {
        physical.validate()?;
        if self.schema != 1 || self.schema_uri != CONTROL_IDS_SCHEMA_URI {
            return Err("firmware control IDs schema differs".into());
        }
        crate::reference::validate_id(&self.id)?;
        crate::reference::validate_id(&self.firmware_id)?;
        for (id, control) in &physical.controls {
            if control
                .connection_ids
                .iter()
                .any(|connection| !connections.contains(connection.as_str()))
            {
                return Err(format!(
                    "physical control {id} names an unknown body connection"
                ));
            }
        }
        let mut debug_names = BTreeSet::new();
        for key in &self.debug_keys {
            if key.name.is_empty() || !debug_names.insert(&key.name) {
                return Err("firmware debug command names are empty or duplicated".into());
            }
        }
        let mut native_names = BTreeSet::new();
        for key in self.native_key_names.iter().flat_map(|names| &names.rows) {
            if key.name.is_empty() || !native_names.insert(&key.name) {
                return Err("firmware native input names are empty or duplicated".into());
            }
        }
        if let Some(reader) = &self.input_readers.scanner {
            reader.routine.validate()?;
            if reader.stable_width != 1
                || reader.stable_offset >= self.scanner.stride
                || reader.pressed_value == reader.released_value
            {
                return Err("firmware stable-state reader is invalid".into());
            }
        }
        crate::reference::validate_sha256(&self.scanner.sha256)?;
        if let Some(names) = &self.native_key_names {
            crate::reference::validate_sha256(&names.sha256)?;
        }
        if self.scanner.count == 0
            || self.scanner.stride < 20
            || self
                .scanner
                .base
                .checked_add(u64::from(self.scanner.count) * u64::from(self.scanner.stride))
                .is_none()
        {
            return Err("firmware input table is invalid".into());
        }
        let mut ids = BTreeSet::new();
        let mut rows = BTreeSet::new();
        for input in &self.inputs {
            crate::reference::validate_id(&input.id)?;
            if !ids.insert(&input.id) {
                return Err(format!("duplicate located input {}", input.id));
            }
            if let Some(key) = &input.debug_key
                && !self
                    .debug_keys
                    .iter()
                    .any(|candidate| candidate.name == *key && candidate.event == input.event)
            {
                return Err(format!(
                    "input {} names an undeclared debug key or event",
                    input.id
                ));
            }
            match &input.target {
                InputTarget::Control { control_id } => {
                    if physical.control(control_id)?.control.mechanical {
                        return Err(format!(
                            "mechanical control {control_id} claims an electronic input"
                        ));
                    }
                }
                InputTarget::Connection { connection_id } => {
                    if !connections.contains(connection_id.as_str()) {
                        return Err(format!(
                            "input {} names an unknown connection {connection_id}",
                            input.id
                        ));
                    }
                }
                InputTarget::Unclaimed => {}
            }
            if let Some(scanner) = &input.scanner
                && (scanner.index >= self.scanner.count
                    || !rows.insert(scanner.index)
                    || scanner.witness.physical_key != input.event)
            {
                return Err(format!(
                    "input {} has an invalid or repeated scanner row",
                    input.id
                ));
            }
            if let Some(electrical) = &input.electrical {
                let connection_id = match electrical {
                    ElectricalInput::Adc { connection_id, .. }
                    | ElectricalInput::Gpio { connection_id, .. }
                    | ElectricalInput::GpioQuadrature { connection_id, .. } => connection_id,
                    | ElectricalInput::GpioFixedCycle { connection_id, .. } => connection_id,
                };
                if !connections.contains(connection_id.as_str()) {
                    return Err(format!(
                        "input {} has unknown electrical connection {connection_id}",
                        input.id
                    ));
                }
                if let InputTarget::Control { control_id } = &input.target
                    && !physical
                        .control(control_id)?
                        .control
                        .connection_ids
                        .contains(connection_id)
                {
                    return Err(format!(
                        "input {} uses a connection not assigned to {control_id}",
                        input.id
                    ));
                }
                if let ElectricalInput::GpioFixedCycle { cycle_sign, .. } = electrical
                    && !matches!(cycle_sign, -1 | 1)
                {
                    return Err(format!("input {} has an invalid encoder cycle sign", input.id));
                }
                if matches!(
                    electrical,
                    ElectricalInput::Adc { .. } | ElectricalInput::Gpio { .. }
                ) && input.scanner.is_none()
                {
                    return Err(format!(
                        "input {} lacks its electrical scanner witness",
                        input.id
                    ));
                }
            }
        }
        if rows.len() != self.scanner.count as usize {
            return Err("generated input document does not retain every scanner row".into());
        }
        for (control_id, binding) in physical.bindings() {
            self.input(&control_id, binding)?;
        }
        for connection_id in self.interfaces.keys() {
            if !connections.contains(connection_id.as_str()) {
                return Err(format!(
                    "firmware input interface names unknown connection {connection_id}"
                ));
            }
        }
        Ok(())
    }

    /// Resolve one explicit vocabulary name and require exact physical ownership.
    pub fn input(&self, control_id: &str, binding: &InputBinding) -> Result<&LocatedInput, String> {
        let event = match binding {
            InputBinding::DebugKey { name } => {
                let mut matches = self.debug_keys.iter().filter(|key| key.name == *name);
                let event = matches
                    .next()
                    .ok_or_else(|| format!("unknown firmware debug key {name}"))?
                    .event;
                if matches.next().is_some() {
                    return Err(format!("duplicate firmware debug key {name}"));
                }
                event
            }
            InputBinding::NativeInput { name } => {
                let mut matches = self
                    .native_key_names
                    .iter()
                    .flat_map(|names| &names.rows)
                    .filter(|key| key.name == *name);
                let event = matches
                    .next()
                    .ok_or_else(|| format!("unknown firmware native input {name}"))?
                    .event;
                if matches.next().is_some() {
                    return Err(format!("duplicate firmware native input {name}"));
                }
                event
            }
        };
        let mut matches = self.inputs.iter().filter(|input| input.event == event
            && matches!(&input.target, InputTarget::Control { control_id: owner } if owner == control_id));
        let input = matches.next().ok_or_else(|| {
            format!("firmware input {event} is not claimed by control {control_id}")
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "firmware control {control_id} has ambiguous input {event}"
            ));
        }
        if let InputBinding::DebugKey { name } = binding
            && input.debug_key.as_ref() != Some(name)
        {
            return Err(format!(
                "firmware input {event} does not declare debug key {name}"
            ));
        }
        if matches!(binding, InputBinding::NativeInput { .. }) && input.debug_key.is_some() {
            return Err(format!(
                "native-only input {} unexpectedly declares a debug transport key",
                input.id
            ));
        }
        Ok(input)
    }
}

impl ResolvedControlAction {
    /// Return declared debug commands, refusing native-only input before transport.
    pub fn debug_keys(&self) -> Result<Vec<&str>, String> {
        self.inputs
            .iter()
            .map(|input| {
                input.debug_key.as_deref().ok_or_else(|| {
                    format!(
                        "control {} has no debug transport key for input {}",
                        self.control_id, input.id
                    )
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn control(behavior: Value) -> Value {
        json!({"name":"Documented control", "location":"rear", "manual_page":3,
            "face":null,"icon":null,"connection_ids":["contact"],"assignable":false,
            "default_function":null,"role":null,"mechanical":false,"behavior":behavior})
    }

    fn fixture() -> (PhysicalControls, FirmwareControlIds) {
        let debug = |name| json!({"kind":"debug_key","name":name});
        let native = |name| json!({"kind":"native_input","name":name});
        let physical = serde_json::from_value(json!({
            "$schema":PHYSICAL_CONTROLS_SCHEMA_URI,"schema":1,"id":"body-controls","body_id":"body",
            "manual":{"url":"https://example.com/manual","title":"Camera manual","pages":[3]},
            "analog_levels":4096,"controls":{
                "menu_ok":control(json!({"kind":"button","press":debug("EXE_M")})),
                "movie":control(json!({"kind":"button","press":native("KEY_FUNCTION3")})),
                "selector":control(json!({"kind":"pad","directions":{
                    "up":debug("UP"),"down":debug("DOWN"),"left":debug("LEFT"),"right":debug("RIGHT")},"press":null})),
                "command_control":control(json!({"kind":"dial","motion":{"kind":"steps",
                    "decrease":debug("CMD_LEFT"),"increase":debug("CMD_RIGHT")},"press":null,"endless":true}))
            }
        })).unwrap();
        let names = [
            "EXE_M",
            "UP",
            "DOWN",
            "LEFT",
            "RIGHT",
            "CMD_LEFT",
            "CMD_RIGHT",
        ];
        let mut debug_keys = Vec::new();
        let mut inputs = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let event = index as u32 + 1;
            debug_keys.push(json!({"name":name,"event":event}));
            let id = match *name {
                "EXE_M" => "menu_ok",
                "UP" => "selector.up",
                "DOWN" => "selector.down",
                "LEFT" => "selector.left",
                "RIGHT" => "selector.right",
                _ => "command_control",
            };
            inputs.push(
                json!({"id":format!("input-{event}"),"event":event,"debug_key":name,
                "target":{"kind":"control","control_id":id},"scanner":null,"electrical":null}),
            );
        }
        inputs.push(json!({"id":"input-movie","event":61,"debug_key":null,
            "target":{"kind":"control","control_id":"movie"},"electrical":null,
            "scanner":{"index":0,"witness":{"physical_key":61,"input_kind":"kind_2","source":11,"lower":0,"upper":0}}}));
        let firmware = serde_json::from_value(json!({
            "$schema":CONTROL_IDS_SCHEMA_URI,"schema":1,"id":"firmware-control-ids","firmware_id":"firmware",
            "scanner":{"base":4096,"count":1,"stride":28,"lookup":8192,"sha256":"0".repeat(64)},
            "input_readers":{"calibration":12288,"scanner":{"stable_offset":21,"stable_width":1,"pressed_value":1,
                "released_value":0,"routine":{"address":8192,"size":4,"sha256":"0".repeat(64)}}},
            "native_key_names":{"rows":[{"event":61,"name":"KEY_FUNCTION3","string_address":16384},
                {"event":1,"name":"KEY_OK","string_address":16400}],"reader":16380,"size":32,"sha256":"0".repeat(64)},
            "debug_keys":debug_keys,"inputs":inputs,"interfaces":{}
        })).unwrap();
        (physical, firmware)
    }

    #[test]
    fn exact_pad_member_identity_resolves_without_aliases() {
        let (physical, firmware) = fixture();
        firmware
            .validate(&physical, &BTreeSet::from(["contact"]))
            .unwrap();
        assert_eq!(
            physical.control("selector.up").unwrap().direction,
            Some(PadDirection::Up)
        );
        let action = physical
            .action(&firmware, "selector.up", ControlOperation::Press)
            .unwrap();
        assert_eq!(action.control_id, "selector.up");
        assert_eq!(action.debug_keys().unwrap(), ["UP"]);
        assert!(physical.control("selector.UP").is_err());
        assert!(physical.control("selector_function_buttons.up").is_err());
        assert!(
            physical
                .action(&firmware, "selector", ControlOperation::Press)
                .is_err()
        );
    }

    #[test]
    fn native_only_action_remains_physical_and_refuses_debug_transport() {
        let (physical, firmware) = fixture();
        let action = physical
            .action(&firmware, "movie", ControlOperation::Press)
            .unwrap();
        assert!(action.asserted);
        assert_eq!(action.inputs[0].event, 61);
        assert_eq!(action.inputs[0].debug_key, None);
        assert!(
            action
                .debug_keys()
                .unwrap_err()
                .contains("no debug transport key")
        );
        assert!(
            !physical
                .action(&firmware, "movie", ControlOperation::Release)
                .unwrap()
                .asserted
        );
        assert!(
            firmware
                .input(
                    "movie",
                    &InputBinding::DebugKey {
                        name: "FUNCTION3".into()
                    }
                )
                .is_err()
        );
        assert!(
            firmware
                .input(
                    "menu_ok",
                    &InputBinding::DebugKey {
                        name: "KEY_OK".into()
                    }
                )
                .is_err()
        );
        assert!(
            firmware
                .input(
                    "menu_ok",
                    &InputBinding::NativeInput {
                        name: "EXE_M".into()
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn debug_preflight_refuses_a_later_native_only_input() {
        let (mut physical, mut firmware) = fixture();
        physical.controls.remove("menu_ok");
        let mut shutter = physical.controls.remove("movie").unwrap();
        shutter.behavior = Some(ControlBehavior::Shutter {
            stages: BTreeMap::from([(
                "full".into(),
                vec![
                    InputBinding::DebugKey {
                        name: "EXE_M".into(),
                    },
                    InputBinding::NativeInput {
                        name: "KEY_FUNCTION3".into(),
                    },
                ],
            )]),
        });
        physical.controls.insert("shutter".into(), shutter);
        for input in &mut firmware.inputs {
            if input.event == 1 || input.event == 61 {
                input.target = InputTarget::Control {
                    control_id: "shutter".into(),
                };
            }
        }
        firmware
            .validate(&physical, &BTreeSet::from(["contact"]))
            .unwrap();
        let action = physical
            .action(
                &firmware,
                "shutter",
                ControlOperation::Stage { id: "full".into() },
            )
            .unwrap();
        assert_eq!(action.inputs.len(), 2);
        assert_eq!(action.inputs[0].debug_key.as_deref(), Some("EXE_M"));
        assert!(action.debug_keys().is_err());
    }

    #[test]
    fn signed_step_selects_declared_binding_and_count() {
        let (physical, firmware) = fixture();
        let action = physical
            .action(
                &firmware,
                "command_control",
                ControlOperation::Step { delta: -3 },
            )
            .unwrap();
        assert_eq!(action.repetitions, 3);
        assert_eq!(action.debug_keys().unwrap(), ["CMD_LEFT"]);
        assert_eq!(
            physical
                .action(
                    &firmware,
                    "command_control",
                    ControlOperation::Step { delta: 2 }
                )
                .unwrap()
                .debug_keys()
                .unwrap(),
            ["CMD_RIGHT"]
        );
        assert!(
            physical
                .action(
                    &firmware,
                    "command_control",
                    ControlOperation::Step { delta: 0 }
                )
                .is_err()
        );
    }

    #[test]
    fn input_claims_and_all_scanner_rows_are_checked() {
        let (physical, mut firmware) = fixture();
        firmware.inputs.last_mut().unwrap().target = InputTarget::Unclaimed;
        assert!(
            firmware
                .validate(&physical, &BTreeSet::from(["contact"]))
                .unwrap_err()
                .contains("not claimed")
        );
        let (_, mut firmware) = fixture();
        firmware.scanner.count = 2;
        assert!(
            firmware
                .validate(&physical, &BTreeSet::from(["contact"]))
                .unwrap_err()
                .contains("every scanner row")
        );
        let (_, mut firmware) = fixture();
        firmware.inputs.last_mut().unwrap().debug_key = Some("FUNCTION3".into());
        assert!(
            firmware
                .validate(&physical, &BTreeSet::from(["contact"]))
                .unwrap_err()
                .contains("undeclared debug key")
        );
    }

    #[test]
    fn electronic_controls_may_be_unbound_but_mechanical_controls_have_no_actions() {
        let (mut physical, _) = fixture();
        physical.controls.get_mut("menu_ok").unwrap().behavior = None;
        physical.validate().unwrap();
        assert!(
            physical
                .action(&fixture().1, "menu_ok", ControlOperation::Press)
                .is_err()
        );
        let control = physical.controls.get_mut("movie").unwrap();
        control.mechanical = true;
        assert!(physical.validate().is_err());
    }

    #[test]
    fn roles_are_unique_and_documents_refuse_unknown_fields() {
        let (mut physical, _) = fixture();
        physical.controls.get_mut("menu_ok").unwrap().role = Some(ControlRole::CommandControl);
        physical.controls.get_mut("command_control").unwrap().role =
            Some(ControlRole::CommandControl);
        assert!(
            physical
                .validate()
                .unwrap_err()
                .contains("assigned more than once")
        );
        let mut value = serde_json::to_value(fixture().0).unwrap();
        value["controls"]["menu_ok"]["alias"] = json!("ok");
        assert!(serde_json::from_value::<PhysicalControls>(value).is_err());
        assert!(
            serde_json::from_value::<InputBinding>(
                json!({"kind":"debug_key","name":"EXE_M","native":"KEY_OK"})
            )
            .is_err()
        );
    }
}
