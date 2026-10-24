//! The reference catalog of SoCs, parts, bodies and firmware, and emulator run configurations.
pub mod application_inputs;
pub mod assembly;
pub mod configuration;
pub mod first_start;
pub mod progress;
pub mod reference;
#[cfg(feature = "schema")]
pub mod schema;
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

/// Supplemental source evidence and caller input identities.
pub mod provenance;

/// Explicit emulator rehearsal qualification records.
pub mod rehearsal;
