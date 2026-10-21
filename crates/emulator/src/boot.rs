//! Standalone boot from catalog facts and explicitly selected user files.
//!
//! Preparation runs before execution. Host readiness checks only observe RAM;
//! first-start acceptance drives the ordinary ADC input on the guest timeline.
mod graph;
mod x70;

use crate::{
    board::{
        bus::{Bus, Mode},
        factory,
    },
    session::{Session, preparation},
};
use fujimod_catalog::{
    configuration::{ResolvedConfiguration, RunEngine, RunExecutionMode, RunStart},
    emulator::Content,
    facts::{CalibrationInterval, ConfigurationLocation, ControlInput},
    reference::PartModel,
};
use fujimod_emulator_cpu::arm::Coprocessor;
use fujimod_emulator_devices::exr2::board::{MemoryChoices, Plan, RefuseAdapters, Resources};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// Every binary input location is supplied by the embedding program.
#[derive(Clone, Debug)]
pub struct Options {
    pub configuration_id: String,
    pub overlays: Vec<String>,
    pub update: PathBuf,
    pub boot_rom: PathBuf,
    pub unicorn: PathBuf,
    /// Committed NAND image, or a complete main-area image without OOB/spare.
    pub nand: Option<PathBuf>,
    /// Sixteen-byte checksum-protected MCU persistent settings packet.
    pub settings: Option<PathBuf>,
}

#[derive(Clone)]
struct Rom {
    content: Content,
    address: u64,
}
#[derive(Clone)]
struct Generated {
    name: String,
    pattern: Vec<u8>,
    size: usize,
}
#[derive(Clone)]
struct Reset {
    core: usize,
    register: Coprocessor,
    set_bits: u64,
}
#[derive(Clone)]
struct Mcu {
    node: String,
    route: usize,
    first_register: u8,
}
#[derive(Clone)]
struct Observation {
    ui_mode: u64,
    system_mode: u64,
    menu_active: u64,
    pending_mode: u64,
    menu_node: u64,
    rec_mode: u32,
    setup_mode: u32,
    scanout: String,
    instance: usize,
    readiness: Readiness,
}
#[derive(Clone)]
struct Readiness {
    category_one: u64,
    category_two: u64,
    input_pending: Option<u64>,
    override_address: u64,
    excluded_mask: u64,
    operation_alert_timer: Option<u64>,
}
#[derive(Clone)]
struct FirstStart {
    key_id: String,
    adc: String,
    channel: usize,
    calibration_pointer: u64,
    calibration_is_pointer: bool,
    lower_offset: u64,
    upper_offset: u64,
    hold_ticks: u64,
    settle_ticks: u64,
    max_presses: usize,
}
/// A fully resolved run assembled before loading caller files or creating CPUs.
pub struct Prepared {
    resolved: ResolvedConfiguration,
    execution: factory::Graph,
    execution_value: Value,
    plan: Plan,
    devices_value: Value,
    scene: Value,
    quantum: usize,
    update: Content,
    flash_offset: usize,
    boot_rom: Rom,
    generated_assets: Vec<Generated>,
    reset_coprocessors: Vec<Reset>,
    mcu: Mcu,
    observation: Observation,
    first_start: Option<FirstStart>,
}

fn content(size: u64, sha256: &str) -> Content {
    Content {
        size,
        sha256: sha256.to_owned(),
    }
}

fn first_start(resolved: &ResolvedConfiguration) -> Result<Option<FirstStart>, String> {
    let configuration = &resolved.configuration;
    let Some(policy) = &configuration.inputs.first_start else {
        return Ok(None);
    };
    let facts = resolved
        .reference
        .firmware
        .generated_facts
        .as_ref()
        .ok_or("first-start input requires generated firmware control facts")?;
    let controls = facts
        .controls
        .as_ref()
        .ok_or("first-start input requires firmware control predicates")?;
    let mut matches = controls.controls.values().filter_map(|control| {
        control
            .keys
            .contains(&policy.key_id)
            .then(|| control.input.as_ref()?.get(&policy.key_id))
            .flatten()
    });
    let predicates = matches.next().ok_or_else(|| {
        format!(
            "firmware has no control predicate for key {}",
            policy.key_id
        )
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "firmware key {} has ambiguous physical control predicates",
            policy.key_id
        ));
    }
    let mut inputs = predicates.iter().filter_map(|predicate| match predicate {
        ControlInput::Adc {
            connection_id,
            intervals,
            ..
        } => Some((connection_id, intervals.as_slice())),
        _ => None,
    });
    let (connection_id, intervals) = inputs
        .next()
        .ok_or_else(|| format!("firmware key {} has no ADC predicate", policy.key_id))?;
    if inputs.next().is_some() || intervals.len() != 1 {
        return Err(format!(
            "firmware key {} has unsupported multiple ADC calibration intervals",
            policy.key_id
        ));
    }
    let connection = resolved
        .reference
        .body
        .connections
        .iter()
        .find(|connection| connection.id == *connection_id)
        .ok_or("first-start ADC connection is absent from the body")?;
    let channel = connection
        .signals
        .iter()
        .filter_map(|signal| match signal {
            fujimod_catalog::reference::ConnectionSignal::Adc { channel, .. } => {
                Some(usize::from(*channel))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if channel.len() != 1 || configuration.inputs.adc.block_id != "adc" {
        return Err("first-start ADC target is ambiguous or unsupported".into());
    }
    let cfgdata = facts
        .cfgdata_location
        .ok_or("first-start calibration has no firmware configuration-block location")?;
    let (calibration_pointer, calibration_is_pointer) = match cfgdata {
        ConfigurationLocation::Pointer { address } => (address, true),
        ConfigurationLocation::Direct { address } => (address, false),
    };
    let CalibrationInterval { lower, upper } = intervals[0];
    Ok(Some(FirstStart {
        key_id: policy.key_id.clone(),
        adc: configuration.inputs.adc.block_id.clone(),
        channel: channel[0],
        calibration_pointer,
        calibration_is_pointer,
        lower_offset: u64::from(lower),
        upper_offset: u64::from(upper),
        hold_ticks: policy.hold_ticks,
        settle_ticks: policy.settle_ticks,
        max_presses: usize::from(policy.max_presses),
    }))
}

fn prepared(resolved: ResolvedConfiguration) -> Result<Prepared, String> {
    let configuration = &resolved.configuration;
    configuration.validate(&configuration.id)?;
    configuration.validate_selection(&resolved.reference)?;
    if configuration.start != RunStart::Rom {
        return Err("native boot only supports ROM-start configurations".into());
    }
    let firmware = &resolved.reference.firmware;
    let update_identity = firmware
        .update
        .as_ref()
        .ok_or("selected firmware has no update content identity")?;
    let rom_identity = firmware
        .boot_rom
        .as_ref()
        .ok_or("selected firmware has no boot-ROM identity")?;
    let installation = firmware
        .installation
        .as_ref()
        .ok_or("selected firmware has no installation layout")?;
    let observation = firmware
        .generated_facts
        .as_ref()
        .and_then(|facts| facts.observation.as_ref())
        .ok_or("selected firmware has no observation facts")?;
    let quantum = configuration.execution.quantum;
    let execution = graph::assemble(
        &resolved.reference,
        configuration.execution.instructions_per_tick,
    )?;
    let execution_value = graph::value(
        &resolved.reference,
        &execution,
        configuration.execution.instructions_per_tick,
    );
    let (plan, devices_value, scene) = x70::assemble(&resolved.reference, configuration)?;
    plan.validate_reset_outputs(
        &execution
            .processors
            .iter()
            .map(|processor| processor.released)
            .collect::<Vec<_>>(),
    )?;
    plan.validate_memory_outputs(
        &execution
            .decoders
            .iter()
            .map(|decoder| MemoryChoices {
                initial: decoder.initial,
                choices: decoder.choices.len(),
            })
            .collect::<Vec<_>>(),
    )?;
    let generated_assets = configuration
        .scene
        .generated_assets
        .iter()
        .map(|asset| {
            Ok(Generated {
                name: asset.id.clone(),
                pattern: asset.pattern.clone(),
                size: usize::try_from(asset.size)
                    .map_err(|_| format!("scene asset {} exceeds host address space", asset.id))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut core_indexes = BTreeMap::new();
    for (index, processor) in resolved.reference.soc.processors.iter().enumerate() {
        core_indexes.insert(processor.id.as_str(), index);
    }
    let reset_coprocessors = resolved
        .reference
        .soc
        .reset
        .coprocessor_writes
        .iter()
        .map(|write| {
            Ok(Reset {
                core: *core_indexes
                    .get(write.core.as_str())
                    .ok_or_else(|| format!("reset write targets absent core {}", write.core))?,
                register: Coprocessor {
                    cp: u32::from(write.cp),
                    opc1: u32::from(write.opc1),
                    crn: u32::from(write.crn),
                    crm: u32::from(write.crm),
                    opc2: u32::from(write.opc2),
                    secure: u32::from(write.secure),
                    wide: write.wide,
                },
                set_bits: write.set_bits,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mcu_attachment = resolved
        .reference
        .body
        .microcontroller
        .as_ref()
        .ok_or("X70 run has no attached MCU")?;
    let mcu_part = resolved
        .reference
        .parts
        .iter()
        .find(|part| part.id == mcu_attachment.part_id)
        .ok_or("X70 MCU part is unresolved")?;
    let PartModel::Microcontroller {
        packet_registers, ..
    } = &mcu_part.model
    else {
        return Err("X70 MCU attachment is not a microcontroller part".into());
    };
    let node = x70::node_for_block(&mcu_attachment.controller_id)?;
    let mcu = Mcu {
        node,
        route: usize::from(mcu_attachment.channel),
        first_register: u8::try_from(packet_registers.first)
            .map_err(|_| "X70 MCU packet register exceeds one byte")?,
    };
    let readiness = &observation.readiness;
    let observation = Observation {
        ui_mode: observation.ui_mode,
        system_mode: observation.system_mode,
        menu_active: observation.menu_active,
        pending_mode: observation.pending_mode,
        menu_node: observation.menu_node,
        rec_mode: observation.rec_mode,
        setup_mode: observation.setup_mode,
        scanout: "scanout".into(),
        instance: 0,
        readiness: Readiness {
            category_one: readiness.category_one,
            category_two: readiness.category_two,
            input_pending: Some(readiness.input_pending),
            override_address: readiness.override_address,
            excluded_mask: u64::from(readiness.excluded_mask),
            operation_alert_timer: Some(readiness.operation_alert_timer),
        },
    };
    let first_start = first_start(&resolved)?;
    let flash_offset = usize::try_from(installation.flash_offset)
        .map_err(|_| "firmware flash offset exceeds host address space")?;
    let update = content(update_identity.size, &update_identity.sha256);
    let boot_rom = Rom {
        content: content(rom_identity.content.size, &rom_identity.content.sha256),
        address: rom_identity.address,
    };
    Ok(Prepared {
        resolved,
        execution,
        execution_value,
        plan,
        devices_value,
        scene,
        quantum,
        update,
        flash_offset,
        boot_rom,
        generated_assets,
        reset_coprocessors,
        mcu,
        observation,
        first_start,
    })
}

/// Resolve and compile a named configuration before any input file or CPU is used.
pub fn prepare(configuration_id: &str, overlays: &[String]) -> Result<Prepared, String> {
    prepared(ResolvedConfiguration::with_overlays(
        configuration_id,
        overlays,
    )?)
}

impl Prepared {
    /// Produce the stable JSON projection used by the native Python adapter.
    pub fn to_json(&self) -> Value {
        json!({
            "schema": 1,
            "configuration_id": self.resolved.configuration.id,
            "body_id": self.resolved.configuration.body_id,
            "firmware_id": self.resolved.configuration.firmware_id,
            "overlays": self.resolved.configuration.overlays,
            "resolved": self.resolved,
            "quantum": self.quantum,
            "execution": self.execution_value,
            "devices": self.devices_value,
            "scene": self.scene,
            "inputs": {
                "update": self.update,
                "boot_rom": {"content":self.boot_rom.content,"address":self.boot_rom.address},
                "flash_offset": self.flash_offset,
                "mcu": {"node":self.mcu.node,"route":self.mcu.route,"first_register":self.mcu.first_register},
                "reset_coprocessors": self.reset_coprocessors.iter().map(|reset| json!({
                    "core":reset.core,
                    "register": {
                        "cp": reset.register.cp,
                        "wide": reset.register.wide,
                        "secure": reset.register.secure,
                        "crn": reset.register.crn,
                        "crm": reset.register.crm,
                        "opc1": reset.register.opc1,
                        "opc2": reset.register.opc2,
                    },
                    "set_bits":reset.set_bits
                })).collect::<Vec<_>>(),
                "observation": {
                    "ui_mode":self.observation.ui_mode,
                    "system_mode":self.observation.system_mode,
                    "menu_active":self.observation.menu_active,
                    "pending_mode":self.observation.pending_mode,
                    "menu_node":self.observation.menu_node,
                    "rec_mode":self.observation.rec_mode,
                    "setup_mode":self.observation.setup_mode,
                    "scanout":self.observation.scanout,
                    "instance":self.observation.instance,
                    "readiness": {
                        "category_one":self.observation.readiness.category_one,
                        "category_two":self.observation.readiness.category_two,
                        "input_pending":self.observation.readiness.input_pending,
                        "override_address":self.observation.readiness.override_address,
                        "excluded_mask":self.observation.readiness.excluded_mask,
                        "operation_alert_timer":self.observation.readiness.operation_alert_timer,
                    }
                },
                "first_start": self.first_start.as_ref().map(|start| json!({
                    "key_id":start.key_id,
                    "adc":start.adc,"channel":start.channel,
                    "calibration_pointer":start.calibration_pointer,
                    "calibration_is_pointer":start.calibration_is_pointer,
                    "lower_offset":start.lower_offset,"upper_offset":start.upper_offset,
                    "hold_ticks":start.hold_ticks,"settle_ticks":start.settle_ticks,
                    "max_presses":start.max_presses,
                })),
                "generated_assets": self.generated_assets.iter().map(|asset| json!({
                    "name":asset.name,"pattern":asset.pattern,"size":asset.size,
                })).collect::<Vec<_>>(),
            }
        })
    }
}

/// Read a bounded regular file, rejecting size changes during preparation.
fn read(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(format!(
            "{} must be a regular file of at most {max_bytes} bytes",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != metadata.len() {
        return Err(format!("{} changed during preparation", path.display()));
    }
    Ok(bytes)
}
fn checked(path: &Path, content: &Content) -> Result<Vec<u8>, String> {
    let bytes = read(path, content.size)?;
    content
        .check(&bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes)
}
fn packet(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != 16
        || bytes[14] != 0xaa
        || bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b)) != 0
    {
        return Err("settings require a sixteen-byte MCU packet with marker 0xaa and an eight-bit zero checksum".into());
    }
    Ok(())
}
fn settings(plan: &mut Value, mcu: &Mcu, bytes: &[u8]) -> Result<(), String> {
    packet(bytes)?;
    let nodes = plan["nodes"]
        .as_array_mut()
        .ok_or("runtime nodes are absent")?;
    let node = nodes
        .iter_mut()
        .find(|n| n["name"] == mcu.node)
        .ok_or("MCU serial node is absent")?;
    let registers = node["model"]["routes"][mcu.route]["endpoint"]["options"]["registers"]
        .as_array_mut()
        .ok_or("MCU register endpoint is absent")?;
    for (offset, byte) in bytes.iter().enumerate() {
        let register = u64::from(mcu.first_register) + offset as u64;
        let row = registers
            .iter_mut()
            .find(|r| r[0].as_u64() == Some(register))
            .ok_or("MCU packet register is absent")?;
        row[1] = json!(byte);
    }
    Ok(())
}

/// Construct an unexecuted strict board, loading the real ROM at its reset map.
/// No directory search, downloaded image, captured default or firmware patch is used.
pub fn open(options: Options) -> Result<Session, String> {
    let prepared = prepare(&options.configuration_id, &options.overlays)?;
    let mut session_definition = prepared.to_json();
    session_definition["inputs"]["nand_selected"] = json!(options.nand.is_some());
    session_definition["inputs"]["settings_selected"] = json!(options.settings.is_some());
    let update = checked(&options.update, &prepared.update)?;
    let rom = checked(&options.boot_rom, &prepared.boot_rom.content)?;
    if prepared.flash_offset >= update.len() {
        return Err("update flash offset exceeds input".into());
    }
    let flash: Vec<u8> = update[prepared.flash_offset..].iter().map(|b| !b).collect();
    let mut assets = BTreeMap::from([("nand_base".into(), flash)]);
    for input in &prepared.generated_assets {
        if input.pattern.is_empty()
            || input.size > 64 << 20
            || !input.size.is_multiple_of(input.pattern.len())
        {
            return Err("generated scene input has an invalid extent".into());
        }
        if assets
            .insert(
                input.name.clone(),
                input.pattern.repeat(input.size / input.pattern.len()),
            )
            .is_some()
        {
            return Err("duplicate scene asset".into());
        }
    }
    let mut devices = prepared.devices_value.clone();
    let mut scene = prepared.scene.clone();
    if let Some(path) = &options.settings {
        settings(&mut devices, &prepared.mcu, &read(path, 16)?)?;
    }
    if let Some(path) = &options.nand {
        let bytes = read(path, 256 << 20)?;
        let nand = &mut scene["nands"]["nand"];
        if bytes.len() as u64
            == nand["options"]["bytes"]
                .as_u64()
                .ok_or("NAND geometry is absent")?
        {
            if !bytes.starts_with(&assets["nand_base"]) {
                return Err(
                    "NAND main-area firmware prefix differs from the selected update".into(),
                );
            }
            nand["overlays"] = json!([[0, "nand_image"]]);
        } else {
            *nand = json!({"kind":"committed","options":nand["options"],"base":"nand_base","image":"nand_image"});
        }
        assets.insert("nand_image".into(), bytes);
    }
    let graph = prepared.execution;
    graph.validate()?;
    let plan = if options.settings.is_some() {
        Plan::parse(&serde_json::to_vec(&devices).map_err(|e| e.to_string())?)?
    } else {
        prepared.plan
    };
    plan.validate_reset_outputs(
        &graph
            .processors
            .iter()
            .map(|p| p.released)
            .collect::<Vec<_>>(),
    )?;
    plan.validate_memory_outputs(
        &graph
            .decoders
            .iter()
            .map(|d| MemoryChoices {
                initial: d.initial,
                choices: d.choices.len(),
            })
            .collect::<Vec<_>>(),
    )?;
    let mut resources = Resources::default();
    resources.inputs = serde_json::from_value::<preparation::Spec>(scene.clone())
        .map_err(|e| e.to_string())?
        .build(&assets)?;
    let assembly = plan.build(resources, 0, &mut RefuseAdapters)?;
    let bus_mode = match prepared.resolved.configuration.execution.mode {
        RunExecutionMode::Strict => Mode::Strict,
    };
    let engine = match prepared.resolved.configuration.execution.engine {
        RunEngine::Unicorn => factory::Engine::Unicorn,
    };
    let bus = Bus::from_graph(bus_mode, assembly.graph)?;
    let mut board = graph.build(
        factory::Options {
            engine,
            unicorn: Some(options.unicorn),
            translation_cache_bytes: 16 << 20,
        },
        bus,
        None,
    )?;
    for controller in assembly.core_interrupts {
        board.configure_core_interrupts(
            controller.core,
            controller.device,
            Box::new(controller.inputs),
        )?;
    }
    board.write_physical(prepared.boot_rom.address, &rom)?;
    for reset in prepared.reset_coprocessors {
        let cpu = board.cpu(reset.core)?;
        let old = cpu.coprocessor(reset.register, None)?;
        cpu.coprocessor(reset.register, Some(old | reset.set_bits))?;
    }
    Ok(Session {
        definition: session_definition,
        board,
        controls: None,
        contact_driver: None,
        resources: assembly.resources,
        nodes: assembly.node_ids,
        contacts: assembly.contacts,
        recording_frames: Default::default(),
    })
}

/// A stopped, passive boot observation. `rec` also identifies the first-start
/// recording role (`system_mode == 3`) after its menus close.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub status: String,
    pub instructions: u64,
    pub virtual_ticks: u64,
    pub ui_mode: u32,
    pub system_mode: u32,
    pub menu_active: u32,
    pub menu_node: u32,
    pub pending_mode: u32,
    pub pending: bool,
    pub setup_presses: usize,
    pub failure: Option<String>,
}
fn word(session: &Session, address: u64) -> Result<u32, String> {
    let mut bytes = [0; 4];
    session.board.read_physical(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn observe(session: &Session, spec: &Prepared, presses: usize) -> Result<Report, String> {
    let o = &spec.observation;
    let r = &o.readiness;
    let wide = |address| -> Result<u64, String> {
        let mut bytes = [0; 8];
        session.board.read_physical(address, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    };
    let one = wide(r.category_one)?;
    let two = wide(r.category_two)?;
    let pending = one & (1 << 9) != 0
        || r.input_pending
            .map(|a| word(session, a))
            .transpose()?
            .unwrap_or(0)
            != 0
        || two & !r.excluded_mask != 0
        || word(session, r.override_address)? != 1 && two & r.excluded_mask != 0
        || r.operation_alert_timer
            .map(|a| word(session, a))
            .transpose()?
            .unwrap_or(0)
            != 0;
    Ok(Report {
        status: "running".into(),
        instructions: session.board.instruction_counts().iter().sum(),
        virtual_ticks: session.board.now(),
        ui_mode: word(session, o.ui_mode)?,
        system_mode: word(session, o.system_mode)?,
        menu_active: word(session, o.menu_active)?,
        menu_node: word(session, o.menu_node)?,
        pending_mode: word(session, o.pending_mode)?,
        pending,
        setup_presses: presses,
        failure: None,
    })
}
fn advance(
    session: &mut Session,
    spec: &Prepared,
    end: u64,
    until: Option<u64>,
) -> Result<(), String> {
    let now: u64 = session.board.instruction_counts().iter().sum();
    if now >= end {
        return Err("instruction limit reached during first-start input".into());
    }
    let tick = session.board.now();
    session.board.run_with_limits(
        end - now,
        spec.quantum,
        Some(now.saturating_add(1_000_000).min(end)),
        until,
    )?;
    if session.board.now() == tick && session.board.instruction_counts().iter().sum::<u64>() == now
    {
        return Err("board is quiescent without an input or device deadline".into());
    }
    Ok(())
}
fn advance_ticks(
    session: &mut Session,
    spec: &Prepared,
    end: u64,
    ticks: u64,
) -> Result<(), String> {
    let until = session
        .board
        .now()
        .checked_add(ticks)
        .ok_or("input deadline overflows")?;
    while session.board.now() < until {
        advance(session, spec, end, Some(until))?;
    }
    Ok(())
}

fn session_prepared(session: &Session) -> Result<Prepared, String> {
    let id = session.definition["configuration_id"]
        .as_str()
        .ok_or("session has no boot configuration")?;
    let overlays = session.definition["overlays"]
        .as_array()
        .ok_or("session has no selected overlays")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or("session overlay identity is not a string")
        })
        .collect::<Result<Vec<_>, _>>()?;
    prepare(id, &overlays)
}

fn accept_setup(
    session: &mut Session,
    spec: &Prepared,
    s: &FirstStart,
    end: u64,
) -> Result<(), String> {
    let base = if s.calibration_is_pointer {
        u64::from(word(session, s.calibration_pointer)?)
    } else {
        s.calibration_pointer
    };
    let half = |offset| -> Result<u16, String> {
        let mut bytes = [0; 2];
        session.board.read_physical(base + offset, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    };
    let lower = half(s.lower_offset)?;
    let upper = half(s.upper_offset)?;
    if lower >= upper || upper >= 1023 {
        return Err("MENU/OK calibration does not describe a pressed ADC interval".into());
    }
    let original = session.adc_samples(&s.adc)?;
    let mut pressed = original.clone();
    *pressed
        .get_mut(s.channel)
        .ok_or("MENU/OK ADC channel is absent")? = (lower + upper) / 2;
    session.set_adc_samples(&s.adc, &pressed)?;
    let held = advance_ticks(session, spec, end, s.hold_ticks);
    // Release even if the instruction budget or a modeled refusal stops the hold.
    session.set_adc_samples(&s.adc, &original)?;
    held?;
    advance_ticks(session, spec, end, s.settle_ticks)
}

/// Run a named runtime to REC or an explicit stopped frontier. First-start menus
/// are accepted through calibrated MENU/OK ADC presses, never RAM writes.
pub fn to_rec(session: &mut Session, instruction_limit: u64) -> Result<Report, String> {
    if instruction_limit == 0 {
        return Err("instruction limit must be positive".into());
    }
    let spec = session_prepared(session)?;
    let end = session
        .board
        .instruction_counts()
        .iter()
        .sum::<u64>()
        .checked_add(instruction_limit)
        .ok_or("instruction limit overflows")?;
    let mut presses = 0;
    loop {
        let mut report = observe(session, &spec, presses)?;
        if report.instructions >= end {
            report.status = "instruction-limit".into();
            return Ok(report);
        }
        let display = session.scanout_state(&spec.observation.scanout)?;
        let presented = display["instances"][spec.observation.instance]["graphics"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|layer| layer["channels"].as_array().into_iter().flatten())
            .any(|channel| {
                channel["enabled"] == true
                    && channel["configured"] == true
                    && channel["width"].as_u64().is_some_and(|n| n > 1)
                    && channel["height"].as_u64().is_some_and(|n| n > 1)
            });
        if report.ui_mode == spec.observation.rec_mode
            && matches!(report.system_mode, 1 | 3)
            && report.menu_active == 0
            && report.pending_mode == 0
            && !report.pending
            && presented
        {
            report.status = "rec".into();
            return Ok(report);
        }
        let result = if report.ui_mode == spec.observation.setup_mode
            && report.system_mode == 3
            && report.menu_active != 0
            && report.pending_mode == 0
            && !report.pending
            && presented
            && spec
                .first_start
                .as_ref()
                .is_some_and(|first_start| presses < first_start.max_presses)
        {
            presses += 1;
            accept_setup(
                session,
                &spec,
                spec.first_start.as_ref().expect("checked above"),
                end,
            )
        } else {
            advance(session, &spec, end, None)
        };
        if let Err(error) = result {
            let mut report = observe(session, &spec, presses)?;
            report.status = if report.instructions >= end {
                "instruction-limit"
            } else {
                "stopped"
            }
            .into();
            report.failure = Some(error);
            return Ok(report);
        }
    }
}

/// Capture composed native scanout without advancing guest time.
pub fn screenshot(session: &Session) -> Result<Vec<u8>, String> {
    let spec = session_prepared(session)?;
    let frame = session
        .scanout(
            &spec.observation.scanout,
            spec.observation.instance,
            true,
            false,
        )?
        .ok_or("LCD has no configured frame")?;
    let rgb = frame.rgb.ok_or("scanout did not return RGB")?;
    fujimod_recording::Frame::rgb(
        frame
            .width
            .try_into()
            .map_err(|_| "frame width exceeds u32")?,
        frame
            .height
            .try_into()
            .map_err(|_| "frame height exceeds u32")?,
        rgb,
    )?
    .png()
    .map(|p| p.to_vec())
}

#[cfg(test)]
mod tests;
