//! Processor-independent function families. Firmware policy and boundaries are inputs.
//! No firmware, disassembly, names or populated indexes are distributed with this crate.
mod decode;
mod candidates;
pub use candidates::FunctionCandidate;
mod inspect;
pub use inspect::*;
mod matching;
mod alignment;
pub use alignment::align_sequences;
mod quality;
pub use quality::NameSupport;
mod model;
pub use model::*;

#[cfg(test)]
mod tests;
