//! Camera-independent USB forwarding. No Fujifilm framing, firmware or policy.
#![forbid(unsafe_code)]
mod client;
mod latency;
pub mod protocol;
pub use client::{Client, Progress, RemoteDevice};
pub use protocol::{Completion, Device, Endpoint, Interface, Operation, Setup};

#[derive(Debug, thiserror::Error)]
#[error("USB proxy: {0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(all(feature = "relay", not(target_arch = "wasm32")))]
pub mod relay;
#[cfg(all(feature = "relay", unix))]
pub mod dev_shares;

mod peer;
pub mod share;
#[cfg(all(feature = "relay", not(target_arch = "wasm32")))]
mod signaling;
