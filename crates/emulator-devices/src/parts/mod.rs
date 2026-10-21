//! External parts attached to SoC controllers, separate from controller law.

pub mod audio_sink;
pub mod hdmi_sink;
pub mod panel;
pub mod spi_registers;

pub mod bis_configuration;
pub mod lsm6ds;
pub mod power_controller;
pub mod resistor_keys;
pub mod serial;
pub mod serial_calibration;

pub mod controls;

pub mod encoder;
pub mod gpio;
pub mod gpio_contact_response;
pub mod leaf_shutter;
pub mod lens;
pub mod nand;
pub mod pins;
pub mod register_status;
pub mod sd_card;
pub mod sensor;
pub mod spi;
pub mod stabilizer;
pub mod touch_controller;
pub mod usb_endpoint;
pub mod wifi_module;
pub mod xtrans16;

pub mod analog;
pub mod command_data_output;
pub mod contact_actuator;
pub mod switch_panel;

pub mod x_mount_link;
pub mod x_mount_lens;
pub mod i2c;
