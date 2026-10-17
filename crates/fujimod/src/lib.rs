//! FujiMod libraries. Enable only the components your application needs.
#[cfg(feature = "usb")]
pub use fujimod_usb as usb;

#[cfg(feature = "defs")]
pub use fujimod_defs as defs;
