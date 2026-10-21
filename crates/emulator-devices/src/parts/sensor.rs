//! Image sensor part supplied by external RGB/RAW frames. CFA, dimensions,
//! black level, active image area and reference white are body data. The RGB spectral response
//! is a declared linear sRGB approximation, not measured sensor calibration.
//! RGB light maps to the active image area; explicit RAW spans the clock raster.
use crate::imaging::exposure::{ApertureHandle, Calibration, ExposedFrame, ExposureHandle};
use crate::imaging::raw::{Cfa, Packing};
use crate::scene::{FrameEdge, FramePixels, SensorFramesHandle};
use serde::{Deserialize, Serialize};
pub trait Controls: Send {
    fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String>;
    fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String>;
    fn restore_checkpoint(&mut self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String>;
    fn fingerprint(&self) -> serde_json::Value;

    fn command(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn timing(
        &mut self,
        horizontal: u32,
        vertical: u32,
        shutter_drive: Option<u64>,
    ) -> Result<(), String>;
    fn save(&self) -> serde_json::Value;
    fn restore(&mut self, state: &serde_json::Value) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imaging::exposure::Exposure;
    use crate::scene::{Frame, SensorFrames};
    use std::sync::{Arc, Mutex};

    #[test]
    fn cropped_samples_keep_full_raster_projection_and_cfa_phase() {
        use crate::imaging::raw::RawView;
        let frames = Arc::new(Mutex::new(SensorFrames::new(
            Frame::new(
                8,
                4,
                FramePixels::Rgb24((0..32).flat_map(|i| [32 + i, 96 + i, 192 + i]).collect()),
            )
            .unwrap(),
        )));
        let mut sensor = Sensor::new(
            Options {
                width: 8,
                height: 4,
                image_area: [0, 0, 8, 4],
                cfa: (0..36).map(|i| ((i / 6 + i % 6) % 3) as u8).collect(),
                black: 0,
                full_scale: 4095,
                reference_white: [256; 3],
            },
            frames,
        )
        .unwrap();
        sensor.latch(10).unwrap();
        let mut full = vec![0; 8 * 4 * 2];
        sensor
            .encode(Packing::U16, [0, 0, 8, 4], &mut full)
            .unwrap();
        let mut crop = vec![0; 4 * 2 * 3 / 2];
        sensor
            .encode(Packing::Packed12, [1, 1, 4, 2], &mut crop)
            .unwrap();
        let full = RawView {
            bytes: &full,
            width: 8,
            height: 4,
            pitch: 16,
            packing: Packing::U16,
        };
        let crop = RawView {
            bytes: &crop,
            width: 4,
            height: 2,
            pitch: 6,
            packing: Packing::Packed12,
        };
        for y in 0..2 {
            for x in 0..4 {
                assert_eq!(crop.sample(x, y), full.sample(x + 1, y + 1));
            }
        }
    }

    #[test]
    fn raw_keeps_sampled_exposure_and_focus_across_new_commands_and_restore() {
        let frames = Arc::new(Mutex::new(SensorFrames::new(
            Frame::new(
                64,
                8,
                FramePixels::Rgb24(
                    (0..64 * 8)
                        .flat_map(|i| [if i % 2 == 0 { 0 } else { 255 }; 3])
                        .collect::<Vec<_>>()
                        .into(),
                ),
            )
            .unwrap(),
        )));
        let wire = Arc::new(Mutex::new(Exposure::default()));
        let iris = Arc::new(Mutex::new(8.));
        let mut sensor = Sensor::new(
            Options {
                width: 6,
                height: 6,
                image_area: [0, 0, 6, 6],
                cfa: vec![0, 1, 2].repeat(12),
                black: 0,
                full_scale: 16380,
                reference_white: [256; 3],
            },
            frames.clone(),
        )
        .unwrap();
        sensor
            .attach_controls(
                Box::new(
                    crate::parts::xtrans16::Sensor::new(
                        crate::parts::xtrans16::Options {
                            pixel_clock_hz: 33_000_000,
                            shutter: None,
                            rolling_references: vec![],
                        },
                        wire.clone(),
                        iris.clone(),
                    )
                    .unwrap(),
                ),
                Calibration {
                    full_scale: 16380,
                    reference_white: [256; 3],
                    saturation_lux_seconds: 1.,
                    color_sample_weights: [2, 5, 2],
                },
                wire.clone(),
                iris,
            )
            .unwrap();
        let focus = Arc::new(Mutex::new(0.));
        sensor.attach_projection(
            crate::imaging::optics::Projection::new(
                crate::imaging::optics::Spec {
                    focus: "lens".into(),
                    focal_length_mm: 18.5,
                    subject_distance_mm: 2000.,
                    sensor_width_mm: 1.,
                    relative_illumination: crate::imaging::optics::Illumination::Uniform,
                },
                focus.clone(),
            )
            .unwrap(),
        );
        sensor.timing(330, 100, None).unwrap();
        sensor.latch(1).unwrap();
        // Latching captures focus; raster work is deferred until RAW DMA.
        *focus.lock().unwrap() = 18.5 * 18.5 / (2000. - 18.5);
        let mut first = vec![0; 72];
        sensor
            .encode(Packing::U16, [0, 0, 6, 6], &mut first)
            .unwrap();
        let saved = sensor.checkpoint().unwrap();
        let saved_frames = frames.lock().unwrap().clone();
        let saved_exposure = *wire.lock().unwrap();
        let before = sensor.fingerprint();
        let corrupt = saved.clone().with_children(vec![]);
        assert!(sensor.restore_checkpoint(&corrupt).is_err());
        assert_eq!(sensor.fingerprint(), before);
        assert_eq!(*wire.lock().unwrap(), saved_exposure);
        sensor.command(&[0x81, 5, 50, 0, 0, 0, 0, 0]).unwrap();
        frames
            .lock()
            .unwrap()
            .set_frame(Frame::new(1, 1, FramePixels::Rgb24(vec![255; 3].into())).unwrap());
        let mut actual = vec![0; 72];
        sensor
            .encode(Packing::U16, [0, 0, 6, 6], &mut actual)
            .unwrap();
        assert_eq!(actual, first);
        sensor.latch(2).unwrap();
        sensor
            .encode(Packing::U16, [0, 0, 6, 6], &mut actual)
            .unwrap();
        assert_ne!(actual, first);
        let changed_exposure = *wire.lock().unwrap();
        sensor.restore_checkpoint(&saved).unwrap();
        assert_eq!(*wire.lock().unwrap(), changed_exposure);
        // The graph owns peer wire and scene values; restore them in place.
        *wire.lock().unwrap() = saved_exposure;
        *frames.lock().unwrap() = saved_frames;
        assert_eq!(sensor.fingerprint(), before);
        sensor
            .encode(Packing::U16, [0, 0, 6, 6], &mut actual)
            .unwrap();
        assert_eq!(actual, first);
    }

    #[test]
    fn rgb_light_fills_only_the_image_area_while_raw_keeps_clock_coordinates() {
        let frames = Arc::new(Mutex::new(SensorFrames::new(
            Frame::new(
                2,
                2,
                FramePixels::Rgb24(vec![0, 0, 0, 85, 85, 85, 170, 170, 170, 255, 255, 255].into()),
            )
            .unwrap(),
        )));
        let options = |area| Options {
            width: 8,
            height: 6,
            image_area: area,
            cfa: vec![0, 1, 2].repeat(12),
            black: 7,
            full_scale: 4095,
            reference_white: [256; 3],
        };
        for area in [
            [0, 0, 0, 4],
            [7, 0, 2, 1],
            [0, 5, 1, 2],
            [usize::MAX, 0, 2, 1],
        ] {
            assert!(Sensor::new(options(area), frames.clone()).is_err());
        }
        let mut sensor = Sensor::new(options([2, 1, 4, 4]), frames.clone()).unwrap();
        let mut bytes = vec![0; 8 * 6 * 2];
        sensor
            .encode(Packing::U16, [0, 0, 8, 6], &mut bytes)
            .unwrap();
        for y in 0..6 {
            for x in 0..8 {
                let expected = if (2..6).contains(&x) && (1..5).contains(&y) {
                    let level = [0, 85, 170, 255][((y - 1) / 2) * 2 + (x - 2) / 2];
                    7 + sensor.linear[0][level]
                } else {
                    7
                };
                let i = (y * 8 + x) * 2;
                assert_eq!(
                    u16::from_le_bytes([bytes[i], bytes[i + 1]]),
                    expected,
                    "{x},{y}"
                );
            }
        }
        let saved = sensor.checkpoint().unwrap();
        let different = Sensor::new(options([0, 0, 8, 6]), frames.clone()).unwrap();
        assert!(different.validate_checkpoint(&saved).is_err());
        frames
            .lock()
            .unwrap()
            .set_frame(Frame::new(8, 6, FramePixels::Raw16((0..48).collect())).unwrap());
        sensor.latch(1).unwrap();
        sensor
            .encode(Packing::U16, [0, 0, 8, 6], &mut bytes)
            .unwrap();
        assert_eq!(
            bytes,
            (0u16..48).flat_map(u16::to_le_bytes).collect::<Vec<_>>()
        );
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub width: usize,
    pub height: usize,
    pub image_area: [usize; 4],
    pub cfa: Vec<u8>,
    pub black: u16,
    pub full_scale: u16,
    pub reference_white: [u16; 3],
}
#[derive(PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    width: usize,
    height: usize,
    image_area: [usize; 4],
    cfa: Cfa,
    black: u16,
    linear: Vec<u16>,
    radiometry: Option<Calibration>,
    projection: Option<crate::imaging::optics::Spec>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    configuration: Configuration,
    controls: bool,
    exposed: bool,
}
pub struct Sensor {
    pub width: usize,
    pub height: usize,
    pub image_area: [usize; 4],
    pub frames: SensorFramesHandle,
    cfa: Cfa,
    black: u16,
    linear: [[u16; 256]; 3],
    controls: Option<Box<dyn Controls>>,
    radiometry: Option<(Calibration, ExposureHandle, ApertureHandle)>,
    exposed: Option<ExposedFrame>,
    projection: Option<crate::imaging::optics::Projection>,
}
impl Sensor {
    pub fn new(options: Options, frames: SensorFramesHandle) -> Result<Self, String> {
        if options.width < 2
            || !options.width.is_multiple_of(2)
            || options.height == 0
            || options.width > 8192
            || options.height > 8192
            || options.width * options.height > 32_000_000
            || options.reference_white.contains(&0)
            || options.full_scale == 0
        {
            return Err("sensor geometry/response is invalid".into());
        }
        let [left, top, width, height] = options.image_area;
        if width == 0
            || height == 0
            || left
                .checked_add(width)
                .is_none_or(|end| end > options.width)
            || top
                .checked_add(height)
                .is_none_or(|end| end > options.height)
        {
            return Err("sensor image area exceeds physical raster".into());
        }
        let linear = std::array::from_fn(|c| {
            std::array::from_fn(|i| {
                let s = i as f64 / 255.;
                let v = if s <= 0.04045 {
                    s / 12.92
                } else {
                    libm::pow((s + 0.055) / 1.055, 2.4)
                };
                libm::rint(v * options.full_scale as f64 * 256. / options.reference_white[c] as f64)
                    .clamp(0., 65535. - options.black as f64) as u16
            })
        });
        Ok(Self {
            width: options.width,
            height: options.height,
            image_area: options.image_area,
            frames,
            cfa: Cfa::new(options.cfa)?,
            black: options.black,
            linear,
            controls: None,
            radiometry: None,
            exposed: None,
            projection: None,
        })
    }
    pub fn attach_controls(
        &mut self,
        controls: Box<dyn Controls>,
        calibration: Calibration,
        exposure: ExposureHandle,
        aperture: ApertureHandle,
    ) -> Result<(), String> {
        self.radiometry = Some((calibration.validate()?, exposure, aperture));
        self.controls = Some(controls);
        Ok(())
    }
    pub fn attach_projection(&mut self, projection: crate::imaging::optics::Projection) {
        self.projection = Some(projection);
    }
    pub fn command(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.controls.as_mut().map_or(Ok(()), |c| c.command(bytes))
    }
    pub fn timing(
        &mut self,
        horizontal: u32,
        vertical: u32,
        shutter_drive: Option<u64>,
    ) -> Result<(), String> {
        self.controls
            .as_mut()
            .map_or(Ok(()), |c| c.timing(horizontal, vertical, shutter_drive))
    }
    fn configuration(&self) -> Configuration {
        Configuration {
            width: self.width,
            height: self.height,
            image_area: self.image_area,
            cfa: self.cfa.clone(),
            black: self.black,
            linear: self.linear.iter().flatten().copied().collect(),
            radiometry: self.radiometry.as_ref().map(|r| r.0),
            projection: self.projection.as_ref().map(|p| p.spec().clone()),
        }
    }
    pub(crate) fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        let mut children = Vec::new();
        if let Some(controls) = &self.controls {
            children.push(controls.checkpoint()?);
        }
        if let Some(exposed) = &self.exposed {
            children.push(exposed.checkpoint()?);
        }
        Ok(crate::checkpoint::Checkpoint::encode(
            "image-sensor",
            &Snapshot {
                configuration: self.configuration(),
                controls: self.controls.is_some(),
                exposed: self.exposed.is_some(),
            },
        )?
        .with_children(children))
    }
    pub(crate) fn validate_checkpoint(
        &self,
        saved: &crate::checkpoint::Checkpoint,
    ) -> Result<(), String> {
        let Snapshot {
            configuration,
            controls,
            exposed,
        } = saved.decode("image-sensor")?;
        if configuration != self.configuration()
            || controls != self.controls.is_some()
            || exposed && self.radiometry.is_none()
            || saved.children().len() != usize::from(controls) + usize::from(exposed)
            || !saved.blobs().is_empty()
        {
            return Err("image sensor checkpoint wiring or extent differs".into());
        }
        if let Some(controls) = &self.controls {
            controls.validate_checkpoint(&saved.children()[0])?;
        }
        if exposed {
            ExposedFrame::from_checkpoint(&saved.children()[usize::from(controls)])?;
        }
        Ok(())
    }
    pub(crate) fn restore_checkpoint(
        &mut self,
        saved: &crate::checkpoint::Checkpoint,
    ) -> Result<(), String> {
        self.validate_checkpoint(saved)?;
        let Snapshot {
            controls, exposed, ..
        } = saved.decode("image-sensor")?;
        let frame = if exposed {
            Some(ExposedFrame::from_checkpoint(
                &saved.children()[usize::from(controls)],
            )?)
        } else {
            None
        };
        if let Some(controls) = &mut self.controls {
            controls.restore_checkpoint(&saved.children()[0])?;
        }
        self.exposed = frame;
        Ok(())
    }
    pub(crate) fn fingerprint(&self) -> serde_json::Value {
        let linear: Vec<u16> = self.linear.iter().flatten().copied().collect();
        serde_json::json!({"width":self.width,"height":self.height,"image_area":self.image_area,"cfa":self.cfa,"black":self.black,
            "linear":crate::fingerprint::words16(&linear),"radiometry":self.radiometry.as_ref().map(|r|r.0),"frames":self.frames.lock().unwrap().fingerprint(),
            "projection":self.projection.as_ref().map(|p|p.spec()),"controls":self.controls.as_ref().map(|c|c.fingerprint()),"exposed":self.exposed.as_ref().map(|e|e.fingerprint())})
    }
    pub fn controls(&self) -> serde_json::Value {
        serde_json::json!({"controller":self.controls.as_ref().map(|c|c.save()),
            "latched":self.exposed.as_ref().map(|e|(e.exposure,e.aperture_squared,e.focus_shift_mm))})
    }
    pub fn restore_controls(&mut self, state: &serde_json::Value) -> Result<(), String> {
        if let Some(c) = &mut self.controls {
            c.restore(state.get("controller").ok_or("missing sensor controller")?)?;
        }
        self.exposed = if let Some(sample) = state.get("latched").filter(|v| !v.is_null()) {
            let (exposure, aperture, focus): (_, _, Option<f64>) =
                serde_json::from_value(sample.clone()).map_err(|e| e.to_string())?;
            let calibration = self
                .radiometry
                .as_ref()
                .ok_or("unexpected radiometric exposure")?
                .0;
            let mut frame = ExposedFrame::from_frame(
                self.frames.lock().unwrap().latched.clone(),
                exposure,
                aperture,
                calibration,
            )?;
            if let Some(focus) = focus {
                if self.projection.is_none() {
                    return Err("sampled focus has no optical projection".into());
                }
                frame.focus_shift_mm = Some(focus);
            }
            Some(frame)
        } else {
            None
        };
        Ok(())
    }
    pub fn physical_exposure(&self) -> Option<serde_json::Value> {
        self.exposed.as_ref().map(|e| {
            let mut state = e.physical_state();
            state["controller"] = self.controls.as_ref().map(|c| c.save()).unwrap_or_default();
            state
        })
    }
    pub fn latch(&mut self, tick: u64) -> Result<FrameEdge, String> {
        if let Some((calibration, exposure, aperture)) = &self.radiometry {
            self.exposed = ExposedFrame::capture(&self.frames, exposure, aperture, *calibration)?;
            if let (Some(projection), Some(frame)) = (&self.projection, &mut self.exposed) {
                frame.focus_shift_mm = Some(projection.focus()?);
            }
        }
        self.frames.lock().unwrap().latch(tick)
    }
    pub fn encode(
        &mut self,
        packing: Packing,
        [left, top, width, height]: [usize; 4],
        out: &mut [u8],
    ) -> Result<(), String> {
        if width == 0
            || width % 2 != 0
            || height == 0
            || left.checked_add(width).is_none_or(|v| v > self.width)
            || top.checked_add(height).is_none_or(|v| v > self.height)
        {
            return Err("sensor output window exceeds physical raster".into());
        }
        if let (Some(projection), Some(frame)) = (&self.projection, &mut self.exposed) {
            let [image_left, image_top, image_width, image_height] = self.image_area;
            let x0 = left.clamp(image_left, image_left + image_width);
            let x1 = (left + width).clamp(image_left, image_left + image_width);
            let y0 = top.clamp(image_top, image_top + image_height);
            let y1 = (top + height).clamp(image_top, image_top + image_height);
            projection.prepare_readout(
                frame,
                frame
                    .focus_shift_mm
                    .ok_or("RAW exposure has no sampled optical focus")?,
                [image_width, image_height],
                [x0 - image_left, y0 - image_top, x1 - x0, y1 - y0],
            )?;
        }
        if self.radiometry.is_some() && self.exposed.is_none() {
            return Err("RAW capture has no completed physical exposure".into());
        }
        let frames = self.frames.lock().unwrap();
        let f = &frames.latched;
        if out.len() != packing.row_bytes(width) * height {
            return Err("sensor output buffer size".into());
        }
        let sample = |x: usize, y: usize| {
            let sx = ((2 * x + 1) * f.width / (2 * self.width)).min(f.width - 1);
            let sy = ((2 * y + 1) * f.height / (2 * self.height)).min(f.height - 1);
            match &f.pixels {
                FramePixels::Raw16(p) => p[sy * f.width + sx],
                FramePixels::Rgb24(p) => {
                    let [left, top, width, height] = self.image_area;
                    if x < left || x - left >= width || y < top || y - top >= height {
                        return self.black;
                    }
                    let (image_x, image_y) = (x - left, y - top);
                    let sx = ((2 * image_x + 1) * f.width / (2 * width)).min(f.width - 1);
                    let sy = ((2 * image_y + 1) * f.height / (2 * height)).min(f.height - 1);
                    let c = self.cfa.color(x, y);
                    if self.radiometry.is_some() {
                        return self.black.saturating_add(
                            self.exposed
                                .as_ref()
                                .map_or(0, |e| e.sample(image_x, image_y, [width, height])[c]),
                        );
                    }
                    self.black
                        .saturating_add(self.linear[c][p[(sy * f.width + sx) * 3 + c] as usize])
                }
            }
        };
        for y in 0..height {
            for x in (0..width).step_by(2) {
                let a = sample(left + x, top + y);
                let b = sample(left + x + 1, top + y);
                match packing {
                    Packing::U16 => {
                        let i = (y * width + x) * 2;
                        out[i..i + 2].copy_from_slice(&a.to_le_bytes());
                        out[i + 2..i + 4].copy_from_slice(&b.to_le_bytes());
                    }
                    Packing::Packed12 => {
                        let (a, b) = (a.min(4095), b.min(4095));
                        let i = (y * width + x) * 3 / 2;
                        out[i..i + 3].copy_from_slice(&[
                            a as u8,
                            ((a >> 8) | (b << 4)) as u8,
                            (b >> 4) as u8,
                        ]);
                    }
                }
            }
        }
        Ok(())
    }
}
