//! Processor-independent function families. Firmware policy and boundaries are inputs.
//! No firmware, disassembly, names or populated indexes are distributed with this crate.
mod decode;
mod matching;
mod model;
pub use model::*;

#[cfg(test)]
mod tests;
