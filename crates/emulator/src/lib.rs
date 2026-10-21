//! Checked board execution, observation and session mechanisms.
pub mod architecture;
#[cfg(feature = "serde")]
pub mod boot;
pub mod inputs;
pub mod board;
pub mod session;

pub const RUSTC_VERSION: &str = env!("FUJIMOD_EMULATOR_RUSTC_VERSION");
pub const TARGET: &str = env!("FUJIMOD_EMULATOR_TARGET");
