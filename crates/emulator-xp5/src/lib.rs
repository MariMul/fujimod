//! X-Processor 5 register-level peripherals, shared by execution engines.
pub mod adc;
pub mod board;
pub mod clocks;
pub mod lz;
pub mod display;
pub mod dma_registers;
pub mod flash_storage;
pub mod flash_storage_controller;
pub mod flash_storage_dma;
pub mod gates;
pub mod general_dma;
pub mod gpio;
pub mod i2c;
pub mod imaging;
pub mod input_capture;
pub mod mailbox;
pub mod osd;
pub mod output_link;
pub mod panel_clock;
pub mod panel_serial;
pub mod renderer;
pub mod serial_sink;
pub mod spi;
pub mod temperature;
pub mod timer;
#[cfg(test)]
mod tests;

