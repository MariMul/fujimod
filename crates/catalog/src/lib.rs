//! Validated firmware, camera, and emulator catalog data.
pub mod application_inputs;
pub mod assembly;
pub mod bulk;
pub mod camera;
pub mod configuration;
pub mod controls;
pub mod emulator;
pub mod facts;
pub mod first_start;
pub mod persistent_inputs;
pub mod reference;
pub mod remote;
pub mod schema;
pub mod service;
pub mod soc;

pub use configuration::{ResolvedConfiguration, RunConfiguration, RunOverlay, RunOverlayChange};
pub use reference::{Body, Firmware, Part, ReferenceCatalog, ResolvedSelection, Soc};
pub use soc::SocHardware;

/// Reusable part assembly attachments.
pub mod parts;

/// Named hardware profiles and device graphs.
pub mod profiles;

/// Located firmware boot and storage layouts.
pub mod firmware;

/// Exported hardware construction profiles.
pub mod construction;

/// Supplemental source evidence and caller input identities.
pub mod provenance;

/// Explicit emulator rehearsal qualification records.
pub mod rehearsal;
