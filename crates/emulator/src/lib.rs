//! Checked board execution, observation and session mechanisms.
pub mod architecture;
pub mod inputs;
pub mod board;
pub mod session;

pub const RUSTC_VERSION: &str = env!("CAMERA_NATIVE_RUSTC_VERSION");
pub const TARGET: &str = env!("CAMERA_NATIVE_TARGET");
