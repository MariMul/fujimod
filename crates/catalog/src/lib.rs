//! Validated firmware, camera, and emulator catalog data.
pub mod camera;
pub mod configuration;
pub mod emulator;
pub mod facts;
pub mod reference;
pub mod schema;
pub mod service;

pub use configuration::{ResolvedConfiguration, RunConfiguration, RunOverlay, RunOverlayChange};
pub use reference::{Body, Firmware, Part, ReferenceCatalog, ResolvedSelection, Soc};
