//! Standalone boot from catalog facts and explicitly selected user files.
//!
//! Preparation runs before execution. Host readiness checks only observe RAM;
//! first-start acceptance drives the ordinary ADC input on the guest timeline.
use crate::{
    board::{
        bus::{Bus, Mode},
        factory,
    },
    session::{Session, preparation},
};
use fujimod_catalog::emulator::Content;
use fujimod_emulator_cpu::arm::Coprocessor;
use fujimod_emulator_devices::exr2::board::{MemoryChoices, Plan, RefuseAdapters, Resources};
use serde::{Deserialize, Serialize};
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
    pub body: String,
    pub firmware: String,
    pub update: PathBuf,
    pub boot_rom: PathBuf,
    pub unicorn: PathBuf,
    /// Committed NAND image exported by the emulator, or a complete raw medium.
    pub nand: Option<PathBuf>,
    /// Sixteen-byte checksum-protected MCU persistent settings packet.
    pub settings: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rom {
    content: Content,
    address: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Generated {
    name: String,
    pattern: Vec<u8>,
    size: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reset {
    core: usize,
    register: Coprocessor,
    set_bits: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mcu {
    node: String,
    route: usize,
    first_register: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Readiness {
    category_one: u64,
    category_two: u64,
    input_pending: Option<u64>,
    override_address: u64,
    excluded_mask: u64,
    operation_alert_timer: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FirstStart {
    adc: String,
    channel: usize,
    calibration_pointer: u64,
    lower_offset: u64,
    upper_offset: u64,
    hold_ticks: u64,
    settle_ticks: u64,
    max_presses: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    #[serde(rename = "$schema")]
    _schema_uri: String,
    schema: u32,
    body: String,
    firmware: String,
    update: Content,
    flash_offset: usize,
    boot_rom: Rom,
    quantum: usize,
    execution: Value,
    devices: Value,
    scene: Value,
    generated_assets: Vec<Generated>,
    reset_coprocessors: Vec<Reset>,
    mcu: Mcu,
    observation: Observation,
    first_start: FirstStart,
}
fn definition(body: &str, firmware: &str) -> Result<Definition, String> {
    let raw = match (body, firmware) {
        ("x70", "x70-110") => include_str!("../../../catalog/emulator/runtimes/x70-110.json"),
        _ => return Err(format!("unsupported body/firmware: {body}/{firmware}")),
    };
    let spec: Definition =
        serde_json::from_str(raw).map_err(|e| format!("runtime catalog: {e}"))?;
    if spec.schema != 1 || spec.body != body || spec.firmware != firmware || spec.quantum == 0 {
        return Err("runtime catalog identity or quantum differs".into());
    }
    Ok(spec)
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
    let mut spec = definition(&options.body, &options.firmware)?;
    let update = checked(&options.update, &spec.update)?;
    let rom = checked(&options.boot_rom, &spec.boot_rom.content)?;
    if spec.flash_offset >= update.len() {
        return Err("update flash offset exceeds input".into());
    }
    let flash: Vec<u8> = update[spec.flash_offset..].iter().map(|b| !b).collect();
    let mut assets = BTreeMap::from([("nand_base".into(), flash)]);
    for input in &spec.generated_assets {
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
    if let Some(path) = &options.settings {
        settings(&mut spec.devices, &spec.mcu, &read(path, 16)?)?;
    }
    if let Some(path) = &options.nand {
        let bytes = read(path, 256 << 20)?;
        let nand = &mut spec.scene["nands"]["nand"];
        if bytes.len() as u64
            == nand["options"]["bytes"]
                .as_u64()
                .ok_or("NAND geometry is absent")?
        {
            if !bytes.starts_with(&assets["nand_base"]) {
                return Err("raw NAND firmware prefix differs from the selected update".into());
            }
            nand["overlays"] = json!([[0, "nand_image"]]);
        } else {
            *nand = json!({"kind":"committed","options":nand["options"],"base":"nand_base","image":"nand_image"});
        }
        assets.insert("nand_image".into(), bytes);
    }
    let graph: factory::Graph =
        serde_json::from_value(spec.execution.clone()).map_err(|e| e.to_string())?;
    graph.validate()?;
    let plan = Plan::parse(&serde_json::to_vec(&spec.devices).map_err(|e| e.to_string())?)?;
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
    resources.inputs = serde_json::from_value::<preparation::Spec>(spec.scene.clone())
        .map_err(|e| e.to_string())?
        .build(&assets)?;
    let assembly = plan.build(resources, 0, &mut RefuseAdapters)?;
    let bus = Bus::from_graph(Mode::Strict, assembly.graph)?;
    let mut board = graph.build(
        factory::Options {
            engine: factory::Engine::Unicorn,
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
    board.write_physical(spec.boot_rom.address, &rom)?;
    for reset in spec.reset_coprocessors {
        let cpu = board.cpu(reset.core)?;
        let old = cpu.coprocessor(reset.register, None)?;
        cpu.coprocessor(reset.register, Some(old | reset.set_bits))?;
    }
    Ok(Session {
        definition: json!({"body":options.body,"firmware":options.firmware,"quantum":spec.quantum,"execution":spec.execution,"devices":spec.devices,"scene":spec.scene,
            "inputs":{"update":spec.update,"boot_rom":spec.boot_rom.content,"nand":options.nand.is_some(),"settings":options.settings.is_some()},"substitutions":[]}),
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
fn observe(session: &Session, spec: &Definition, presses: usize) -> Result<Report, String> {
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
    spec: &Definition,
    end: u64,
    until: Option<u64>,
) -> Result<(), String> {
    let now: u64 = session.board.instruction_counts().iter().sum();
    if now >= end {
        return Err("instruction limit reached during first-start input".into());
    }
    session.board.run_with_limits(
        end - now,
        spec.quantum,
        Some(now.saturating_add(1_000_000).min(end)),
        until,
    )
}
fn advance_ticks(
    session: &mut Session,
    spec: &Definition,
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
fn accept_setup(session: &mut Session, spec: &Definition, end: u64) -> Result<(), String> {
    let s = &spec.first_start;
    let base = u64::from(word(session, s.calibration_pointer)?);
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
    let body = session.definition["body"]
        .as_str()
        .ok_or("session has no boot body")?;
    let firmware = session.definition["firmware"]
        .as_str()
        .ok_or("session has no boot firmware")?;
    let spec = definition(body, firmware)?;
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
            && presses < spec.first_start.max_presses
        {
            presses += 1;
            accept_setup(session, &spec, end)
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
    let spec = definition(
        session.definition["body"]
            .as_str()
            .ok_or("session has no boot body")?,
        session.definition["firmware"]
            .as_str()
            .ok_or("session has no boot firmware")?,
    )?;
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
