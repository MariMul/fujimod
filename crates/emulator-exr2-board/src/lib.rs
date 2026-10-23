//! EXR II board construction: builds every node of a checked plan with its
//! real Rust constructor. A remaining language implementation is an explicit
//! adapter node supplied by the native frontend; absent adapters refuse. Final
//! crates call [`register_board_builder`] so `Plan::build` reaches this crate.
mod construct;
pub mod serial;
#[cfg(test)]
mod tests;
use fujimod_emulator_exr2_serial::dma;
use fujimod_emulator_devices::graph::{DeviceGraph, Node as GraphNode, validate_mappings};
use fujimod_emulator_exr2_plan::{AdapterFactory, BuiltBoard, Model, Plan, Resources, Wire};
use std::collections::{BTreeMap, BTreeSet};

/// Decode a model's options, which plans carry as JSON for the model crate to read.
pub(crate) fn decode<T: serde::de::DeserializeOwned>(
    value: impl std::borrow::Borrow<serde_json::Value>,
) -> Result<T, String> {
    serde_json::from_value(value.borrow().clone()).map_err(|e| format!("EXR II model options: {e}"))
}

/// Make `Plan::build` construct boards with this crate. Final crates call
/// this before building any EXR II board.
pub fn register_board_builder() {
    fujimod_emulator_exr2_plan::set_board_builder(build);
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

fn build(
    plan: Plan,
    mut resources: Resources,
    start_tick: u64,
    adapter: &mut dyn AdapterFactory,
) -> Result<BuiltBoard, String> {
    plan.validate()?;
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
    for (name, kind) in plan.wires {
        use fujimod_emulator_exr2_plan::WireKind;
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
    let mut built = BuiltBoard {
        graph: DeviceGraph::new(),
        node_ids: BTreeMap::new(),
        core_interrupts: vec![],
        contacts: BTreeMap::new(),
        resources: Resources::default(),
    };
    let mut dependencies = vec![];
    let mut direct_dependencies = vec![];
    for node in plan
        .nodes
        .into_iter()
        .filter(|n| n.boot_entries.contains(&plan.boot_entry))
    {
        let id = built.graph.nodes.len();
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
                built: &mut built,
                start_tick,
                adapter,
            }
            .serial_devices(options, gpio)?;
            for (mapping, device) in devices {
                if let Some(mapping) = mapping {
                    built.graph.register_windows(vec![mapping], device)?;
                } else {
                    built.graph.attach(device);
                }
            }
            assert_eq!(built.graph.nodes.len(), id + 5);
            // Every physical register block can start or cancel a serial
            // deadline. Wake the shared coordinator when any is accessed.
            // DMA consolidation remaps these edges below.
            direct_dependencies.extend((0..4).map(|offset| (id + 4, id + offset)));
        } else if let Model::SerialDmaLink { options, input } = node.model { let options: fujimod_emulator_exr2_serial::serial_dma::Options = crate::decode(options)?; let input: fujimod_emulator_exr2_serial::serial_dma::Input = crate::decode(input)?; 
            let devices = fujimod_emulator_exr2_serial::serial_dma::Nodes::new(options, input)?;
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
                return Err("serial/DMA nodes require exactly their wired register windows".into());
            }
            let mut ids = Vec::new();
            for (mappings, device) in devices.devices() {
                ids.push(built.graph.register_windows(mappings, device)?);
            }
            assert_eq!(ids.len(), 3);
            assert_eq!(built.graph.nodes.len(), id + 3);
            public_id = ids[0];
            let (serial, selector, dma) = (ids[0], ids[1], ids[2]);
            direct_dependencies.extend([(dma, serial), (serial, dma), (dma, selector)]);
        } else {
            let device = construct::Builder {
                name: &node.name,
                id,
                windows: &node.windows,
                resources: &mut resources,
                built: &mut built,
                start_tick,
                adapter,
            }
            .device(node.model)?;
            let actual = built.graph.register_windows(node.windows, device)?;
            assert_eq!(actual, id);
        }
        dependencies.push((
            public_id,
            node.dependencies
                .into_iter()
                .chain(refs)
                .collect::<BTreeSet<_>>(),
        ));
        built.node_ids.insert(node.name, public_id);
    }
    let merged = consolidate_dma(&mut built.graph, &mut built.node_ids, &mut resources)?;
    let mut edges = vec![BTreeSet::new(); built.graph.nodes.len()];
    for (old_id, names) in dependencies {
        let id = merged.remap[old_id];
        for name in names {
            let source = *built
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
        built.graph.depend_on(id, sources.into_iter().collect())?;
    }
    built.resources = resources;
    Ok(built)
}
