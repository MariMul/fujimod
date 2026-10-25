//! Copied XP5 setup with a post-DDR FAT32 loader and gated RAM handoff.
use crate::image::{self, File, Result, Source, extent, put, require, word};
use serde::{Deserialize, Serialize};
mod template {
    include!("xp5.generated.rs");
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub address: u64,
    pub mask: u32,
    pub value: u32,
    pub kind: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub host: u64,
    pub timeout_setting: u16,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProof {
    pub source: Source,
    pub offset: usize,
    pub size: usize,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub prefix: Source,
    pub low_length: usize,
    pub length_flags: u32,
    pub base: u64,
    pub stack: u64,
    pub active_size: usize,
    pub bootstrap_offset: usize,
    pub initialize_offset: usize,
    pub gate_offset: usize,
    pub delay: crate::Proof,
    pub reset_dat: usize,
    pub reset_copy_size: usize,
    pub selector: usize,
    pub board: Board,
    pub board_proofs: Vec<SourceProof>,
}
fn put64(data: &mut [u8], at: usize, value: u64) -> Result<()> {
    extent(data, at, 8)?;
    data[at..at + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn branch(at: u64, target: u64, link: bool) -> Result<u32> {
    let delta = i128::from(target) - i128::from(at);
    require(
        delta % 4 == 0 && (-(1 << 27)..(1 << 27)).contains(&delta),
        "AArch64 branch outside immediate range",
    )?;
    Ok((if link { 0x94000000 } else { 0x14000000 }) | (((delta >> 2) as u32) & 0x03ffffff))
}
pub fn package(inverted: &[u8], setup: &Setup, ram: Vec<u8>) -> Result<Vec<File>> {
    let mut prefix = setup.prefix.read(inverted)?;
    for proof in &setup.board_proofs {
        let source = proof.source.read(inverted)?;
        image::hash(extent(&source, proof.offset, proof.size)?, &proof.sha256)?;
    }
    require(
        prefix.len() == 0x40000 && (0x800..=prefix.len()).contains(&setup.active_size),
        "XP5 prefix extent differs",
    )?;
    require(
        setup.base.is_multiple_of(0x1000) && (0xff000000..0xffff0000).contains(&setup.base),
        "Invalid XP5 SRAM base",
    )?;
    require(
        setup.length_flags & !0xc0000000 == 0
            && word(&prefix, 0x10)? == setup.length_flags | setup.low_length as u32,
        "XP5 setup length field differs",
    )?;
    require(
        !ram.is_empty()
            && ram.len() < 1 << 30
            && setup
                .selector
                .checked_add(8)
                .is_some_and(|end| end <= setup.reset_copy_size)
            && setup.reset_copy_size <= ram.len(),
        "XP5 payload/reset extent differs",
    )?;
    let mut reset = extent(inverted, setup.reset_dat, setup.reset_copy_size)?.to_vec();
    require(
        word(&reset, setup.selector)? == 1 && word(&reset, setup.selector + 4)? == 0,
        "XP5 stock reset selector differs",
    )?;
    put64(&mut reset, setup.selector, 0)?;
    require(
        reset == ram[..setup.reset_copy_size],
        "XP5 reset copy differs beyond preloaded selector",
    )?;
    let copied_end = setup.base + setup.low_length as u64;
    let workspace = setup.base + prefix.len() as u64 - 0x1000;
    let first_free = (setup.stack + 0x100).max(setup.base + setup.active_size as u64 - 0x800);
    let address = (first_free + 0x3fff) & !0x3fff;
    let capacity = workspace.saturating_sub(address) / 0x4000 * 0x4000;
    require(
        setup.base < copied_end && copied_end < setup.stack && capacity >= 0x4000,
        "No XP5 extension above the stock stack and below boot workspace",
    )?;
    let mut program = template::IMAGE.to_vec();
    let end = (address + program.len() as u64 + 0xfff) & !0xfff;
    require(
        end <= address + capacity && end - setup.base <= prefix.len() as u64 - 0x800,
        "XP5 guest exceeds SRAM workspace",
    )?;
    let board = &setup.board;
    let cfg = template::SETUP_SD_BOARD;
    require(
        board.operations.len() <= 32
            && board.timeout_setting <= 14
            && (0xf0000000..0xfffff000).contains(&board.host),
        "Invalid XP5 SD binding",
    )?;
    require(
        extent(&program, cfg, 16 + 32 * 24)?.iter().all(|v| *v == 0),
        "XP5 board template changed",
    )?;
    put64(&mut program, cfg, board.host)?;
    put(&mut program, cfg + 8, u32::from(board.timeout_setting))?;
    put(&mut program, cfg + 12, board.operations.len() as u32)?;
    for (n, op) in board.operations.iter().enumerate() {
        require(
            op.kind <= 2
                && (op.kind == 2
                    || ((0xf0000000..0xfffff000).contains(&op.address)
                        && op.address.is_multiple_of(4))),
            "Invalid XP5 SD operation",
        )?;
        let at = cfg + 16 + n * 24;
        put64(&mut program, at, op.address)?;
        put(&mut program, at + 8, op.mask)?;
        put(&mut program, at + 12, op.value)?;
        put(&mut program, at + 16, op.kind)?;
    }
    let request = template::SETUP_SD_REQUEST;
    require(
        extent(&program, request, 40)? == [1u64, 0, 0, 0, 0].map(u64::to_le_bytes).concat(),
        "XP5 request template changed",
    )?;
    for (n, value) in [
        1,
        ram.len() as u64,
        u64::from(crate::xp4::crc32(&ram)),
        100,
        100000,
    ]
    .into_iter()
    .enumerate()
    {
        put64(&mut program, request + n * 8, value)?;
    }
    require(
        setup.bootstrap_offset >= 0x800
            && setup.gate_offset >= 0x800
            && setup.initialize_offset >= 0x800,
        "XP5 hook precedes setup",
    )?;
    let call = setup.base + setup.bootstrap_offset as u64 - 0x800 + 36;
    let gate = setup.base + setup.gate_offset as u64 - 0x800;
    let initialize = setup.base + setup.initialize_offset as u64 - 0x800;
    let delay_offset = (setup.delay.address as u64)
        .checked_sub(setup.base)
        .ok_or_else(|| image::Error::Invalid("XP5 delay precedes setup".into()))?
        as usize;
    image::hash(
        extent(&prefix, delay_offset + 0x800, setup.delay.size)?,
        &setup.delay.sha256,
    )?;
    for &(at, kind, name) in template::EXTERNALS {
        let target = match name {
            "boot_setup_initialize" => initialize,
            "boot_setup_delay" => setup.delay.address as u64,
            "boot_setup_ram_entry_resume" => gate + 4,
            _ => {
                return Err(image::Error::Invalid(
                    "Unexpected XP5 guest external".into(),
                ));
            }
        };
        put(
            &mut program,
            at,
            branch(address + at as u64, target, kind == 283)?,
        )?;
    }
    require(
        word(&prefix, setup.bootstrap_offset + 36)? == branch(call, initialize, true)?,
        "XP5 initializer call differs",
    )?;
    require(
        [
            word(&prefix, setup.gate_offset)?,
            word(&prefix, setup.gate_offset + 4)?,
            word(&prefix, setup.gate_offset + 8)?,
        ] == [0x580000de, 0xd5033fdf, 0xd65f03c0],
        "XP5 zero handoff differs",
    )?;
    let copied_size = (end - setup.base) as usize;
    require(
        extent(
            &prefix,
            setup.low_length + 0x800,
            copied_size - setup.low_length,
        )?
        .iter()
        .all(|v| *v == 0xff),
        "XP5 extension overwrites stock bytes",
    )?;
    let offset = (address - setup.base) as usize + 0x800;
    prefix[offset..offset + program.len()].copy_from_slice(&program);
    put(
        &mut prefix,
        setup.bootstrap_offset + 36,
        branch(call, address + template::SETUP_SD_INITIALIZE as u64, true)?,
    )?;
    put(
        &mut prefix,
        setup.gate_offset,
        branch(gate, address + template::A64_BOOT_ZERO_GATE as u64, false)?,
    )?;
    put(&mut prefix, 0x10, setup.length_flags | copied_size as u32)?;
    require(
        extent(&program, template::SETUP_SD_PROGRESS, 64)?
            .iter()
            .all(|v| *v == 0)
            && word(&program, template::A64_BOOT_READY)? == 0,
        "XP5 handoff is already published",
    )?;
    Ok(vec![
        File::new("marble_init.bin", prefix)?,
        File::new("ffboot.bin", ram)?,
    ])
}
