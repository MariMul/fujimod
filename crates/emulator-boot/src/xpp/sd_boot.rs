//! X-T20 cold SD start with the ROM window and its low address decoder.
use super::{Assembly, ProcessorStart};
use fujimod_catalog_resolution::ResolvedConfiguration;
use fujimod_emulator::board::{
    decoder::{Choice, Decoder},
    factory::{MemoryMap, Peripheral, Ram},
    memory::BusMapping,
};
use fujimod_emulator_cpu::Register;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const BOOT_ROM_ADDRESS: u64 = 0xfff0_0000;
const BOOT_ROM_BYTES: usize = 0x8000;
const SRAM: u64 = 0xfff0_d000;
const SRAM_BYTES: usize = 0x1000;
const UNCACHED_DRAM: u64 = 0x4000_0000;

pub(crate) fn card_rom(
    resolved: &ResolvedConfiguration,
    path: &Path,
    starts: &mut [ProcessorStart],
) -> Result<Vec<u8>, String> {
    let firmware = &resolved.reference.firmware;
    let identity = firmware
        .boot_rom
        .as_ref()
        .ok_or("camera has no boot ROM identity")?;
    let mut rom = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if firmware.body_id != "xt20"
        || identity.address != BOOT_ROM_ADDRESS
        || rom.len() < BOOT_ROM_BYTES
        || rom.len() as u64 != identity.content.size
        || format!("{:x}", Sha256::digest(&rom)) != identity.content.sha256
    {
        return Err("SD boot requires the checked X-T20 boot ROM".into());
    }
    // The dump includes mutable state above the ROM's translation table.
    rom.truncate(BOOT_ROM_BYTES);
    for start in starts {
        start.registers = vec![(Register::Pc, BOOT_ROM_ADDRESS), (Register::Pstate, 0xd3)];
    }
    Ok(rom)
}

pub(crate) fn configure_sd_boot(assembly: &mut Assembly) -> Result<(), String> {
    let graph = &mut assembly.execution;
    // ROM reads and writes card sectors through the uncached DRAM alias.
    let mut uncached = graph
        .mappings
        .iter_mut()
        .filter(|mapping| mapping.address == UNCACHED_DRAM && mapping.physical == 0);
    let uncached_size = uncached
        .next()
        .map(|mapping| {
            mapping.permissions = 7;
            mapping.size
        })
        .ok_or("SD boot needs the uncached DRAM mapping")?;
    for mapping in uncached {
        mapping.permissions = 7;
    }
    if !graph
        .dma_mappings
        .iter()
        .any(|mapping| mapping.address == UNCACHED_DRAM)
    {
        graph.dma_mappings.push(BusMapping {
            address: UNCACHED_DRAM,
            physical: 0,
            size: uncached_size,
            permissions: 3,
        });
    }
    let mut dram = Vec::with_capacity(graph.processors.len());
    for mapping in graph
        .mappings
        .iter_mut()
        .filter(|mapping| mapping.address == 0)
    {
        if mapping.physical != 0 || mapping.size <= BOOT_ROM_BYTES {
            return Err("SD boot needs a contiguous low DRAM mapping".into());
        }
        dram.push(MemoryMap {
            size: BOOT_ROM_BYTES,
            ..mapping.clone()
        });
        mapping.address += BOOT_ROM_BYTES as u64;
        mapping.physical += BOOT_ROM_BYTES as u64;
        mapping.size -= BOOT_ROM_BYTES;
    }
    if dram.len() != graph.processors.len() {
        return Err("SD boot needs one low DRAM mapping for each core".into());
    }
    let low = graph
        .dma_mappings
        .iter_mut()
        .find(|mapping| mapping.address == 0)
        .ok_or("SD boot needs a low DMA window")?;
    if low.physical != 0 || low.size <= BOOT_ROM_BYTES {
        return Err("SD boot needs a contiguous low DMA window".into());
    }
    let dram_dma = BusMapping {
        size: BOOT_ROM_BYTES,
        ..*low
    };
    low.address += BOOT_ROM_BYTES as u64;
    low.physical += BOOT_ROM_BYTES as u64;
    low.size -= BOOT_ROM_BYTES;

    let regions = [(BOOT_ROM_ADDRESS, BOOT_ROM_BYTES, 5), (SRAM, SRAM_BYTES, 7)];
    for (base, size, permissions) in regions {
        graph.ram.push(Ram { base, size });
        graph.dma_mappings.push(BusMapping {
            address: base,
            physical: base,
            size,
            permissions: permissions & 3,
        });
        graph
            .mappings
            .extend((0..graph.processors.len()).map(|core| MemoryMap {
                core,
                address: base,
                physical: base,
                size,
                permissions,
            }));
        let end = base + size as u64;
        graph.peripherals = std::mem::take(&mut graph.peripherals)
            .into_iter()
            .flat_map(|region| {
                let upper = region.address + region.size as u64;
                if region.address >= end || upper <= base {
                    return [Some(region), None];
                }
                let lower = (region.address < base).then(|| Peripheral {
                    size: (base - region.address) as usize,
                    ..region
                });
                let higher = (upper > end).then(|| Peripheral {
                    address: end,
                    physical: region.physical + end - region.address,
                    size: (upper - end) as usize,
                    core: region.core,
                });
                [lower, higher]
            })
            .flatten()
            .collect();
    }
    let rom = dram
        .iter()
        .map(|mapping| MemoryMap {
            physical: BOOT_ROM_ADDRESS,
            permissions: 5,
            ..mapping.clone()
        })
        .collect::<Vec<_>>();
    let rom_dma = BusMapping {
        physical: BOOT_ROM_ADDRESS,
        permissions: 1,
        ..dram_dma
    };
    graph.mappings.extend(rom.iter().cloned());
    graph.dma_mappings.push(rom_dma);
    let decoder = graph.decoders.len();
    graph.decoders.push(Decoder {
        initial: 0,
        choices: vec![
            Choice {
                processors: rom,
                dma: vec![rom_dma],
            },
            Choice {
                processors: dram,
                dma: vec![dram_dma],
            },
        ],
    });

    let selector = 0xfff0_a000;
    let ddr = 0xfff0_e000;
    let power = 0xfff1_0000;
    let remap = 0xff00_0000;
    let bus = (0xff24_2000..0xff24_9000)
        .step_by(0x1000)
        .collect::<Vec<u64>>();
    let ports = [0xff14_2000, 0xff14_3000, 0xff14_7000, 0xff14_8000];
    let windows = [selector, ddr, power, remap]
        .into_iter()
        .chain(bus.iter().copied())
        .chain(ports)
        .collect::<Vec<_>>();
    for core in 0..graph.processors.len() {
        for &address in &windows {
            if !graph.peripherals.iter().any(|p| {
                p.core == core
                    && address >= p.address
                    && address + 0x1000 <= p.address + p.size as u64
            }) {
                graph.peripherals.push(Peripheral {
                    core,
                    address,
                    physical: address,
                    size: 0x1000,
                });
            }
        }
    }
    let nodes = assembly.devices["nodes"]
        .as_array_mut()
        .ok_or("XPP graph needs device nodes")?;
    nodes.retain(|node| {
        !node["model"]["base"]
            .as_u64()
            .is_some_and(|base| windows.contains(&base))
    });
    nodes.insert(
        0,
        json!({
            "name": "boot",
            "model": {
                "kind": "boot", "keys": ["sd-strap"],
                "options": {
                    "selector_base": selector, "clock_base": power, "ddr_base": ddr,
                    "remap_base": remap, "bus_bases": bus, "memory_port_bases": ports,
                    "decoder": decoder, "normal_selector": 0x18, "card_selector": 0,
                    "settle_ticks": 2, "refresh_level": 0,
                },
            },
        }),
    );
    Ok(())
}
