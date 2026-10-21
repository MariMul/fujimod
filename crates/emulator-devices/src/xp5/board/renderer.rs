//! XP5 renderer windows from the selected processor configuration.
use crate::graph::DeviceGraph;
use crate::{ip::dmp::smaph::Renderer, xp5::renderer::AddressHigh};
pub fn register(bus: &mut DeviceGraph, base: u64, size: u64) -> Result<(), String> {
    if base != 0xff93c000 || size != 0x3000 {
        return Err("DMP renderer requires ff93c000+3000".into());
    }
    bus.register(base, size, Box::new(Renderer::new(base, 153)?))?;
    bus.register(0xff90e73c, 4, Box::new(AddressHigh))?;
    Ok(())
}
