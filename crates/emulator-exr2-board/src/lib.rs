//! Checked, CPU-independent EXR II graph construction. Every native node
//! calls its real Rust constructor. A remaining language implementation is an
//! explicit adapter node supplied by the native frontend; absent adapters refuse.
mod construct;
mod resources;
pub mod serial;
mod spec;
#[cfg(test)]
mod tests;
use fujimod_emulator_devices::exr2::dma;
use fujimod_emulator_devices::{
    Device,
    graph::{DeviceGraph, Mapping, Node as GraphNode, validate_mappings},
};
pub use resources::checkpoint::ResourceSnapshot;
pub use resources::{CardPins, CoreInterrupts, Inputs, Resources, Wire};
pub use spec::{
    Adapter, BootEntry, Card, Model, Node, Plan, Sensor, SerialEndpoint, SerialRoute,
    SoftwareOutput, WireKind,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct Assembly {
    pub graph: DeviceGraph,
    pub node_ids: BTreeMap<String, usize>,
    pub core_interrupts: Vec<CoreInterrupts>,
    pub contacts: BTreeMap<String, fujimod_emulator_devices::exr2::sdhc::Contacts>,
    pub resources: Resources,
}

struct DmaMerge {
    remap: Vec<usize>,
}

fn consolidate_dma(
    graph: &mut DeviceGraph,
    node_ids: &mut BTreeMap<String, usize>,
    resources: &mut Resources,
) -> Result<DmaMerge, String> {
    const BASE: u64 = 0xfff02000;
    const END: u64 = BASE + 0x200;
    const GLOBAL: u64 = BASE + 0x300;
    let old_nodes = std::mem::take(&mut graph.nodes);
    let selected: BTreeSet<usize> = old_nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            node.mappings
                .iter()
                .any(|mapping| {
                    (mapping.base >= BASE && mapping.base < END)
                        || mapping.base == GLOBAL && mapping.size == 4
                })
                .then_some(id)
        })
        .collect();
    if selected.is_empty() {
        graph.nodes = old_nodes;
        return Ok(DmaMerge {
            remap: (0..graph.nodes.len()).collect(),
        });
    }

    let first = *selected.first().unwrap();
    let mut controller_mappings = Vec::new();
    for &id in &selected {
        let node = &old_nodes[id];
        controller_mappings.extend_from_slice(&node.mappings);
    }
    controller_mappings.sort_by_key(|mapping| mapping.base);
    validate_mappings(&controller_mappings, std::iter::empty())?;
    let mut old_nodes: Vec<Option<GraphNode>> = old_nodes.into_iter().map(Some).collect();
    let mut modes = Vec::with_capacity(selected.len());
    for id in selected.iter().copied() {
        let node = old_nodes[id].take().unwrap();
        modes.push(dma::controller::ChannelMode {
            mappings: node.mappings,
            device: node.device,
        });
    }
    let mut controller = Some(dma::controller::Controller::new(modes)?);
    let mut remap = vec![usize::MAX; old_nodes.len()];
    let mut rebuilt = Vec::with_capacity(old_nodes.len() - selected.len() + 1);
    for old_id in 0..old_nodes.len() {
        if old_id == first {
            let index = rebuilt.len();
            remap[first] = index;
            rebuilt.push(GraphNode {
                mappings: controller_mappings.clone(),
                device: Box::new(controller.take().unwrap()),
                native_dependencies: vec![],
            });
        } else if selected.contains(&old_id) {
            remap[old_id] = remap[first];
        } else {
            remap[old_id] = rebuilt.len();
            rebuilt.push(old_nodes[old_id].take().unwrap());
        }
    }
    graph.nodes = rebuilt;
    for id in node_ids.values_mut() {
        *id = remap[*id];
    }
    for users in resources
        .users
        .values_mut()
        .chain(resources.image_users.values_mut())
    {
        let mapped = users.iter().map(|id| remap[*id]).collect();
        *users = mapped;
    }
    Ok(DmaMerge { remap })
}
/// Execution-side memory decoder ports available to this device graph.
pub struct MemoryChoices {
    pub initial: usize,
    pub choices: usize,
}
pub trait AdapterFactory {
    fn construct(
        &mut self,
        name: &str,
        spec: &Adapter,
        windows: &[Mapping],
        resources: &Resources,
        nodes: &BTreeMap<String, usize>,
    ) -> Result<Box<dyn Device>, String>;
}
pub struct RefuseAdapters;
impl AdapterFactory for RefuseAdapters {
    fn construct(
        &mut self,
        name: &str,
        spec: &Adapter,
        _: &[Mapping],
        _: &Resources,
        _: &BTreeMap<String, usize>,
    ) -> Result<Box<dyn Device>, String> {
        Err(format!(
            "{name} requires unavailable adapter {}",
            spec.implementation
        ))
    }
}
pub fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
impl Plan {
    pub fn validate_reset_outputs(&self, released: &[bool]) -> Result<(), String> {
        for node in self
            .nodes
            .iter()
            .filter(|n| n.boot_entries.contains(&self.boot_entry))
        {
            if let Model::CoreRelease { options, core, .. } = &node.model {
                if released.get(*core) != Some(&options.released) {
                    return Err("core release reset state disagrees with execution graph".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_memory_outputs(&self, decoders: &[MemoryChoices]) -> Result<(), String> {
        for node in self
            .nodes
            .iter()
            .filter(|n| n.boot_entries.contains(&self.boot_entry))
        {
            if let Model::Block98000 { options, wiring } = &node.model {
                let decoder = decoders
                    .get(wiring.decoder)
                    .ok_or("boot control has no memory decoder")?;
                if wiring
                    .selections
                    .iter()
                    .any(|s| s.choice >= decoder.choices)
                    || !wiring
                        .selections
                        .iter()
                        .any(|s| s.word == options.selection_reset && s.choice == decoder.initial)
                {
                    return Err(
                        "boot control selections disagree with execution memory decoder".into(),
                    );
                }
            }
        }
        Ok(())
    }
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        let plan: Self = serde_json::from_slice(json).map_err(|e| format!("EXR II graph: {e}"))?;
        plan.validate()?;
        Ok(plan)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.nodes.is_empty() {
            return Err("unsupported or empty EXR II graph".into());
        }
        if self.wires.keys().any(|name| !identifier(name)) {
            return Err("invalid EXR II wire name".into());
        }
        let mut names = BTreeMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if !identifier(&node.name)
                || names.insert(node.name.as_str(), index).is_some()
                || node.boot_entries.is_empty()
                || node.boot_entries.iter().collect::<BTreeSet<_>>().len()
                    != node.boot_entries.len()
            {
                return Err("invalid or duplicate EXR II node name/boot entries".into());
            }
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node
                .windows
                .iter()
                .any(|m| m.base.checked_add(m.size).is_none_or(|end| end > 1 << 32))
            {
                return Err(format!("{} has a non-ARM32 device window", node.name));
            }
            validate_mappings(&node.windows, std::iter::empty())?;
            let mut dependencies = BTreeSet::new();
            for dependency in &node.dependencies {
                if dependency == &node.name || !dependencies.insert(dependency) {
                    return Err("duplicate or self native dependency".into());
                }
            }
            for (name, constructed) in node
                .dependencies
                .iter()
                .map(|n| (n.as_str(), false))
                .chain(node.model.node_references().into_iter().map(|n| (n, true)))
            {
                let target = *names
                    .get(name)
                    .ok_or_else(|| format!("{} references absent {name}", node.name))?;
                if constructed && target >= index {
                    return Err(format!("{} is constructed before {name}", node.name));
                }
                if node
                    .boot_entries
                    .iter()
                    .any(|entry| !self.nodes[target].boot_entries.contains(entry))
                {
                    return Err(format!(
                        "{} depends on {name} outside its boot entries",
                        node.name
                    ));
                }
            }
        }
        for entry in [BootEntry::Rom, BootEntry::Loader] {
            let mut mappings: Vec<Mapping> = vec![];
            for node in self
                .nodes
                .iter()
                .filter(|n| n.boot_entries.contains(&entry))
            {
                validate_mappings(&node.windows, mappings.iter())?;
                mappings.extend_from_slice(&node.windows);
            }
        }
        if !self
            .nodes
            .iter()
            .any(|n| n.boot_entries.contains(&self.boot_entry))
        {
            return Err("EXR II boot entry selects no nodes".into());
        }
        Ok(())
    }
    pub fn build(
        self,
        mut resources: Resources,
        start_tick: u64,
        adapter: &mut dyn AdapterFactory,
    ) -> Result<Assembly, String> {
        self.validate()?;
        if !resources.wires.is_empty()
            || !resources.users.is_empty()
            || !resources.image_users.is_empty()
            || !resources.gpio.is_empty()
            || !resources.pulses.is_empty()
            || !resources.controllers.is_empty()
            || !resources.encoders.is_empty()
            || !resources.hdmi_sinks.is_empty()
            || !resources.clocks.is_empty()
            || !resources.software_sources.is_empty()
        {
            return Err("EXR II resources must be fresh and explicitly named".into());
        }
        for (name, kind) in self.wires {
            use spec::WireKind;
            resources.wires.insert(
                name,
                match kind {
                    WireKind::LogicLevel(high) => Wire::logic_level(high),
                    WireKind::SoftwarePulse => Wire::software_pulse(),
                    WireKind::Parameters => Wire::parameters(),
                    WireKind::MemoryOutputs => Wire::memory_outputs(),
                    WireKind::Stream => Wire::stream(),
                    WireKind::OutputChannels => Wire::output_channels(),
                    WireKind::IrqRoutes => Wire::irq_routes(),
                    WireKind::PhaseEdges => Wire::phase_edges(),
                    WireKind::BridgeRegisters => Wire::bridge_registers(),
                    WireKind::OpticalPosition => Wire::optical_position(),
                    WireKind::Exposure => Wire::exposure(),
                    WireKind::Focus => Wire::focus(),
                    WireKind::FramePeriod => Wire::frame_period(),
                    WireKind::ImageGrid => Wire::image_grid(),
                    WireKind::Aperture => Wire::aperture(),
                },
            );
        }
        let mut assembly = Assembly {
            graph: DeviceGraph::new(),
            node_ids: BTreeMap::new(),
            core_interrupts: vec![],
            contacts: BTreeMap::new(),
            resources: Resources::default(),
        };
        let mut dependencies = vec![];
        let mut direct_dependencies = vec![];
        for node in self
            .nodes
            .into_iter()
            .filter(|n| n.boot_entries.contains(&self.boot_entry))
        {
            let id = assembly.graph.nodes.len();
            let mut public_id = id;
            let refs = node
                .model
                .node_references()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if matches!(&node.model, Model::Power { .. }) {
                let Model::Power { options, gpio } = node.model else {
                    unreachable!()
                };
                let devices = construct::Builder {
                    name: &node.name,
                    id,
                    windows: &node.windows,
                    resources: &mut resources,
                    assembly: &mut assembly,
                    start_tick,
                    adapter,
                }
                .serial_devices(options, gpio)?;
                for (mapping, device) in devices {
                    if let Some(mapping) = mapping {
                        assembly.graph.register_windows(vec![mapping], device)?;
                    } else {
                        assembly.graph.attach(device);
                    }
                }
                assert_eq!(assembly.graph.nodes.len(), id + 5);
                // Every physical register block can start or cancel a serial
                // deadline. Wake the shared coordinator when any is accessed.
                // DMA consolidation remaps these edges below.
                direct_dependencies.extend((0..4).map(|offset| (id + 4, id + offset)));
            } else if let Model::SerialDmaLink { options, input } = node.model {
                let devices =
                    fujimod_emulator_devices::exr2::serial_dma::Assembly::new(options, input)?;
                let expected: BTreeSet<_> = devices
                    .windows()
                    .iter()
                    .map(|&(begin, end)| (begin, end - begin))
                    .collect();
                let declared = node
                    .windows
                    .iter()
                    .map(|window| (window.base, window.size))
                    .collect();
                if expected != declared {
                    return Err(
                        "serial/DMA assembly requires exactly its wired register windows".into(),
                    );
                }
                let mut ids = Vec::new();
                for (mappings, device) in devices.devices() {
                    ids.push(assembly.graph.register_windows(mappings, device)?);
                }
                assert_eq!(ids.len(), 3);
                assert_eq!(assembly.graph.nodes.len(), id + 3);
                public_id = ids[0];
                let (serial, selector, dma) = (ids[0], ids[1], ids[2]);
                direct_dependencies.extend([(dma, serial), (serial, dma), (dma, selector)]);
            } else {
                let device = construct::Builder {
                    name: &node.name,
                    id,
                    windows: &node.windows,
                    resources: &mut resources,
                    assembly: &mut assembly,
                    start_tick,
                    adapter,
                }
                .device(node.model)?;
                let actual = assembly.graph.register_windows(node.windows, device)?;
                assert_eq!(actual, id);
            }
            dependencies.push((
                public_id,
                node.dependencies
                    .into_iter()
                    .chain(refs)
                    .collect::<BTreeSet<_>>(),
            ));
            assembly.node_ids.insert(node.name, public_id);
        }
        let merged = consolidate_dma(&mut assembly.graph, &mut assembly.node_ids, &mut resources)?;
        let mut edges = vec![BTreeSet::new(); assembly.graph.nodes.len()];
        for (old_id, names) in dependencies {
            let id = merged.remap[old_id];
            for name in names {
                let source = *assembly
                    .node_ids
                    .get(&name)
                    .ok_or("native dependency absent for selected boot entry")?;
                if source != id {
                    edges[id].insert(source);
                }
            }
        }
        for (old_target, old_source) in direct_dependencies {
            let target = merged.remap[old_target];
            let source = merged.remap[old_source];
            if target != source {
                edges[target].insert(source);
            }
        }
        for users in resources.users.values() {
            for &id in users {
                edges[id].extend(users.iter().copied().filter(|&source| source != id));
            }
        }
        for (id, sources) in edges.into_iter().enumerate() {
            assembly
                .graph
                .depend_on(id, sources.into_iter().collect())?;
        }
        assembly.resources = resources;
        Ok(assembly)
    }
}
