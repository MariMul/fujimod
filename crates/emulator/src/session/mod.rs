//! Shared host session capabilities for native and browser clients.
//! Host observations never advance guest time or acknowledge device state.
pub mod card;
pub mod checkpoint;
pub mod contact_driver;
pub mod durable;
pub mod history;
pub mod inputs;
pub mod nand;
pub mod preparation;
pub mod scanout;
#[cfg(test)]
mod tests;
pub mod usb;
pub mod xp5;

use crate::board::Board;
use fujimod_emulator_exr2_storage::sdhc::Contacts;
use fujimod_emulator_exr2_plan::Resources;
use std::collections::BTreeMap;

pub struct Session {
    pub definition: serde_json::Value,
    pub board: Board,
    pub controls: Option<fujimod_emulator_parts_controls::switch_panel::SwitchPanel>,
    pub contact_driver: Option<contact_driver::Driver>,
    pub resources: Resources,
    /// XP5 external controls and GPIO inputs, when this board uses XP5 devices.
    pub xp5: Option<xp5::State>,
    pub nodes: BTreeMap<String, usize>,
    pub contacts: BTreeMap<String, Contacts>,
    /// Host rendering memo; excluded from checkpoints, fingerprints and modeled state.
    pub recording_frames: std::cell::RefCell<
        BTreeMap<
            (String, usize, bool),
            fujimod_recording::cache::CaptureCache<
                Option<std::sync::Arc<fujimod_emulator_exr2_display::scanout::Frame>>,
            >,
        >,
    >,
}
impl Session {
    pub fn fingerprint(&self) -> Result<serde_json::Value, String> {
        let mut value = serde_json::json!({"schema":1,"definition":self.definition,
            "controls":self.controls.as_ref().map(|p| serde_json::json!({"spec":p.spec(),"positions":p.positions()})),
            "contact_driver":self.contact_driver.as_ref().map(|d|d.fingerprint()).transpose()?,
            "xp5_controls":self.xp5.is_some()});
        value["board"] = self.board.fingerprint()?;
        value["resources"] = self.resources.fingerprint()?;
        Ok(value)
    }
    pub fn attach_xp5(mut self, state: xp5::State) -> Result<Self, String> {
        if self.xp5.is_some() {
            return Err("session already has XP5 controls".into());
        }
        self.xp5 = Some(state);
        Ok(self)
    }
    pub fn logic_input(&mut self, name: &str, high: bool) -> Result<(), String> {
        let level = self.resources.logic_level(name)?;
        let users = self.resources.wire_users(name)?;
        level.drive(high);
        for user in users {
            self.board.inputs_changed(user)?;
        }
        Ok(())
    }
    /// Change an explicitly wired HDMI sink input while processors are
    /// stopped, and deliver the corresponding HPD edge through the
    /// transmitter's native device context.
    pub fn hdmi_connection(
        &mut self,
        sink: &str,
        transmitter: &str,
        connected: bool,
    ) -> Result<(), String> {
        let sink_id = self.node(sink)?;
        let transmitter_id = self.node(transmitter)?;
        self.board
            .device_input::<fujimod_emulator_parts_io::hdmi_sink::Input, _>(
                sink_id,
                |input, _| {
                    input.set_connected(connected);
                    Ok(())
                },
            )?;
        self.board
            .device_input::<fujimod_emulator_exr2_io::hdmi::Transmitter, _>(
                transmitter_id,
                |device, cx| {
                    device.set_connected(cx, connected);
                    Ok(())
                },
            )
    }
    pub fn configure_frames(
        &mut self,
        name: &str,
        period: u64,
        cadence: fujimod_emulator_devices::scene::StatisticsCadence,
    ) -> Result<(), String> {
        let now = self.board.now();
        self.board
            .edit_device::<fujimod_emulator_devices::scene::clocked::ClockedFrames, _>(
                self.node(name)?,
                |frames| frames.configure(period, cadence, now),
            )
    }
    pub fn frame_state(&self, name: &str) -> Result<serde_json::Value, String> {
        use fujimod_emulator_devices::Device;
        self.board
            .inspect_device::<fujimod_emulator_devices::scene::clocked::ClockedFrames, _>(
                self.node(name)?,
                |frames| Ok(frames.save()),
            )
    }
    pub fn serial_state(&self, name: &str) -> Result<serde_json::Value, String> {
        use fujimod_emulator_devices::Device;
        self.board
            .inspect_device::<fujimod_emulator_exr2_serial::spi::SpiController, _>(
                self.node(name)?,
                |serial| Ok(serial.save()),
            )
    }
    pub fn serial_status_input(
        &mut self,
        name: &str,
        route: usize,
        register: u8,
        value: u8,
    ) -> Result<(), String> {
        self.board
            .edit_device::<fujimod_emulator_exr2_serial::spi::SpiController, _>(
                self.node(name)?,
                |serial| serial.status_input(route, register, value),
            )
    }
    pub fn node(&self, name: &str) -> Result<usize, String> {
        self.nodes
            .get(name)
            .copied()
            .ok_or_else(|| format!("no such session node {name}"))
    }
    pub fn gpio(&mut self, name: &str, port: u8, pin: u8, high: bool) -> Result<(), String> {
        self.resources.pads(name)?.request(
            fujimod_emulator_devices::parts::pins::Pin { port, pin },
            high,
        )?;
        self.board.inputs_changed(self.node(name)?)
    }
    pub fn gpio_input_levels(&self, name: &str) -> Result<[u32; 16], String> {
        Ok(self.resources.pads(name)?.input_levels())
    }
    pub fn gpio_level(&self, name: &str, port: u8, pin: u8) -> Result<bool, String> {
        self.resources
            .pads(name)?
            .input_level(fujimod_emulator_devices::parts::pins::Pin { port, pin })
    }
    pub fn gpio_pad_state(
        &self,
        name: &str,
        port: u8,
        pin: u8,
    ) -> Result<fujimod_emulator_devices::exr2::gpio::PadState, String> {
        self.resources
            .pads(name)?
            .pad_state(fujimod_emulator_devices::parts::pins::Pin { port, pin })
    }
    pub fn rotate(&mut self, name: &str, cycles: i32) -> Result<(), String> {
        self.resources
            .encoder(name)?
            .queue_cycles(self.board.now(), cycles)?;
        self.board.inputs_changed(self.node(name)?)
    }
    fn image_inputs_changed(&mut self, name: &str) -> Result<(), String> {
        for user in self.resources.image_users(name)? {
            self.board.inputs_changed(user)?;
        }
        Ok(())
    }
    pub fn rgb(
        &mut self,
        name: &str,
        width: usize,
        height: usize,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.resources
            .inputs
            .images
            .get(name)
            .ok_or("no such image input")?
            .prepare_rgb(width, height, bytes)?;
        self.image_inputs_changed(name)
    }
    pub fn analytic(&mut self, name: &str) -> Result<(), String> {
        self.resources
            .inputs
            .images
            .get(name)
            .ok_or("no such image input")?
            .use_analytic();
        self.image_inputs_changed(name)
    }
    pub fn sensor_image(&mut self, name: &str) -> Result<(), String> {
        self.resources
            .inputs
            .images
            .get(name)
            .ok_or("no such image input")?
            .use_sensor();
        self.image_inputs_changed(name)
    }
    pub fn sensor_rgb(
        &self,
        name: &str,
        width: usize,
        height: usize,
        bytes: &[u8],
        white_luminance: f64,
    ) -> Result<(), String> {
        use fujimod_emulator_devices::scene::{Frame, FramePixels};
        if !white_luminance.is_finite() || white_luminance <= 0. {
            return Err("scene white luminance must be positive finite cd/m²".into());
        }
        let input = self
            .resources
            .inputs
            .sensors
            .get(name)
            .ok_or("no such sensor input")?;
        if bytes.len() > 64 << 20 {
            return Err("sensor RGB input exceeds 64 MiB".into());
        }
        let mut frame = Frame::new(width, height, FramePixels::Rgb24(bytes.to_vec().into()))?;
        frame.white_luminance = white_luminance;
        input
            .lock()
            .map_err(|_| "sensor input is poisoned")?
            .set_frame(frame);
        Ok(())
    }
    /// Replace a sensor input with an external little-endian U16 RAW raster.
    /// The sensor's programmed window and transfer format still control DMA.
    pub fn sensor_raw16(
        &self,
        name: &str,
        width: usize,
        height: usize,
        bytes: &[u8],
    ) -> Result<(), String> {
        use fujimod_emulator_devices::scene::{Frame, FramePixels};
        let input = self
            .resources
            .inputs
            .sensors
            .get(name)
            .ok_or("no such sensor input")?;
        let expected = width
            .checked_mul(height)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or("sensor RAW input size overflow")?;
        if bytes.len() > 64 << 20 || bytes.len() != expected {
            return Err("sensor RAW input must contain exactly width * height U16 samples".into());
        }
        let samples = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let frame = Frame::new(width, height, FramePixels::Raw16(samples))?;
        input
            .lock()
            .map_err(|_| "sensor input is poisoned")?
            .set_frame(frame);
        Ok(())
    }
    /// Install a RAF's visible crop as mosaic radiance. Its recorded exposure
    /// is the normalization, so a later exposure scales the sensor counts.
    pub fn sensor_raf(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        use fujimod_emulator_devices::scene::Frame;
        self.sensor_frame(name, std::sync::Arc::new(Frame::from_raf_bytes(bytes)?))
    }
    /// Install shared immutable sensor pixels without retaining source file bytes.
    pub fn sensor_frame(
        &self,
        name: &str,
        frame: std::sync::Arc<fujimod_emulator_devices::scene::Frame>,
    ) -> Result<(), String> {
        self.resources
            .inputs
            .sensors
            .get(name)
            .ok_or("no such sensor input")?
            .lock()
            .map_err(|_| "sensor input is poisoned")?
            .current = frame;
        Ok(())
    }
}
