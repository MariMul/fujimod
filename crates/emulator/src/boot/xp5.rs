//! XP5 device topology resolved from the selected SoC profile.
//!
//! The profile owns device addresses and options. Run inputs own the external
//! values supplied to those devices; neither the caller nor a board runner
//! needs a second copy of the register-window map.

use crate::board::{
    InterruptMap, aprofile,
    bus::{Bus, Mode},
    factory,
};
use fujimod_catalog::{
    profiles::RegisterWindow,
    reference::{ResolvedSelection, SocBlockKind},
};
use fujimod_emulator_devices::{
    graph::{DeviceGraph, DeviceWindow},
    parts::{
        controls::{Controls, Source},
        pins::DriveInput,
        resistor_keys::{Options as ResistorKeysOptions, ResistorKeys},
    },
    xp5::board as xp5_board,
};
use serde_json::Value;
use std::time::Duration;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn attachment_kind(
    selection: &ResolvedSelection,
    window: &RegisterWindow,
) -> Result<&'static str, String> {
    let attachment = window.attachment.as_deref().ok_or_else(|| {
        format!(
            "XP5 window {} has no device kind or attachment",
            window.name
        )
    })?;
    let block = selection
        .soc
        .blocks
        .iter()
        .find(|block| block.id == attachment)
        .ok_or_else(|| {
            format!(
                "XP5 window {} attaches to missing SoC block {attachment}",
                window.name
            )
        })?;
    match block.kind {
        SocBlockKind::Timer => Ok("timer"),
        SocBlockKind::SerialController => Ok("spi-controller"),
        kind => Err(format!(
            "XP5 window {} cannot attach SoC block {} of kind {:?}",
            window.name, block.id, kind
        )),
    }
}

/// Convert one catalog window into the device graph's checked register window.
fn device_window(
    selection: &ResolvedSelection,
    window: &RegisterWindow,
) -> Result<DeviceWindow, String> {
    let kind = window
        .kind
        .as_deref()
        .map(str::to_owned)
        .map(Ok)
        .unwrap_or_else(|| attachment_kind(selection, window).map(str::to_owned))?;
    let options = window
        .options
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| format!("XP5 window {} options: {error}", window.name))?
        .map(|value| {
            toml::Value::try_from(value)
                .map_err(|error| format!("XP5 window {} options: {error}", window.name))
        })
        .transpose()?
        .and_then(|value| value.as_table().cloned())
        .unwrap_or_default();
    Ok(DeviceWindow {
        base: window.base,
        size: window.size,
        kind,
        options,
    })
}

/// Resolve the selected XP5 profile's explicit device windows.
pub fn windows(
    selection: &ResolvedSelection,
    profile_id: &str,
) -> Result<Vec<DeviceWindow>, String> {
    let profile = selection
        .soc
        .hardware
        .as_ref()
        .and_then(|hardware| hardware.profiles.get(profile_id))
        .ok_or_else(|| {
            format!(
                "SoC {} has no hardware profile {profile_id}",
                selection.soc.id
            )
        })?;
    let windows = profile
        .windows
        .as_ref()
        .ok_or_else(|| format!("SoC profile {profile_id} declares no register windows"))?;
    windows
        .iter()
        .map(|window| device_window(selection, window))
        .collect()
}

/// An XP5 device bus assembled from the selected SoC profile.
pub struct DeviceAssembly {
    pub bus: Bus,
    pub inputs: super::super::session::xp5::State,
    pub nodes: BTreeMap<String, usize>,
}

/// Construct XP5 devices and retain their shared external input state.
///
/// The resistor-key controller is supplied by the caller so its electrical
/// layout and timing remain application inputs rather than emulator policy.
pub fn assemble_devices(
    windows: Vec<DeviceWindow>,
    controls_options: ResistorKeysOptions,
    mut resources: xp5_board::Resources,
) -> Result<DeviceAssembly, String> {
    let input_driver: Arc<dyn DriveInput> = resources.pins.clone();
    let controls = Arc::new(ResistorKeys::new(input_driver, controls_options)?);
    controls.start(Duration::ZERO);
    let control_interface: Arc<dyn Controls> = controls.clone();
    resources.controls = control_interface;
    let mut graph = DeviceGraph::new();
    let mut unknown = |window: &fujimod_emulator_devices::graph::DeviceWindow| {
        Err(format!(
            "unknown XP5 device {} at {:#x}",
            window.kind, window.base
        ))
    };
    xp5_board::register_all(&mut graph, &windows, &mut resources, &mut unknown)?;
    if !resources.displays.is_empty() {
        return Err("XP5 profile has unused display configurations".into());
    }
    let control_source = graph.attach(Box::new(Source::new(controls.clone())));
    let gpio_devices = windows
        .iter()
        .filter(|window| matches!(window.kind.as_str(), "gpio" | "gpio-port3"))
        .map(|window| graph.at(window.base))
        .collect::<Result<Vec<_>, _>>()?;
    let mut nodes = BTreeMap::new();
    for window in &windows {
        match graph.at(window.base) {
            Ok(node) => {
                nodes.insert(format!("{:#x}", window.base), node);
            }
            // Clock banks are a sparse envelope: its base is not itself a
            // register device, and holes in the envelope must remain unmapped.
            Err(_) if window.kind == "clock-banks" => {}
            Err(error) => return Err(error),
        }
    }
    // Expose the actual mapped nodes inside composite envelopes for callers
    // that need to inspect a specific clock bank.
    for node in &graph.nodes {
        for mapping in &node.mappings {
            nodes.insert(format!("{:#x}", mapping.base), graph.at(mapping.base)?);
        }
    }
    for &device in &gpio_devices {
        let mut causes = graph.nodes[device].native_dependencies.clone();
        causes.push(control_source);
        graph.depend_on(device, causes)?;
    }
    let bus = Bus::from_graph(Mode::Strict, graph)?;
    Ok(DeviceAssembly {
        bus,
        inputs: super::super::session::xp5::State::new(
            resources.pins,
            controls,
            gpio_devices,
            control_source,
        ),
        nodes,
    })
}

/// One caller-supplied initial memory region for an XP5 run.
pub struct MemoryImage {
    pub address: u64,
    pub bytes: Vec<u8>,
}

/// Complete caller-owned inputs for opening an XP5 board on the shared factory.
///
/// Firmware bytes are supplied as memory images and storage backing. This API
/// does not compare them with a bundled firmware identity or apply patches.
pub struct OpenOptions {
    pub definition: Value,
    pub processors: factory::Graph,
    pub windows: Vec<DeviceWindow>,
    pub controls: ResistorKeysOptions,
    pub resources: xp5_board::Resources,
    pub engine: factory::Engine,
    pub unicorn: Option<PathBuf>,
    pub interrupts: Option<InterruptMap>,
    pub aprofile: Vec<(usize, aprofile::Config)>,
    pub images: Vec<MemoryImage>,
    pub scene: Option<Value>,
}

fn validate_images(graph: &factory::Graph, images: &[MemoryImage]) -> Result<(), String> {
    let mut ranges = Vec::with_capacity(images.len());
    let mut total = 0usize;
    for image in images {
        if image.bytes.is_empty() {
            return Err("XP5 memory image is empty".into());
        }
        total = total
            .checked_add(image.bytes.len())
            .ok_or("XP5 memory image extent overflows")?;
        if total > 512 << 20 {
            return Err("XP5 initial memory images exceed 512 MiB".into());
        }
        let end = image
            .address
            .checked_add(image.bytes.len() as u64)
            .ok_or("XP5 memory image address overflows")?;
        if !graph.ram.iter().any(|ram| {
            let ram_end = ram.base.checked_add(ram.size as u64);
            image.address >= ram.base && ram_end.is_some_and(|ram_end| end <= ram_end)
        }) {
            return Err("XP5 memory image escapes declared physical RAM".into());
        }
        ranges.push((image.address, end));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("XP5 initial memory images overlap".into());
    }
    Ok(())
}

/// Open a caller-described XP5 board using FujiMod's standard bus, CPU factory
/// and session. Firmware selection and byte identity policy belong to caller.
pub fn open(options: OpenOptions) -> Result<super::super::session::Session, String> {
    validate_images(&options.processors, &options.images)?;
    options.processors.validate()?;
    let assembly = assemble_devices(options.windows, options.controls, options.resources)?;
    let mut board = options.processors.build(
        factory::Options {
            engine: options.engine,
            unicorn: options.unicorn,
            translation_cache_bytes: 16 << 20,
        },
        assembly.bus,
        options.interrupts,
    )?;
    for (core, profile) in options.aprofile {
        board.configure_aprofile(core, profile)?;
    }
    for image in options.images {
        board.write_physical(image.address, &image.bytes)?;
    }
    if let Some(scene) = options.scene.as_ref() {
        board.set_scene(scene)?;
    }
    let mut session = super::super::session::Session {
        definition: options.definition,
        board,
        controls: None,
        contact_driver: None,
        resources: Default::default(),
        xp5: None,
        nodes: assembly.nodes,
        contacts: BTreeMap::new(),
        recording_frames: Default::default(),
    };
    session.xp5 = Some(assembly.inputs);
    Ok(session)
}
