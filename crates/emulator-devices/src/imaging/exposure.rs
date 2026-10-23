//! Radiometric sensor input. RGB encodes linearized relative scene radiance;
//! its declared white luminance supplies absolute cd/m². Sensor signal is
//! irradiance * integration time * analog gain, clipped at the ADC full well.
use crate::scene::{Frame, FramePixels, SensorFramesHandle};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    pub full_scale: u16,
    pub reference_white: [u16; 3],
    pub saturation_lux_seconds: f64,
    pub color_sample_weights: [u8; 3],
}
impl Calibration {
    pub fn validate(self) -> Result<Self, String> {
        if self.full_scale == 0
            || self.reference_white.contains(&0)
            || !self.saturation_lux_seconds.is_finite()
            || self.saturation_lux_seconds <= 0.
            || self.color_sample_weights.contains(&0)
        {
            return Err("invalid radiometric sensor calibration".into());
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Exposure {
    pub seconds: f64,
    pub analog_gain: f64,
}
pub type ExposureHandle = Arc<Mutex<Exposure>>;
pub type ApertureHandle = Arc<Mutex<f64>>;

/// Derived optical pixels retain their full-scene law even when a readout
/// computes only a subset. Checkpoints store materialized canonical pixels.
#[derive(Clone)]
pub(super) enum Filtered {
    Full(Arc<[u8]>),
    Grid([usize; 2], Arc<[u8]>),
    Readout(Arc<super::optics::Projected>),
    BinnedReadout([usize; 2], Arc<super::optics::Projected>),
}
impl Filtered {
    pub fn projected(pixels: Arc<super::optics::Projected>) -> Self {
        if pixels.complete() {
            Self::Full(pixels.materialize())
        } else {
            Self::Readout(pixels)
        }
    }
    pub fn materialize(&self) -> Arc<[u8]> {
        match self {
            Self::Full(p) | Self::Grid(_, p) => p.clone(),
            Self::Readout(p) | Self::BinnedReadout(_, p) => p.materialize(),
        }
    }
    fn same_pixels(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Full(a), Self::Full(b)) => Arc::ptr_eq(a, b),
            (Self::Grid(da, a), Self::Grid(db, b)) => da == db && Arc::ptr_eq(a, b),
            (Self::Readout(a), Self::Readout(b)) => Arc::ptr_eq(a, b),
            (Self::BinnedReadout(da, a), Self::BinnedReadout(db, b)) => {
                da == db && Arc::ptr_eq(a, b)
            }
            _ => false,
        }
    }
}

/// Immutable prepared raw storage and precomputed byte offsets for a readout.
/// Construction only accepts samples already prepared by a guest consumer.
pub(crate) struct SampleOffsets {
    pub pixels: Arc<[u8]>,
    pub columns: Vec<usize>,
    pub rows: Vec<usize>,
}
impl SampleOffsets {
    #[inline(always)]
    pub fn sample(&self, row: usize, column: usize) -> [u16; 3] {
        let at = row + column;
        std::array::from_fn(|c| {
            u16::from_le_bytes([self.pixels[at + c * 2], self.pixels[at + c * 2 + 1]])
        })
    }
}
/// Immutable values at one physical exposure edge, shared by the preview and
/// its pre-ISP metering windows. Subsequent host or register updates cannot
/// change an exposure already sampled.
#[derive(Clone)]
pub struct ExposedFrame {
    pub frame: Arc<Frame>,
    pub exposure: Exposure,
    pub aperture_squared: f64,
    pub color_sample_weights: [u8; 3],
    response: [[u16; 256]; 3],
    pub(super) response_scale: [f64; 3],
    pub focus_shift_mm: Option<f64>,
    pub(super) filtered: Option<Filtered>,
}
impl ExposedFrame {
    /// Equality of the immutable inputs consumed by `sample`, independent of
    /// frame-edge numbering and physical metadata that do not change pixels.
    pub fn same_samples(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frame, &other.frame)
            && self.response == other.response
            && self.focus_shift_mm == other.focus_shift_mm
            && self.aperture_squared == other.aperture_squared
            && match (&self.filtered, &other.filtered) {
                (None, None) => true,
                (Some(a), Some(b)) => a.same_pixels(b),
                _ => false,
            }
    }
    pub fn capture(
        frames: &SensorFramesHandle,
        exposure: &ExposureHandle,
        aperture: &ApertureHandle,
        calibration: Calibration,
    ) -> Result<Option<Self>, String> {
        let exposure = *exposure.lock().map_err(|_| "sensor exposure poisoned")?;
        if exposure.seconds == 0. {
            return Ok(None);
        }
        let aperture_squared = *aperture.lock().map_err(|_| "iris transmission poisoned")?;
        let frame = frames
            .lock()
            .map_err(|_| "sensor input poisoned")?
            .current
            .clone();
        Self::from_frame(frame, exposure, aperture_squared, calibration).map(Some)
    }
    pub fn from_frame(
        frame: Arc<Frame>,
        exposure: Exposure,
        aperture_squared: f64,
        calibration: Calibration,
    ) -> Result<Self, String> {
        if !exposure.seconds.is_finite()
            || exposure.seconds <= 0.
            || !exposure.analog_gain.is_finite()
            || exposure.analog_gain <= 0.
            || !aperture_squared.is_finite()
            || aperture_squared <= 0.
        {
            return Err("invalid physical exposure".into());
        }
        let calibration = calibration.validate()?;
        if !frame.white_luminance.is_finite() || frame.white_luminance <= 0. {
            return Err("sensor input needs positive finite white radiance".into());
        }
        let scale = frame.white_luminance * core::f64::consts::PI / (4. * aperture_squared)
            * exposure.seconds
            * exposure.analog_gain
            / calibration.saturation_lux_seconds;
        let response = std::array::from_fn(|c| {
            std::array::from_fn(|v| {
                let s = v as f64 / 255.;
                let linear = if s <= 0.04045 {
                    s / 12.92
                } else {
                    libm::pow((s + 0.055) / 1.055, 2.4)
                };
                libm::rint(
                    linear * scale * calibration.full_scale as f64 * 256.
                        / calibration.reference_white[c] as f64,
                )
                .clamp(0., 16383.) as u16
            })
        });
        // Mosaic radiance has no photometric white or reference white: its
        // counts are the RAF normalization scaled by the physical exposure.
        // The largest site sets the scale, matching the optical scene's
        // normalization, so both sampling laws share one count mapping.
        let response_scale: [f64; 3] = match frame.mosaic() {
            Some(sites) => [f64::from(sites.max) * mosaic_gain(exposure, aperture_squared); 3],
            None => std::array::from_fn(|c| {
                scale * calibration.full_scale as f64 * 256. / calibration.reference_white[c] as f64
            }),
        };
        if response_scale.iter().any(|v| !v.is_finite() || *v < 0.) {
            return Err("physical radiance exceeds finite sensor response".into());
        }
        Ok(Self {
            frame,
            exposure,
            aperture_squared,
            color_sample_weights: calibration.color_sample_weights,
            response,
            response_scale,
            focus_shift_mm: None,
            filtered: None,
        })
    }
    /// Counts per unit of mosaic radiance: sensor counts = radiance × seconds /
    /// f_number² × analog_gain, with aperture_squared = f_number².
    fn mosaic_gain(&self) -> f64 {
        mosaic_gain(self.exposure, self.aperture_squared)
    }
    pub(crate) fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        let response: Vec<u16> = self.response.iter().flatten().copied().collect();
        Ok(crate::checkpoint::Checkpoint::encode(
            "exposed-frame",
            &(
                self.exposure,
                self.aperture_squared,
                self.color_sample_weights,
                response,
                self.response_scale,
                self.focus_shift_mm,
                self.filtered.as_ref().map(|p| match p {
                    Filtered::Grid(dimensions, _) | Filtered::BinnedReadout(dimensions, _) => {
                        *dimensions
                    }
                    _ => [self.frame.width, self.frame.height],
                }),
            ),
        )?
        .with_children(vec![self.frame.checkpoint()?])
        .with_blobs(self.filtered.iter().map(Filtered::materialize).collect()))
    }
    pub(crate) fn from_checkpoint(saved: &crate::checkpoint::Checkpoint) -> Result<Self, String> {
        let (
            exposure,
            aperture_squared,
            color_sample_weights,
            response,
            response_scale,
            focus_shift_mm,
            filtered,
        ): (
            Exposure,
            f64,
            [u8; 3],
            Vec<u16>,
            [f64; 3],
            Option<f64>,
            Option<[usize; 2]>,
        ) = saved.decode("exposed-frame")?;
        if !exposure.seconds.is_finite()
            || exposure.seconds <= 0.
            || !exposure.analog_gain.is_finite()
            || exposure.analog_gain <= 0.
            || !aperture_squared.is_finite()
            || aperture_squared <= 0.
            || color_sample_weights.contains(&0)
            || response.len() != 768
            || response.iter().any(|v| *v > 16383)
            || response_scale.iter().any(|v| !v.is_finite() || *v < 0.)
            || focus_shift_mm.is_some_and(|v| !v.is_finite())
            || saved.children().len() != 1
            || saved.blobs().len() != usize::from(filtered.is_some())
        {
            return Err("invalid exposed-frame checkpoint response or geometry".into());
        }
        let frame = Arc::new(Frame::from_checkpoint(&saved.children()[0])?);
        let filtered = if let Some(dimensions) = filtered {
            let pixels = &saved.blobs()[0];
            if dimensions.contains(&0)
                || dimensions.iter().any(|&n| n > 8192)
                || pixels.len() != dimensions[0] * dimensions[1] * 6
                || pixels
                    .chunks_exact(2)
                    .any(|word| u16::from_le_bytes([word[0], word[1]]) > 16383)
            {
                return Err("invalid exposed-frame filtered pixel checkpoint".into());
            }
            Some(Filtered::Grid(dimensions, pixels.clone()))
        } else {
            None
        };
        Ok(Self {
            frame,
            exposure,
            aperture_squared,
            color_sample_weights,
            response: std::array::from_fn(|c| std::array::from_fn(|i| response[c * 256 + i])),
            response_scale,
            focus_shift_mm,
            filtered,
        })
    }
    #[inline]
    pub fn sample(&self, x: usize, y: usize, grid: [usize; 2]) -> [u16; 3] {
        if let Some(Filtered::BinnedReadout([width, height], pixels)) = &self.filtered {
            let sx = ((2 * x + 1) * width / (2 * grid[0])).min(width - 1);
            let sy = ((2 * y + 1) * height / (2 * grid[1])).min(height - 1);
            return pixels.sample(sx, sy);
        }
        if let Some(Filtered::Grid([width, height], pixels)) = &self.filtered {
            let sx = ((2 * x + 1) * width / (2 * grid[0])).min(width - 1);
            let sy = ((2 * y + 1) * height / (2 * grid[1])).min(height - 1);
            let at = (sy * width + sx) * 6;
            return std::array::from_fn(|c| {
                u16::from_le_bytes([pixels[at + c * 2], pixels[at + c * 2 + 1]])
            });
        }
        let f = &self.frame;
        let sx = ((2 * x + 1) * f.width / (2 * grid[0])).min(f.width - 1);
        let sy = ((2 * y + 1) * f.height / (2 * grid[1])).min(f.height - 1);
        self.sample_source(sx, sy)
    }
    /// Sample a coordinate already mapped into the immutable source raster.
    /// Raster consumers can compute their row/column maps once per frame.
    #[inline(always)]
    pub(crate) fn sample_source(&self, sx: usize, sy: usize) -> [u16; 3] {
        let f = &self.frame;
        if matches!(
            self.filtered,
            Some(Filtered::Grid(_, _) | Filtered::BinnedReadout(_, _))
        ) {
            return self.sample(sx, sy, [f.width, f.height]);
        }
        if let Some(Filtered::Readout(pixels)) = &self.filtered {
            return pixels.sample(sx, sy);
        }
        if let Some(Filtered::Full(pixels)) = &self.filtered {
            let at = (sy * f.width + sx) * 6;
            return std::array::from_fn(|c| {
                u16::from_le_bytes([pixels[at + c * 2], pixels[at + c * 2 + 1]])
            });
        }
        match &f.pixels {
            FramePixels::Rgb24(p) => {
                std::array::from_fn(|c| self.response[c][p[(sy * f.width + sx) * 3 + c] as usize])
            }
            FramePixels::Raw16(p) => [p[sy * f.width + sx]; 3],
            // Mosaic frames always carry their cell means; see `Frame::mosaic`.
            FramePixels::Mosaic { .. } => f.mosaic().map_or([0; 3], |sites| {
                let gain = self.mosaic_gain();
                sites
                    .site(sx, sy)
                    .map(|v| libm::rint(f64::from(v) * gain).clamp(0., 16383.) as u16)
            }),
        }
    }
    pub fn sample_dimensions(&self) -> [usize; 2] {
        match &self.filtered {
            Some(Filtered::Grid(dimensions, _) | Filtered::BinnedReadout(dimensions, _)) => {
                *dimensions
            }
            _ => [self.frame.width, self.frame.height],
        }
    }
    pub(crate) fn sample_offsets(
        &self,
        grid: [usize; 2],
        window: [usize; 4],
        step: usize,
    ) -> Option<SampleOffsets> {
        let [width, height] = self.sample_dimensions();
        let [x, y, w, h] = window;
        match &self.filtered {
            Some(Filtered::Readout(p) | Filtered::BinnedReadout(_, p)) => {
                p.sample_offsets(grid, window, step)
            }
            Some(Filtered::Full(p) | Filtered::Grid(_, p)) => Some(SampleOffsets {
                pixels: p.clone(),
                columns: (x..(x + w).min(grid[0]))
                    .step_by(step)
                    .map(|x| ((2 * x + 1) * width / (2 * grid[0])).min(width - 1) * 6)
                    .collect(),
                rows: (y..(y + h).min(grid[1]))
                    .map(|y| ((2 * y + 1) * height / (2 * grid[1])).min(height - 1) * width * 6)
                    .collect(),
            }),
            None => None,
        }
    }
    pub(crate) fn covers(&self, grid: [usize; 2], windows: &[[usize; 4]]) -> bool {
        match &self.filtered {
            Some(Filtered::Readout(p) | Filtered::BinnedReadout(_, p)) => p.covers(grid, windows),
            _ => true,
        }
    }
    pub(crate) fn prepared_pixels(&self) -> usize {
        match &self.filtered {
            Some(Filtered::Readout(p) | Filtered::BinnedReadout(_, p)) => p.prepared_pixels(),
            Some(Filtered::Full(p) | Filtered::Grid(_, p)) => p.len() / 6,
            None => 0,
        }
    }
    pub fn physical_state(&self) -> serde_json::Value {
        serde_json::json!({"exposure":self.exposure,"aperture_squared":self.aperture_squared,
            "white_luminance":self.frame.white_luminance,"focus_shift_mm":self.focus_shift_mm})
    }
    pub fn fingerprint(&self) -> serde_json::Value {
        let response: Vec<u16> = self.response.iter().flatten().copied().collect();
        serde_json::json!({"frame":self.frame.fingerprint(),"exposure":self.exposure,
            "aperture_squared":self.aperture_squared,"color_sample_weights":self.color_sample_weights,
            "focus_shift_mm":self.focus_shift_mm,"response_scale":self.response_scale,
            "filtered":self.filtered.as_ref().map(|pixels|crate::fingerprint::bytes(&pixels.materialize())),
            "sample_dimensions":self.sample_dimensions(),
            "response":crate::fingerprint::words16(&response)})
    }
}

fn mosaic_gain(exposure: Exposure, aperture_squared: f64) -> f64 {
    exposure.seconds * exposure.analog_gain / aperture_squared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SensorFrames;
    #[test]
    fn time_iris_gain_and_absolute_radiance_share_one_signal_law() {
        let frames = Arc::new(Mutex::new(SensorFrames::new(
            Frame::new(1, 1, FramePixels::Rgb24(vec![128; 3].into())).unwrap(),
        )));
        let e = Arc::new(Mutex::new(Exposure {
            seconds: 0.001,
            analog_gain: 1.,
        }));
        let a = Arc::new(Mutex::new(64.));
        let c = Calibration {
            full_scale: 16380,
            reference_white: [256; 3],
            saturation_lux_seconds: 1.,
            color_sample_weights: [2, 5, 2],
        };
        let shot = || ExposedFrame::capture(&frames, &e, &a, c).unwrap().unwrap();
        let first = shot();
        let base = first.sample(0, 0, [1, 1])[0];
        assert!(base > 0 && base < 4096);
        e.lock().unwrap().seconds *= 2.;
        assert!((shot().sample(0, 0, [1, 1])[0] as i32 - 2 * base as i32).abs() <= 1);
        *a.lock().unwrap() *= 2.;
        assert_eq!(shot().sample(0, 0, [1, 1])[0], base);
        e.lock().unwrap().analog_gain = 2.;
        assert!((shot().sample(0, 0, [1, 1])[0] as i32 - 2 * base as i32).abs() <= 1);
        assert_eq!(first.sample(0, 0, [1, 1])[0], base);
        let mut dark = Frame::new(1, 1, FramePixels::Rgb24(vec![128; 3].into())).unwrap();
        dark.white_luminance /= 2.;
        frames.lock().unwrap().set_frame(dark);
        assert_eq!(shot().sample(0, 0, [1, 1])[0], base);
    }
}

#[cfg(test)]
mod mosaic_tests {
    use super::*;
    use crate::scene::{Frame, read_raf};
    /// X-Trans 6×6 CFA: 0 = R, 1 = G, 2 = B, as in a RAF's pattern.
    const XTRANS: [[u8; 6]; 6] = [
        [0, 2, 1, 2, 0, 1],
        [1, 1, 0, 1, 1, 2],
        [1, 1, 2, 1, 1, 0],
        [2, 0, 1, 0, 2, 1],
        [1, 1, 2, 1, 1, 0],
        [1, 1, 0, 1, 1, 2],
    ];
    fn calibration() -> Calibration {
        Calibration {
            full_scale: 16383,
            reference_white: [256; 3],
            saturation_lux_seconds: 1.,
            color_sample_weights: [2, 5, 2],
        }
    }
    /// A mosaic whose radiance is the raw counts under the recorded exposure,
    /// using the RAF normalization: radiance = counts / (seconds × gain / f²).
    fn recorded_frame(width: usize, height: usize, raw: &[u16], recorded: Exposure) -> Arc<Frame> {
        let norm = recorded.seconds * recorded.analog_gain / 81.;
        let radiance = crate::scene::mosaic::Radiance::from_counts(raw.iter().copied(), norm);
        Arc::new(
            Frame::new(
                width,
                height,
                FramePixels::Mosaic {
                    cfa: XTRANS,
                    radiance,
                },
            )
            .unwrap(),
        )
    }
    fn recorded() -> Exposure {
        Exposure {
            seconds: 1. / 800.,
            analog_gain: 1.,
        }
    }
    fn synthetic_raw(width: usize, height: usize) -> Vec<u16> {
        (0..width * height)
            .map(|i| (i * 2311 % 15000 + 1) as u16)
            .collect()
    }
    #[test]
    fn mosaic_frame_reproduces_its_recorded_counts_at_native_resolution() {
        let (width, height) = (13, 8);
        let raw = synthetic_raw(width, height);
        let frame = recorded_frame(width, height, &raw, recorded());
        let exposed = ExposedFrame::from_frame(frame, recorded(), 81., calibration()).unwrap();
        for y in 0..height {
            for x in 0..width {
                let c = usize::from(XTRANS[y % 6][x % 6]);
                assert_eq!(
                    exposed.sample_source(x, y)[c],
                    raw[y * width + x],
                    "site ({x}, {y})"
                );
            }
        }
    }
    #[test]
    fn doubling_exposure_doubles_mosaic_counts_until_full_scale() {
        let (width, height) = (13, 8);
        let raw = synthetic_raw(width, height);
        let frame = recorded_frame(width, height, &raw, recorded());
        let doubled = Exposure {
            seconds: 2. / 800.,
            analog_gain: 1.,
        };
        let exposed = ExposedFrame::from_frame(frame, doubled, 81., calibration()).unwrap();
        let mut clipped = 0;
        for y in 0..height {
            for x in 0..width {
                let c = usize::from(XTRANS[y % 6][x % 6]);
                let expected = (2 * raw[y * width + x]).min(16383);
                clipped += usize::from(2 * raw[y * width + x] > 16383);
                assert_eq!(exposed.sample_source(x, y)[c], expected, "site ({x}, {y})");
            }
        }
        assert!(clipped > 0, "the test must reach full scale");
    }
    #[test]
    fn stopping_down_from_f9_to_f4_5_multiplies_mosaic_counts_by_four() {
        let (width, height) = (13, 8);
        let raw = synthetic_raw(width, height);
        let frame = recorded_frame(width, height, &raw, recorded());
        let exposed = ExposedFrame::from_frame(frame, recorded(), 20.25, calibration()).unwrap();
        for y in 0..height {
            for x in 0..width {
                let c = usize::from(XTRANS[y % 6][x % 6]);
                assert_eq!(
                    exposed.sample_source(x, y)[c],
                    (4 * raw[y * width + x]).min(16383),
                    "site ({x}, {y})"
                );
            }
        }
    }
    #[test]
    fn real_raf_sites_reproduce_their_raw_counts_after_black() {
        let Ok(path) = std::env::var("FUJIMOD_TEST_RAF") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read the test RAF");
        let scene = read_raf(std::io::Cursor::new(&bytes)).expect("parse the test RAF");
        let recorded = scene.exposure.clone();
        assert_eq!((recorded.iso, recorded.dynamic_range), (800, 400));
        // Analog gain relative to ISO 200 DR100, as the RAF normalization defines it.
        let analog_gain = f64::from(recorded.iso) / 200.
            * match recorded.dynamic_range {
                200 => 0.5,
                400 => 0.25,
                _ => 1.,
            };
        assert_eq!(analog_gain, 1.);
        let exposure = Exposure {
            seconds: recorded.seconds,
            analog_gain,
        };
        let frame = Frame::from_raf(&scene).expect("RAF crop as mosaic");
        let FramePixels::Mosaic { cfa, .. } = &frame.pixels else {
            panic!("RAF crop is a mosaic frame");
        };
        let cfa = *cfa;
        let (width, height) = (frame.width, frame.height);
        let (left, top) = (scene.crop_left as usize, scene.crop_top as usize);
        let exposed = ExposedFrame::from_frame(
            Arc::new(frame),
            exposure,
            recorded.f_number * recorded.f_number,
            calibration(),
        )
        .unwrap();
        // Read the file's raw plane directly: 2048 TIFF bytes follow the raw offset.
        let raw_offset = u32::from_be_bytes([bytes[100], bytes[101], bytes[102], bytes[103]]);
        let samples = raw_offset as usize + 2048;
        let raw_at = |x: usize, y: usize| {
            let at = samples + 2 * (y * scene.width as usize + x);
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        };
        let black = scene.black[0];
        let mut lit = 0;
        for i in 0..400usize {
            let sx = (i * 7919 + 13) % width;
            let sy = (i * 104_729 + 7) % height;
            let (x, y) = (sx + left, sy + top);
            assert_eq!(cfa[sy % 6][sx % 6], scene.cfa[y % 6][x % 6], "crop phase");
            let raw = raw_at(x, y).saturating_sub(black);
            lit += usize::from(raw > 0);
            let c = usize::from(cfa[sy % 6][sx % 6]);
            assert_eq!(exposed.sample_source(sx, sy)[c], raw, "site ({x}, {y})");
        }
        assert!(lit > 100, "the sample must include lit sites");
    }
}
