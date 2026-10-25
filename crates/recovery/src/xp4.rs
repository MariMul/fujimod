//! Topaz raw FFINIT, relocated DDR setup, and cold preloaded startup.
use crate::image::{self, Edit, Plan, Result, extent, hash, put, require, word};
use serde::{Deserialize, Serialize};
#[allow(dead_code)]
mod template {
    include!("ffinit.generated.rs");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileApi {
    pub mount: u32,
    pub open: u32,
    pub read: u32,
    pub close: u32,
    pub load_address: u32,
    pub workspace: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub dat_offset: usize,
    pub active_size: usize,
    pub active_sha256: String,
    pub slot_size: u32,
    pub flash_aperture: u32,
    pub code_offset: usize,
    pub code_size: usize,
    pub code_sha256: String,
    pub continuation_offset: usize,
    pub continuation_address: u32,
    pub linked_sram_base: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColdStart {
    pub mode_flag: usize,
    pub mode_test: usize,
}

fn mov_half(word: u32, register: u32, top: bool) -> Result<u32> {
    require(
        word & 0xfff0f000 == (if top { 0xe3400000 } else { 0xe3000000 }) | register << 12,
        "Expected A32 MOVW/MOVT",
    )?;
    Ok((word >> 4 & 0xf000) | (word & 0xfff))
}
fn immediate(word: u32, value: u32) -> u32 {
    (word & !0x000f0fff) | ((value & 0xf000) << 4) | (value & 0xfff)
}
fn pair(code: &[u8], at: usize, register: u32) -> Result<u32> {
    Ok(mov_half(word(code, at)?, register, false)?
        | mov_half(word(code, at + 4)?, register, true)? << 16)
}
fn branch(code: &[u8], at: usize, opcode: u32) -> Result<usize> {
    let w = word(code, at)?;
    require(w >> 24 == opcode, "Changed A32 branch")?;
    let target = at as i64 + 8 + ((((w & 0xffffff) as i32) << 8 >> 6) as i64);
    require(
        target >= 0 && (target as usize) < code.len(),
        "A32 branch outside loader",
    )?;
    Ok(target as usize)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Worker {
    pub compare: usize,
    pub call: usize,
    pub entry: usize,
    pub join: usize,
    pub barrier: u32,
}
pub fn worker(code: &[u8], flag: usize) -> Result<Worker> {
    let mut found = Vec::new();
    for at in (0..code.len().saturating_sub(40)).step_by(4) {
        if word(code, at + 8)? != 0xe5933000
            || word(code, at + 12)? != 0xe3530001
            || [
                word(code, at + 28)?,
                word(code, at + 32)?,
                word(code, at + 36)?,
            ] != [0xe3a02000, 0xe583200c, 0xf57ff04f]
        {
            continue;
        }
        let candidate = (|| -> Result<Worker> {
            require(
                pair(code, at, 3)? as usize == flag,
                "Worker uses a different mode flag",
            )?;
            let target = branch(code, at + 16, 0x0a)?;
            let entry = branch(code, target, 0xeb)?;
            require(
                branch(code, target + 4, 0xea)? == at + 20,
                "Worker barrier join changed",
            )?;
            let barrier = pair(code, at + 20, 3)?;
            Ok(Worker {
                compare: at + 12,
                call: target,
                entry,
                join: at + 20,
                barrier,
            })
        })();
        if let Ok(candidate) = candidate {
            found.push(candidate);
        }
    }
    require(found.len() == 1, "No unique checked XP4 worker decision")?;
    Ok(found.remove(0))
}
pub fn cold_plan(inverted: &[u8], plan: &Plan, cold: &ColdStart) -> Result<Plan> {
    let loader = plan
        .regions
        .first()
        .ok_or_else(|| image::Error::Invalid("Missing RAM loader".into()))?;
    require(loader.address == 0, "XP4 loader must start at RAM zero")?;
    let code = loader.source.read(inverted)?;
    require(
        word(&code, cold.mode_flag)? == 1,
        "XP4 cold reset flag changed",
    )?;
    let worker = worker(&code, cold.mode_flag)?;
    let mode: Vec<_> = plan
        .edits
        .iter()
        .filter(|e| e.address == cold.mode_flag)
        .collect();
    require(
        mode.len() == 1 && mode[0].old == 1 && mode[0].new == 0,
        "Expected exactly one XP4 warm-mode edit",
    )?;
    let mut plan = plan.clone();
    plan.edits.retain(|e| e.address != cold.mode_flag);
    for address in [cold.mode_test + 12, cold.mode_test + 84, worker.compare] {
        plan.edits.push(Edit {
            address,
            old: 0xe3530001,
            new: 0xe3530000,
            what: "cold preloaded comparison".into(),
        });
    }
    Ok(plan)
}
pub fn crc32(bytes: &[u8]) -> u32 {
    const fn table() -> [u32; 256] {
        let mut rows = [0; 256];
        let mut index = 0;
        while index < 256 {
            let mut value = index as u32;
            let mut bit = 0;
            while bit < 8 {
                value = (value >> 1) ^ if value & 1 == 1 { 0xedb88320 } else { 0 };
                bit += 1;
            }
            rows[index] = value;
            index += 1;
        }
        rows
    }
    const TABLE: [u32; 256] = table();
    let mut crc = u32::MAX;
    for byte in bytes {
        crc = (crc >> 8) ^ TABLE[((crc ^ u32::from(*byte)) & 255) as usize];
    }
    !crc
}

/// Construct using only supplied setup bytes and the processor's file ABI.
/// The low-address entry supports execution harnesses using the same loader.
pub fn ffinit(
    inverted: &[u8],
    setup: &Setup,
    api: &FileApi,
    payload: &[u8],
    base: u32,
    workspace_override: Option<u32>,
) -> Result<Vec<u8>> {
    let prefix = extent(inverted, setup.dat_offset, setup.active_size)?;
    hash(prefix, &setup.active_sha256)?;
    let code = extent(prefix, setup.code_offset, setup.code_size)?;
    hash(code, &setup.code_sha256)?;
    let data_start = setup
        .code_offset
        .checked_add(setup.code_size)
        .ok_or_else(|| image::Error::Invalid("Setup extent overflow".into()))?;
    require(
        data_start < setup.continuation_offset
            && data_start.is_multiple_of(4)
            && setup.continuation_offset.is_multiple_of(4),
        "Invalid DDR data extent",
    )?;
    let original_data = extent(prefix, data_start, setup.continuation_offset - data_start)?;
    require(
        api.load_address == setup.linked_sram_base,
        "File ABI and DDR setup link disagree",
    )?;
    let stack_offset = api
        .workspace
        .checked_sub(setup.linked_sram_base)
        .ok_or_else(|| image::Error::Invalid("ROM workspace precedes setup".into()))?;
    let stack = base
        .checked_add(stack_offset)
        .ok_or_else(|| image::Error::Invalid("Stack overflow".into()))?
        & !0xfff;
    let region = base
        .checked_add(0x4000)
        .ok_or_else(|| image::Error::Invalid("Code region overflow".into()))?;
    require(
        code.len() + 0x1400 < 0x4000 && stack > region + 0x1000,
        "DDR setup or stack overlaps loader",
    )?;
    let data_address = region
        .checked_add(template::FFINIT_SETUP_DATA as u32)
        .ok_or_else(|| image::Error::Invalid("DDR placement overflow".into()))?;
    let source_data = setup
        .flash_aperture
        .checked_add(data_start as u32)
        .ok_or_else(|| image::Error::Invalid("Flash pointer overflow".into()))?;
    let flash_end = u64::from(setup.flash_aperture) + u64::from(setup.slot_size);
    let is_flash = |value: u32| {
        u64::from(value) >= u64::from(setup.flash_aperture) && u64::from(value) < flash_end
    };
    let data_end = setup.flash_aperture as u64 + setup.continuation_offset as u64;
    let relocate = |value: u32| -> Result<u32> {
        require(
            value >= source_data && u64::from(value) < data_end,
            "DDR flash reference outside extracted data",
        )?;
        data_address
            .checked_add(value - source_data)
            .ok_or_else(|| image::Error::Invalid("DDR pointer overflow".into()))
    };
    let mut data = original_data.to_vec();
    for at in (0..data.len()).step_by(4) {
        let value = word(&data, at)?;
        if !is_flash(value) {
            continue;
        }
        let relocated = relocate(value)?;
        require(at >= 4, "Unbounded DDR table pointer")?;
        let count = word(&data, at - 4)? as usize;
        let offset = (value - source_data) as usize;
        require(
            count > 0 && count <= 0x400 && offset + count * 8 <= data.len(),
            "DDR pointer is not a bounded register table",
        )?;
        for row in 0..count {
            require(
                (0xf0000000..0xfffff000).contains(&word(original_data, offset + row * 8)?),
                "Invalid DDR register address",
            )?;
        }
        put(&mut data, at, relocated)?;
    }
    let mut output =
        vec![0; 0x4000 + template::FFINIT_SETUP_DATA + data.len() + template::SUFFIX.len()];
    let final_size = (output.len() + 3) & !3;
    // The progress structure is zero-initialized BSS following the strings.
    output.resize(final_size + 16, 0);
    require(
        u64::from(base) + output.len() as u64 <= u64::from(stack) - 0x1000,
        "FFINIT overlaps ROM workspace or stack",
    )?;
    output[..code.len()].copy_from_slice(code);
    output[0x4000..0x4000 + template::TEXT.len()].copy_from_slice(template::TEXT);
    let target = &mut output[0x4000..];
    for index in 0..256 {
        let mut value = index as u32;
        for _ in 0..8 {
            value = (value >> 1) ^ if value & 1 == 1 { 0xedb88320 } else { 0 };
        }
        put(target, template::FFINIT_CRC_TABLE + index * 4, value)?;
    }
    let workspace = payload
        .len()
        .checked_add(0xffff)
        .map(|n| n & !0xffff)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| image::Error::Invalid("Payload workspace overflow".into()))?;
    let workspace = workspace_override.unwrap_or(workspace);
    for (index, value) in [
        0,
        payload.len() as u32,
        crc32(payload),
        workspace,
        api.mount,
        api.open,
        api.read,
        api.close,
        data_address,
    ]
    .into_iter()
    .enumerate()
    {
        put(target, template::FFINIT_CONFIG + index * 4, value)?;
    }
    target[template::FFINIT_BOOT_HEADER..template::FFINIT_BOOT_HEADER + 0x800]
        .copy_from_slice(extent(prefix, 0, 0x800)?);
    target[template::FFINIT_SETUP_DATA..template::FFINIT_SETUP_DATA + data.len()]
        .copy_from_slice(&data);
    let suffix = template::FFINIT_SETUP_DATA + data.len();
    target[suffix..suffix + template::SUFFIX.len()].copy_from_slice(template::SUFFIX);
    for &(at, kind, value) in template::FIXUPS {
        let address = match value {
            -1 => stack,
            -2 => base + 4,
            offset => {
                let offset = offset as usize;
                let shifted = if offset >= template::FFINIT_SETUP_DATA + 4 {
                    offset + data.len() - 4
                } else {
                    offset
                };
                region
                    .checked_add(shifted as u32)
                    .ok_or_else(|| image::Error::Invalid("Template relocation overflow".into()))?
            }
        };
        let old = word(target, at)?;
        let value = match kind {
            2 => address,
            43 => immediate(old, address),
            44 => immediate(old, address >> 16),
            _ => unreachable!(),
        };
        put(target, at, value)?;
    }
    let mut references = 0;
    let mut literals = Vec::new();
    for at in (0..code.len()).step_by(4) {
        let low = word(code, at)?;
        if is_flash(low) {
            literals.push((at, low));
        }
        if at + 8 > code.len() {
            continue;
        }
        let high = word(code, at + 4)?;
        if low >> 28 != high >> 28 {
            continue;
        }
        let register = low >> 12 & 15;
        if let (Ok(lo), Ok(hi)) = (
            mov_half((low & 0x0fffffff) | 0xe0000000, register, false),
            mov_half((high & 0x0fffffff) | 0xe0000000, register, true),
        ) {
            let value = lo | hi << 16;
            if is_flash(value) {
                let new = relocate(value)?;
                put(&mut output, at, immediate(low, new))?;
                put(&mut output, at + 4, immediate(high, new >> 16))?;
                references += 1;
            }
        }
    }
    require(
        references > 0 && literals == [(0xa0, setup.continuation_address)],
        "DDR setup has additional absolute flash literals or no data references",
    )?;
    require(word(&output, 0)? == 0xf1010000, "DDR entry differs")?;
    let destination = 0x4000 + template::FFINIT_ENTRY;
    put(
        &mut output,
        0,
        0xea000000 | (((destination as u32 - 8) >> 2) & 0xffffff),
    )?;
    put(
        &mut output,
        0xa0,
        region + template::FFINIT_DRAM_READY as u32,
    )?;
    Ok(output)
}

pub fn layout(setup: &Setup, api: &FileApi, base: u32) -> serde_json::Value {
    let data_size = setup.continuation_offset - setup.code_offset - setup.code_size;
    let region = base + 0x4000;
    let progress = template::FFINIT_PROGRESS + data_size - 4;
    let symbols = serde_json::json!({
        "ffinit_entry":region+template::FFINIT_ENTRY as u32,
        "ffinit_dram_ready":region+template::FFINIT_DRAM_READY as u32,
        "ffinit_prepare":region+template::FFINIT_PREPARE as u32,
        "ffinit_load":region+template::FFINIT_LOAD as u32,
        "ffinit_progress":region+progress as u32,
        "ffinit_boot_header":region+template::FFINIT_BOOT_HEADER as u32,
        "ffinit_setup_data":region+template::FFINIT_SETUP_DATA as u32,
        "ffinit_config":region+template::FFINIT_CONFIG as u32,
        "ffinit_counter_start":region+template::FFINIT_COUNTER_START as u32,
        "ffinit_counter_read":region+template::FFINIT_COUNTER_READ as u32,
        "ffinit_counter_stop":region+template::FFINIT_COUNTER_STOP as u32,
        "ffinit_crc32":region+template::FFINIT_CRC32 as u32,
        "ffinit_halt":region+template::FFINIT_HALT as u32
    });
    serde_json::json!({"load_address":base,"code_region":region,"stack_top":(api.workspace-api.load_address+base)&!0xfff,"symbols":symbols,"sections":[{"name":".text","offset":0,"size":template::TEXT.len()}]})
}
