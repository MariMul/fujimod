//! Three-byte sensor writes (81/address/value) and external horizontal/vertical
//! scan clocks. Sensor configuration writes select gain, shutter registers,
//! and vertical timing. Gain is4095/(4095-code).
//! Timing/ADC responsivity remain declared physical calibration, not firmware
//! inspection or a caller signature. The sensor receives actual scan periods.
use fujimod_emulator_imaging::exposure::{Exposure, ExposureHandle};
use fujimod_emulator_devices::parts::optics::ApertureHandle;
use crate::sensor::Controls;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollingReference {
    pub mode: u8,
    pub horizontal: u32,
    pub vertical: u32,
    pub lines: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub pixel_clock_hz: u64,
    pub shutter: Option<crate::leaf_shutter::Options>,
    #[serde(default)]
    pub rolling_references: Vec<RollingReference>,
    /// Frames read out without valid sync after a drive-mode change. The
    /// receiver sees no frame edge for them.
    #[serde(default)]
    pub mode_change_frames: u8,
}
#[derive(Serialize, Deserialize)]
struct State {
    registers: Vec<u8>,
    horizontal: u32,
    vertical: u32,
    shutter_drive: Option<u64>,
    global_seconds: Option<f64>,
    /// Integration started for the following rolling readout. Analog gain is
    /// applied when that readout reaches the ADC, rather than at reset time.
    rolling_seconds: Option<f64>,
    /// Invalid frames still to come after the last drive-mode change.
    settling: u8,
}
pub struct Sensor {
    state: State,
    options: Options,
    exposure: ExposureHandle,
    aperture: ApertureHandle,
    shutter: Option<crate::leaf_shutter::LeafShutter>,
    frame_clock: Option<(fujimod_emulator_imaging::timing::FramePeriod, u64)>,
}
impl Sensor {
    pub fn new(
        options: Options,
        exposure: ExposureHandle,
        aperture: ApertureHandle,
    ) -> Result<Self, String> {
        if options.pixel_clock_hz == 0 {
            return Err("sensor needs a physical pixel clock".into());
        }
        Ok(Self {
            state: State {
                registers: vec![0; 256],
                horizontal: 0,
                vertical: 0,
                shutter_drive: None,
                global_seconds: None,
                rolling_seconds: None,
                settling: 0,
            },
            shutter: options
                .shutter
                .clone()
                .map(crate::leaf_shutter::LeafShutter::new)
                .transpose()?,
            options,
            frame_clock: None,
            exposure,
            aperture,
        })
    }
    fn publish(&mut self) -> Result<(), String> {
        if self.state.registers[1] & 4 != 0 && self.state.horizontal != 0 {
            let shutter = self
                .shutter
                .as_ref()
                .ok_or("global reset needs its physical leaf shutter")?;
            let shr =
                u32::from(self.state.registers[5]) | (u32::from(self.state.registers[6] & 15) << 8);
            // Configuring global reset before enabling its external shutter
            // waveform is valid. No exposed frame exists until both are ready.
            if let Some(drive) = self.state.shutter_drive {
                self.state.global_seconds = Some(
                    shutter.integration(
                        drive as f64 / self.options.pixel_clock_hz as f64,
                        f64::from(shr) * f64::from(self.state.horizontal)
                            / self.options.pixel_clock_hz as f64,
                        *self
                            .aperture
                            .lock()
                            .map_err(|_| "iris transmission poisoned")?,
                    )?,
                );
            }
        }
        let s = &self.state;
        let word = |n: usize| u32::from(s.registers[n]) | (u32::from(s.registers[n + 1] & 15) << 8);
        let gain = word(0x32);
        if gain >= 4095 {
            return Err("sensor AGC reaches its singular code4095".into());
        }
        // The input receiver's padded vertical raster can include a long
        // idle interval. Selected sensor drive modes declare their own reset
        // reference, independent of that receiver raster's padding. Matching
        // the entire scan tuple keeps shorter rasters on their actual counter;
        // reusing a padded-mode reference can falsely clamp SHR to one line.
        let reference = self
            .options
            .rolling_references
            .iter()
            .find(|r| {
                s.registers[0] & 0xfc == r.mode
                    && s.horizontal == r.horizontal
                    && s.vertical == r.vertical
            })
            .map_or(s.vertical, |r| r.lines);
        let extension = u64::from(word(2)) + 1;
        let exposure_rows = u64::from(reference) * extension;
        if let Some((output, tick_hz)) = &self.frame_clock {
            // Reset references bound integration, not the scan's blanking
            // interval. Frame edges follow the complete programmed raster.
            let frame_rows = u64::from(s.vertical) * extension;
            let clocks = u128::from(frame_rows) * u128::from(s.horizontal) * u128::from(*tick_hz);
            let period = (clocks + u128::from(self.options.pixel_clock_hz) / 2)
                / u128::from(self.options.pixel_clock_hz);
            *output.lock().map_err(|_| "sensor frame clock poisoned")? =
                u64::try_from(period).map_err(|_| "sensor frame clock exceeds guest timebase")?;
        }
        let integration = exposure_rows.saturating_sub(u64::from(word(5))).max(1);
        *self
            .exposure
            .lock()
            .map_err(|_| "sensor exposure poisoned")? = Exposure {
            seconds: if s.horizontal == 0
                || s.vertical == 0
                || (s.registers[1] & 4 != 0 && s.global_seconds.is_none())
            {
                0.
            } else {
                s.global_seconds.unwrap_or(
                    integration as f64 * s.horizontal as f64 / self.options.pixel_clock_hz as f64,
                )
            },
            analog_gain: 4095. / f64::from(4095 - gain),
        };
        Ok(())
    }
    pub fn attach_frame_clock(
        &mut self,
        output: fujimod_emulator_imaging::timing::FramePeriod,
        ticks_per_second: u64,
    ) -> Result<(), String> {
        if ticks_per_second == 0 {
            return Err("sensor timebase must be positive".into());
        }
        self.frame_clock = Some((output, ticks_per_second));
        self.publish()
    }
}
impl Sensor {
    fn integrate(&mut self) -> Result<(), String> {
        self.publish()?;
        if self.state.registers[1] & 4 != 0 || self.state.global_seconds.is_some() {
            // Global reset integrates until the physical leaf closes; it does
            // not inherit the previous rolling frame's reset interval.
            self.state.rolling_seconds = None;
            return Ok(());
        }
        let mut readout = self
            .exposure
            .lock()
            .map_err(|_| "sensor exposure poisoned")?;
        if readout.seconds <= 0. {
            self.state.rolling_seconds = None;
            return Ok(());
        }
        let next = readout.seconds;
        if let Some(integrated) = self.state.rolling_seconds {
            readout.seconds = integrated;
        }
        self.state.rolling_seconds = Some(next);
        Ok(())
    }
}
impl Controls for Sensor {
    fn checkpoint(&self) -> Result<fujimod_emulator_devices::checkpoint::Checkpoint, String> {
        fujimod_emulator_devices::checkpoint::Checkpoint::encode(
            "xtrans16-controls",
            &(
                &self.options,
                &self.state,
                self.frame_clock.as_ref().map(|(_, ticks)| *ticks),
            ),
        )
    }
    fn validate_checkpoint(&self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        let (options, state, timebase): (Options, State, Option<u64>) =
            saved.decode("xtrans16-controls")?;
        if timebase != self.frame_clock.as_ref().map(|(_, ticks)| *ticks)
            || options != self.options
            || state.registers.len() != 256
            || state
                .global_seconds
                .is_some_and(|v| !v.is_finite() || v <= 0.)
            || state
                .rolling_seconds
                .is_some_and(|v| !v.is_finite() || v <= 0.)
            || state.settling > options.mode_change_frames
            || !saved.blobs().is_empty()
            || !saved.children().is_empty()
        {
            return Err(
                "sensor controller checkpoint configuration or register extent differs".into(),
            );
        }
        Ok(())
    }
    fn restore_checkpoint(&mut self, saved: &fujimod_emulator_devices::checkpoint::Checkpoint) -> Result<(), String> {
        self.validate_checkpoint(saved)?;
        let (_, state, _): (Options, State, Option<u64>) = saved.decode("xtrans16-controls")?;
        self.state = state;
        // Shared physical exposure/aperture wires restore independently in place.
        Ok(())
    }
    fn fingerprint(&self) -> serde_json::Value {
        serde_json::json!({"options":self.options,"state":self.state,"frame_clock":self.frame_clock.as_ref().map(|(_,ticks)|*ticks)})
    }

    fn command(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() % 8 != 0
            || bytes
                .chunks_exact(8)
                .any(|b| b[0] != 0x81 || b[3..].iter().any(|&v| v != 0))
        {
            return Err("X-Trans sensor requires81/address/value words".into());
        }
        for b in bytes.chunks_exact(8) {
            if b[1] == 0 && b[2] & 0xfc != self.state.registers[0] & 0xfc {
                self.state.global_seconds = None;
                self.state.settling = self.options.mode_change_frames;
            }
            if b[1] == 1 && b[2] & 4 != 0 && self.state.registers[1] & 4 == 0 {
                self.state.global_seconds = None;
            }
            self.state.registers[b[1] as usize] = b[2];
        }
        self.publish()
    }
    fn frame(&mut self) -> Result<bool, String> {
        let valid = self.state.settling == 0;
        self.state.settling = self.state.settling.saturating_sub(1);
        self.integrate()?;
        Ok(valid)
    }
    fn timing(
        &mut self,
        horizontal: u32,
        vertical: u32,
        shutter_drive: Option<u64>,
    ) -> Result<(), String> {
        self.state.shutter_drive = shutter_drive;
        self.state.horizontal = horizontal;
        self.state.vertical = vertical;
        self.publish()
    }
    fn save(&self) -> serde_json::Value {
        serde_json::to_value(&self.state).unwrap()
    }
    fn restore(&mut self, state: &serde_json::Value) -> Result<(), String> {
        let state: State = serde_json::from_value(state.clone()).map_err(|e| e.to_string())?;
        if state.registers.len() != 256 {
            return Err("sensor register extent changed".into());
        }
        self.state = state;
        self.publish()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_readout_uses_prior_integration_and_current_adc_gain() {
        let wire: ExposureHandle = Default::default();
        let mut sensor = Sensor::new(
            Options {
                pixel_clock_hz: 33_000_000,
                shutter: None,
                rolling_references: vec![],
                mode_change_frames: 0,
            },
            wire.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(8.)),
        )
        .unwrap();
        sensor.timing(660, 1000, None).unwrap();
        sensor.frame().unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.02);
        sensor
            .command(&[
                0x81, 5, 244, 0, 0, 0, 0, 0, 0x81, 6, 1, 0, 0, 0, 0, 0, 0x81, 0x32, 0, 0, 0, 0, 0,
                0, 0x81, 0x33, 8, 0, 0, 0, 0, 0,
            ])
            .unwrap();
        sensor.frame().unwrap();
        let readout = *wire.lock().unwrap();
        assert_eq!(readout.seconds, 0.02);
        assert!((readout.analog_gain - 2.).abs() < 0.001);
        sensor
            .command(&[0x81, 5, 32, 0, 0, 0, 0, 0, 0x81, 6, 3, 0, 0, 0, 0, 0])
            .unwrap();
        // The snapshot must retain .01 even though the newest SHR configures
        // .004. Reconstructing pending integration from the registers loses it.
        let checkpoint = sensor.checkpoint().unwrap();
        let fingerprint = sensor.fingerprint();
        assert_eq!(sensor.fingerprint(), fingerprint);
        sensor.save();
        assert_eq!(sensor.fingerprint(), fingerprint);
        sensor.frame().unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.01);
        sensor.frame().unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.004);
        sensor.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(sensor.fingerprint(), fingerprint);
        sensor.frame().unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.01);
        sensor.frame().unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.004);
    }

    #[test]
    fn physical_scan_clock_preserves_vertical_blanking_and_raster_extension() {
        let period: fujimod_emulator_imaging::timing::FramePeriod = Default::default();
        let exposure: ExposureHandle = Default::default();
        let mut s = Sensor::new(
            Options {
                pixel_clock_hz: 33_000_000,
                shutter: None,
                rolling_references: vec![RollingReference {
                    mode: 0xec,
                    horizontal: 706,
                    vertical: 9348,
                    lines: 856,
                }],
                mode_change_frames: 0,
            },
            exposure.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(8.)),
        )
        .unwrap();
        s.attach_frame_clock(period.clone(), 12_000_000).unwrap();
        assert_eq!(*period.lock().unwrap(), 0);
        s.timing(734, 450, None).unwrap();
        assert_eq!(*period.lock().unwrap(), 120109);
        let saved = s.checkpoint().unwrap();
        let saved_period = *period.lock().unwrap();
        s.command(&[0x81, 2, 1, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(*period.lock().unwrap(), 240218);
        // Exposure SHR and analog gain do not alter the frame period.
        s.command(&[0x81, 5, 100, 0, 0, 0, 0, 0, 0x81, 0x32, 128, 0, 0, 0, 0, 0])
            .unwrap();
        assert_eq!(*period.lock().unwrap(), 240218);
        s.restore_checkpoint(&saved).unwrap();
        *period.lock().unwrap() = saved_period;
        assert_eq!(*period.lock().unwrap(), 120109);
        s.timing(706, 9348, None).unwrap();
        let scan_period = *period.lock().unwrap();
        s.command(&[0x81, 0, 0xec, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(*period.lock().unwrap(), 2_399_887);
        assert_eq!(*period.lock().unwrap(), scan_period);
        assert!((exposure.lock().unwrap().seconds - 856. * 706. / 33_000_000.).abs() < 1e-12);
        // A reset reference belongs to the complete declared scan tuple.
        // Another raster retains its own counter extent and frame cadence.
        s.timing(706, 935, None).unwrap();
        s.command(&[0x81, 5, 0x9e, 0, 0, 0, 0, 0, 0x81, 6, 3, 0, 0, 0, 0, 0])
            .unwrap();
        assert_eq!(*period.lock().unwrap(), 240040);
        assert!((exposure.lock().unwrap().seconds - 9. * 706. / 33_000_000.).abs() < 1e-12);
        s.command(&[0x81, 2, 1, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(*period.lock().unwrap(), 480080);
        assert!((exposure.lock().unwrap().seconds - 944. * 706. / 33_000_000.).abs() < 1e-12);
    }
    #[test]
    fn register_shutter_gain_and_restore_change_physical_exposure() {
        let wire: ExposureHandle = Default::default();
        let mut s = Sensor::new(
            Options {
                pixel_clock_hz: 33000000,
                shutter: None,
                rolling_references: vec![],
                mode_change_frames: 0,
            },
            wire.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(8.)),
        )
        .unwrap();
        s.timing(660, 1000, None).unwrap();
        let frame = wire.lock().unwrap().seconds;
        assert_eq!(frame, 0.02);
        s.command(&[0x81, 5, 244, 0, 0, 0, 0, 0, 0x81, 6, 1, 0, 0, 0, 0, 0])
            .unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.01);
        s.command(&[0x81, 0x32, 0, 0, 0, 0, 0, 0, 0x81, 0x33, 8, 0, 0, 0, 0, 0])
            .unwrap();
        assert!((wire.lock().unwrap().analog_gain - 2.).abs() < 0.001);
        let saved = s.save();
        s.timing(330, 1000, None).unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.005);
        s.restore(&saved).unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.01);
    }
    #[test]
    fn reset_reference_and_leaf_closure_survive_the_readout_phase() {
        let wire: ExposureHandle = Default::default();
        let mut s = Sensor::new(
            Options {
                pixel_clock_hz: 33_000_000,
                shutter: Some(crate::leaf_shutter::Options {
                    closing_delay_seconds: vec![(2., 0.003), (16., 0.003)],
                }),
                rolling_references: vec![RollingReference {
                    mode: 0xec,
                    horizontal: 660,
                    vertical: 9000,
                    lines: 1000,
                }],
                mode_change_frames: 0,
            },
            wire.clone(),
            std::sync::Arc::new(std::sync::Mutex::new(8.)),
        )
        .unwrap();
        let command = |pairs: &[(u8, u8)]| {
            pairs
                .iter()
                .flat_map(|&(r, v)| [0x81, r, v, 0, 0, 0, 0, 0])
                .collect::<Vec<_>>()
        };
        s.timing(660, 9000, Some(6600)).unwrap();
        s.command(&command(&[(0, 0xec), (5, 244), (6, 1)])).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.01).abs() < 1e-12);
        s.timing(660, 1000, None).unwrap();
        s.command(&command(&[(0, 0x2d), (1, 4), (5, 100), (6, 0)]))
            .unwrap();
        assert_eq!(wire.lock().unwrap().seconds, 0.);
        s.timing(660, 1000, Some(6600)).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.0012).abs() < 1e-12);
        s.timing(660, 1000, None).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.0012).abs() < 1e-12);
        s.command(&command(&[(1, 0), (0, 0x2c)])).unwrap();
        s.timing(660, 3374, None).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.0012).abs() < 1e-12);
        let saved = s.save();
        s.command(&command(&[(0, 0xec)])).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.06548).abs() < 1e-12);
        s.restore(&saved).unwrap();
        assert!((wire.lock().unwrap().seconds - 0.0012).abs() < 1e-12);
    }

    #[test]
    fn drive_mode_change_withholds_valid_frames() {
        let wire: ExposureHandle = Default::default();
        let mut sensor = Sensor::new(
            Options {
                pixel_clock_hz: 33_000_000,
                shutter: None,
                rolling_references: vec![],
                mode_change_frames: 2,
            },
            wire,
            std::sync::Arc::new(std::sync::Mutex::new(8.)),
        )
        .unwrap();
        sensor.timing(660, 1000, None).unwrap();
        let mode = |value| [0x81, 0, value, 0, 0, 0, 0, 0];
        sensor.command(&mode(0xec)).unwrap();
        let frames: Vec<bool> = (0..3).map(|_| sensor.frame().unwrap()).collect();
        assert_eq!(frames, [false, false, true]);
        // Low register-0 bits are not the drive mode.
        sensor.command(&mode(0xed)).unwrap();
        assert!(sensor.frame().unwrap());
        sensor.command(&mode(0x28)).unwrap();
        let frames: Vec<bool> = (0..3).map(|_| sensor.frame().unwrap()).collect();
        assert_eq!(frames, [false, false, true]);
    }
}
