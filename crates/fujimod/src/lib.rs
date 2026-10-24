//! FujiMod libraries. Enable only the components your application needs.
#[cfg(feature = "usb")]
pub use fujimod_usb as usb;

#[cfg(feature = "app80")]
pub use fujimod_usb_app80 as usb_app80;

#[cfg(feature = "workflows")]
pub use fujimod_workflows as workflows;

#[cfg(feature = "catalog")]
pub use fujimod_catalog as catalog;

#[cfg(feature = "catalog-configuration")]
pub use fujimod_catalog_configuration as catalog_configuration;

#[cfg(feature = "catalog-camera")]
pub use fujimod_catalog_camera as catalog_camera;

#[cfg(feature = "catalog-service")]
pub use fujimod_catalog_service as catalog_service;

#[cfg(feature = "emulator")]
pub use fujimod_emulator as emulator;

#[cfg(feature = "emulator-devices")]
pub use fujimod_emulator_devices as emulator_devices;

#[cfg(feature = "emulator-cpu")]
pub use fujimod_emulator_cpu as emulator_cpu;

#[cfg(feature = "emulator-cpu-unicorn")]
pub use fujimod_emulator_cpu_unicorn as emulator_cpu_unicorn;

#[cfg(all(
    feature = "emulator-cpu-hvf",
    target_os = "macos",
    target_arch = "aarch64"
))]
pub use fujimod_emulator_cpu_hvf as emulator_cpu_hvf;

#[cfg(feature = "remote")]
pub use fujimod_remote as remote;
