//! Parallel NAND main-area reader, geometry and correction contract.
use super::flash::{Locator, Routine, number, one};
use crate::Lookup;
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;

fn wide_constants(r: &Routine) -> Vec<u64> {
    r.code
        .windows(2)
        .filter_map(|p| {
            let (a, b) = (&p[0], &p[1]);
            let suffix = a.mnemonic.strip_prefix("movw")?;
            if b.mnemonic != format!("movt{suffix}") {
                return None;
            }
            let (reg, low) = a.operands.split_once(", #")?;
            let (other, high) = b.operands.split_once(", #")?;
            (reg == other).then(|| Some(number(low).ok()? | number(high).ok()? << 16))?
        })
        .collect()
}
fn stores(r: &Routine) -> BTreeMap<u64, u64> {
    let mut regs = BTreeMap::<String, u64>::new();
    let mut out = BTreeMap::new();
    for i in &r.code {
        let ops: Vec<_> = i.operands.split(", ").collect();
        if i.mnemonic == "bl" {
            for reg in ["r0", "r1", "r2", "r3", "ip"] {
                regs.remove(reg);
            }
            continue;
        }
        if i.mnemonic == "str" && ops.len() == 2 {
            if let (Some(value), Some(address)) = (
                regs.get(ops[0]),
                regs.get(ops[1].trim_start_matches('[').trim_end_matches(']')),
            ) {
                out.insert(*address, *value);
            }
        }
        let value = match i.mnemonic.as_str() {
            "mov" | "movw" if ops.len() == 2 => number(ops[1].trim_start_matches('#')).ok(),
            "movt" if ops.len() == 2 => regs.get(ops[0]).and_then(|low| {
                number(ops[1].trim_start_matches('#'))
                    .ok()
                    .map(|high| (low & 0xffff) | (high << 16))
            }),
            _ => None,
        };
        if let Some(reg) = ops.first() {
            if let Some(value) = value {
                regs.insert(reg.to_string(), value);
            } else if !matches!(i.mnemonic.as_str(), "str" | "strb" | "cmp" | "tst") {
                regs.remove(*reg);
            }
        }
    }
    out
}
fn calls(r: &Routine, target: u64) -> bool {
    r.code.iter().any(|i| {
        i.mnemonic == "bl" && number(i.operands.trim_start_matches('#')).ok() == Some(target)
    })
}
/// Locate a corrected 2 KiB physical-page read ABI. The update origin is decoded
/// by the container reader. No execution transport is claimed by this fact.
/// Call under the located driver lock, with a zero-initialized progress word.
pub fn exr2_flash<L: Lookup>(
    index: &L,
    firmware: &str,
    image_origin: u64,
    control_offset: u64,
) -> Result<Value> {
    let locator = Locator {
        index,
        firmware,
        unit: "main".into(),
    };
    let initialized = locator.named("nand_driver_initialized", |r| {
        r.has("ldr", "r0, [r3]") && r.code.len() == 8
    })?;
    let flag = one(wide_constants(&initialized), "NAND initialized flag")?;
    let initializer = locator.named("nand_initialize_driver", |r| {
        stores(r).get(&flag) == Some(&1)
    })?;
    let assignments = stores(&initializer);
    let getter = locator.named("nand_get_page_size", |r| {
        r.has("ldrheq", "r0, [r3, #8]")
            && r.has("addeq", "r2, r2, r2, lsl #1")
            && r.has("addeq", "r3, r3, r2, lsl #3")
    })?;
    let constants = wide_constants(&getter);
    let mut geometry = vec![];
    for &address in &constants {
        let Some(&selected) = assignments.get(&address) else {
            continue;
        };
        if address == flag || selected > 3 {
            continue;
        }
        for &table in &constants {
            let Some(record) = index
                .read(firmware, "main", table + selected * 24, 24)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let blocks = u32::from_le_bytes(record[..4].try_into().unwrap());
            let page = u16::from_le_bytes(record[8..10].try_into().unwrap()) as u32;
            let pages = u32::from(record[10]);
            if (512..=8192).contains(&blocks) && matches!((page, pages), (512, 32) | (2048, 64)) {
                geometry.push((address, selected, table, blocks, page, pages));
            }
        }
    }
    let (geometry_address, selected, table, blocks, page, pages) =
        one(geometry, "selected NAND geometry")?;
    if page != 2048 {
        return Err(format!(
            "selected NAND geometry has {page}-byte pages; corrected small-page ABI is unresolved"
        ));
    }
    let reader = locator.named("nand_read_transfer", |r| {
        (r.has("mov", "r8, r8, lsl #2") || r.has("lsl", "r8, r8, #2"))
            && r.has("tst", "r1, #0x1f")
            && r.has("mov", "r1, #0x30")
            && r.has("movw", "r3, #0x8003")
            && r.has("movne", "r6, #0xa")
            && r.has("moveq", "r3, #0x70")
            && r.has("movne", "r3, #0x40")
    })?;
    let gate = locator.named("nand_set_controller_gate", |r| {
        r.has("cmp", "r0, #0")
            && r.has("cmp", "r0, #1")
            && r.has("mov", "r0, #1")
            && r.has("mov", "r0, #0")
    })?;
    let correction = locator.named("nand_set_location_bank_enable", |r| {
        r.has("movne", "r2, #0x80000000") && r.has("str", "r2, [r3, #-0xf67]")
    })?;
    let poll = locator.named("nand_poll_controller_busy", |r| {
        r.has("and", "r3, r3, #4") && r.has("movw", "r4, #0x271")
    })?;
    let command = locator.named("nand_pack_command_bytes", |r| {
        r.has("bfi", "ip, r1, #0x10, #8") && r.has("str", "ip, [r3, #-0xfe3]")
    })?;
    let trigger = locator.named("nand_set_trigger", |r| r.has("str", "r2, [r3, #-0xff7]"))?;
    let timing = locator.named("nand_get_timing", |r| r.has("ldr", "r3, [r3, #-0xffb]"))?;
    let wrapper = locator.named("nand_read_pages_wrapper", |r| {
        calls(r, reader.header.key.address)
            && r.has("str", "ip, [sp, #0xc]")
            && r.has("str", "ip, [sp, #8]")
    })?;
    let physical = locator.named("nand_read_physical_blocks", |r| {
        calls(r, wrapper.header.key.address) && wide_constants(r).contains(&table)
    })?;
    let lock = locator.routine(
        physical
            .header
            .calls
            .first()
            .ok_or("physical NAND reader has no lock")?
            .1,
    )?;
    let unlock = locator.routine(
        physical
            .header
            .calls
            .last()
            .ok_or("physical NAND reader has no unlock")?
            .1,
    )?;
    let semaphore = one(
        lock.code
            .iter()
            .filter_map(|i| {
                (i.mnemonic == "mov" && i.operands.starts_with("r0, #")).then(|| i.operands.clone())
            })
            .collect(),
        "NAND lock semaphore",
    )?;
    if lock.code.len() != 6
        || !lock.has("mvn", "r1, #0")
        || unlock.code.len() != 5
        || !unlock.has("mov", &semaphore)
        || !calls(&physical, lock.header.key.address)
        || !calls(&physical, unlock.header.key.address)
    {
        return Err("physical NAND reader lock/unlock contract differs".into());
    }
    for routine in [&gate, &poll, &command, &trigger, &timing] {
        if !calls(&reader, routine.header.key.address) {
            return Err("NAND read family does not call the verified controller helpers".into());
        }
    }
    if !calls(&gate, correction.header.key.address) || !calls(&poll, timing.header.key.address) {
        return Err("NAND correction gate or polling call graph differs".into());
    }
    let mut ranges = vec![reader.span("nand_read_transfer")];
    for (r, name) in [
        (&gate, "NAND correction mode"),
        (&correction, "NAND correction gate"),
        (&poll, "NAND completion polling"),
        (&command, "NAND read command bytes"),
        (&trigger, "NAND read trigger"),
        (&timing, "NAND corrected status"),
        (&lock, "NAND driver lock"),
        (&unlock, "NAND driver unlock"),
        (&initializer, "NAND geometry initialization"),
        (&getter, "NAND page geometry query"),
    ] {
        ranges.push(r.span(name));
    }
    ranges.push(
        json!({"name":"selected NAND geometry record","address":table+selected*24,"size":24}),
    );
    Ok(json!({
        "device":{"kind":"raw-nand"},
        "window":{"address":0,"size":u64::from(blocks)*u64::from(page)*u64::from(pages)},
        "update_image":{"dat_offset":image_origin,"xor":255},
        "reader":{"symbol":"nand_read_transfer","alignment":page,"nand":{"page_bytes":page,"pages_per_block":pages,"destination_alignment":32,"controller_mode":0,"event_mode":0,"timeout_ms":2000,"max_pages_per_call":1,"lock":lock.header.key.address,"unlock":unlock.header.key.address}},
        "transport":null,"code_ranges":ranges,
        "state_checks":[{"name":"NAND driver initialized","address":flag,"size":4,"value":1},{"name":"selected NAND geometry","address":geometry_address,"size":4,"value":selected}],
        "control":{"offset":control_offset,"size":32768}
    }))
}

/// Choose an update-backed control from the raw reset-vector allocation. The
/// first-stage header grammar and an aligned complete ARM vector table must
/// agree; this does not depend on the function index containing reset code.
pub fn exr2_control(image: &[u8]) -> Result<u64> {
    let signature = [0x00ffaa55u32, 0x01234567, 0x89abcdef];
    if !signature
        .iter()
        .enumerate()
        .all(|(i, word)| image.get(i * 4..i * 4 + 4) == Some(word.to_le_bytes().as_slice()))
    {
        return Err("EXR II NAND first-stage header differs".into());
    }
    let vectors: Vec<u8> = (0..8).flat_map(|_| 0xe59ff018u32.to_le_bytes()).collect();
    let prefix = &image[..image.len().min(0x100000)];
    let candidates: Vec<u64> = prefix
        .windows(vectors.len())
        .enumerate()
        .filter_map(|(p, bytes)| {
            if bytes != vectors || !p.is_multiple_of(0x4000) || image.get(p..p + 32768).is_none() {
                return None;
            }
            Some(p as u64)
        })
        .collect();
    candidates
        .into_iter()
        .min()
        .ok_or_else(|| "no update-backed NAND reset-vector control".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_uses_the_first_aligned_complete_reset_allocation() {
        let mut image = vec![0; 0x18000];
        for (p, word) in [0x00ffaa55u32, 0x01234567, 0x89abcdef].iter().enumerate() {
            image[p * 4..p * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        for start in [0x4000, 0x8000] {
            for p in 0..8 {
                image[start + p * 4..start + p * 4 + 4]
                    .copy_from_slice(&0xe59ff018u32.to_le_bytes());
            }
        }
        assert_eq!(exr2_control(&image).unwrap(), 0x4000);
        image[0] ^= 1;
        assert!(exr2_control(&image).is_err());
        image[0] ^= 1;
        assert!(exr2_control(&image[..0x9000]).is_err());
    }
}
