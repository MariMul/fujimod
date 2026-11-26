//! Thumb conditional execution state shared by ARM profiles.
pub mod exclusive;

/// Fetch a Thumb-2 instruction in architectural halfword order.
pub fn fetch(cpu: &dyn super::super::Cpu, address: u32) -> Result<(u32, u32), String> {
    let mut bytes = [0; 2];
    cpu.read_virtual(address.into(), super::super::Access::Fetch, &mut bytes)?;
    let first = u32::from(u16::from_le_bytes(bytes));
    if first < 0xe800 {
        return Ok((first, 2));
    }
    cpu.read_virtual(
        address.wrapping_add(2).into(),
        super::super::Access::Fetch,
        &mut bytes,
    )?;
    Ok(((first << 16) | u32::from(u16::from_le_bytes(bytes)), 4))
}

/// CPSR encodes ITSTATE in bits26:25 and15:10. Preserve flags and other state.
pub fn advance_it(cpsr: u32) -> u32 {
    let it = (((cpsr >> 25) & 3) | ((cpsr >> 8) & 0xfc)) as u8;
    let next = if it & 7 == 0 {
        0
    } else {
        (it & 0xe0) | ((it << 1) & 0x1f)
    };
    (cpsr & !0x0600_fc00) | ((next as u32 & 3) << 25) | ((next as u32 & 0xfc) << 8)
}
pub fn condition_passed(cpsr: u32) -> bool {
    let it = (((cpsr >> 25) & 3) | ((cpsr >> 8) & 0xfc)) as u8;
    if it == 0 {
        return true;
    }
    let (n, z, c, v) = (
        cpsr >> 31 != 0,
        cpsr & (1 << 30) != 0,
        cpsr & (1 << 29) != 0,
        cpsr & (1 << 28) != 0,
    );
    let cond = it >> 4;
    let yes = match cond >> 1 {
        0 => z,
        1 => c,
        2 => n,
        3 => v,
        4 => c && !z,
        5 => n == v,
        6 => n == v && !z,
        _ => true,
    };
    if cond & 1 != 0 && cond != 15 {
        !yes
    } else {
        yes
    }
}
