//! A declared thin-lens subject plane and scalar box point-spread approximation.
//! Only the physical focus wire, iris, scene pixels, and optical dimensions enter
//! this law. It supplies pixels to every sensor consumer, never an AF decision.
use super::exposure::ExposedFrame;
use crate::scene::{Frame, FramePixels};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
pub type FocusHandle = Arc<Mutex<f64>>;
/// Relative field illumination is explicit optical fixture data. Cosine-fourth
/// is the ideal thin-lens approximation, not a measured camera lens profile.
#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Illumination {
    Uniform,
    CosineFourth,
}
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub focus: String,
    pub focal_length_mm: f64,
    pub subject_distance_mm: f64,
    pub sensor_width_mm: f64,
    pub relative_illumination: Illumination,
}
mod horizontal;
mod readout;
mod scene;
use readout::Coordinates;
pub(super) use readout::Projected;
use scene::Scene;

pub struct Projection {
    focus: FocusHandle,
    spec: Spec,
    subject_shift: f64,
    scene: Mutex<Option<Arc<Scene>>>,
    prepared: Mutex<Option<Prepared>>,
    horizontal: Mutex<horizontal::Cache>,
    binned: Mutex<Vec<Arc<Scene>>>,
    readouts: Mutex<Vec<BinnedPrepared>>,
}
const CACHE_PIXELS: usize = 4 * 1024 * 1024;
struct Prepared {
    scene: Arc<Scene>,
    coordinates: Arc<Coordinates>,
    radius: usize,
    fraction: u32,
    linear: Arc<[[u16; 3]]>,
    response_scale: [f64; 3],
    image_distance: Option<f64>,
    pixels: Arc<Projected>,
}
struct BinnedPrepared {
    scene: Arc<Scene>,
    grid: [usize; 2],
    windows: Vec<[usize; 4]>,
    radius: [usize; 2],
    fraction: [u32; 2],
    coordinates: Arc<Coordinates>,
    linear: Arc<[[u16; 3]]>,
    response_scale: [f64; 3],
    distance: Option<f64>,
    pixels: Arc<Projected>,
}
impl Projection {
    pub fn new(spec: Spec, focus: FocusHandle) -> Result<Self, String> {
        if !spec.focal_length_mm.is_finite()
            || spec.focal_length_mm <= 0.
            || !spec.subject_distance_mm.is_finite()
            || spec.subject_distance_mm <= spec.focal_length_mm
            || !spec.sensor_width_mm.is_finite()
            || spec.sensor_width_mm <= 0.
        {
            return Err("optical projection requires finite positive dimensions and a subject beyond the focal length".into());
        }
        let subject_shift = spec.focal_length_mm
            * (spec.focal_length_mm / (spec.subject_distance_mm - spec.focal_length_mm));
        if !(spec.focal_length_mm + subject_shift).is_finite() {
            return Err("optical image distance exceeds finite geometry".into());
        }
        Ok(Self {
            focus,
            spec,
            subject_shift,
            scene: Mutex::new(None),
            prepared: Mutex::new(None),
            horizontal: Mutex::new(horizontal::Cache::default()),
            binned: Mutex::new(Vec::new()),
            readouts: Mutex::new(Vec::new()),
        })
    }
    pub(crate) fn spec(&self) -> &Spec {
        &self.spec
    }
    pub fn focus(&self) -> Result<f64, String> {
        let value = *self.focus.lock().map_err(|_| "focus wire poisoned")?;
        if !value.is_finite() {
            return Err("focus shift is nonfinite".into());
        }
        Ok(value)
    }
    /// The frame's full-resolution linear scene. The scene of the current
    /// sensor frame is always retained, whatever its size: frames are immutable
    /// and rarely replaced. Mosaic scenes retain cell means and build row
    /// prefixes on demand; RGB scenes retain their small row prefixes.
    fn full_scene(&self, frame: &Arc<Frame>) -> Result<Arc<Scene>, String> {
        let mut cache = self
            .scene
            .lock()
            .map_err(|_| "optical scene cache poisoned")?;
        if let Some(old) = cache.as_ref().filter(|old| old.frame.same_pixels(frame)) {
            return Ok(old.clone());
        }
        let scene = Arc::new(Scene::new(frame.clone())?);
        *cache = Some(scene.clone());
        Ok(scene)
    }
    pub(crate) fn prepare_binned_windows(
        &self,
        frame: &mut ExposedFrame,
        dimensions: [usize; 2],
        grid: [usize; 2],
        windows: &[[usize; 4]],
    ) -> Result<(), String> {
        if !projects(&frame.frame) {
            return Ok(());
        }
        let original = [frame.frame.width, frame.frame.height];
        let scene = if dimensions == original {
            self.full_scene(&frame.frame)?
        } else {
            // A cached binned scene needs no full-resolution scene: a sensor
            // frame larger than the scene cache is linearized once per size.
            let mut cache = self
                .binned
                .lock()
                .map_err(|_| "binned scene cache poisoned")?;
            cache.retain(|p| p.frame.same_pixels(&frame.frame));
            if let Some(old) = cache.iter().find(|p| p.dimensions == dimensions) {
                old.clone()
            } else {
                let binned = Arc::new(self.full_scene(&frame.frame)?.binned(dimensions));
                let cost = dimensions[0] * dimensions[1];
                while cache
                    .iter()
                    .map(|p| p.dimensions[0] * p.dimensions[1])
                    .sum::<usize>()
                    + cost
                    > CACHE_PIXELS
                    && !cache.is_empty()
                {
                    cache.remove(0);
                }
                if cost <= CACHE_PIXELS {
                    cache.push(binned.clone());
                }
                binned
            }
        };
        let shift = frame
            .focus_shift_mm
            .ok_or("readout has no captured optical focus")?;
        let diameter = (shift - self.subject_shift).abs() * self.spec.focal_length_mm
            / (libm::sqrt(frame.aperture_squared)
                * (self.spec.focal_length_mm + self.subject_shift));
        let sensor_height = self.spec.sensor_width_mm * original[1] as f64 / original[0] as f64;
        let radius = [
            diameter * dimensions[0] as f64 / (2. * self.spec.sensor_width_mm),
            diameter * dimensions[1] as f64 / (2. * sensor_height),
        ];
        let whole =
            radius.map(|r| libm::floor(r).min(dimensions[0].max(dimensions[1]) as f64) as usize);
        let fraction = std::array::from_fn(|i| {
            libm::rint((radius[i] - libm::floor(radius[i])) * 65536.).clamp(0., 65535.) as u32
        });
        let distance = match self.spec.relative_illumination {
            Illumination::Uniform => None,
            Illumination::CosineFourth => Some(self.spec.focal_length_mm + shift),
        };
        if distance.is_some_and(|d| !d.is_finite() || d <= 0.) {
            return Err("invalid readout image distance".into());
        }
        let mut cache = self
            .readouts
            .lock()
            .map_err(|_| "optical readout cache poisoned")?;
        cache.retain(|p| p.scene.frame.same_pixels(&scene.frame));
        let found = cache.iter().position(|p| {
            Arc::ptr_eq(&p.scene, &scene)
                && p.grid == grid
                && p.windows == windows
                && p.radius == whole
                && p.fraction == fraction
        });
        if let Some(i) = found {
            let old = &mut cache[i];
            if old.response_scale != frame.response_scale || old.distance != distance {
                old.pixels = Arc::new(Projected::new_axes(
                    scene,
                    old.coordinates.clone(),
                    whole,
                    fraction,
                    &old.linear,
                    frame.response_scale,
                    [self.spec.sensor_width_mm, sensor_height],
                    distance,
                ));
                old.response_scale = frame.response_scale;
                old.distance = distance;
            }
            frame.filtered = Some(super::exposure::Filtered::BinnedReadout(
                dimensions,
                old.pixels.clone(),
            ));
            return Ok(());
        }
        let coordinates = match cache.iter().find(|old| {
            Arc::ptr_eq(&old.scene, &scene) && old.grid == grid && old.windows == windows
        }) {
            Some(old) => old.coordinates.clone(),
            None => Arc::new(Coordinates::windows(dimensions, grid, windows)?),
        };
        let horizontal = self
            .horizontal
            .lock()
            .map_err(|_| "horizontal optical cache poisoned")?
            .prepare(scene.clone(), coordinates.clone(), whole, fraction[0]);
        let linear: Arc<[[u16; 3]]> = readout::project_prepared_axes(
            &scene,
            &coordinates,
            whole,
            fraction,
            horizontal.as_ref(),
        )
        .into();
        let pixels = Arc::new(Projected::new_axes(
            scene.clone(),
            coordinates.clone(),
            whole,
            fraction,
            &linear,
            frame.response_scale,
            [self.spec.sensor_width_mm, sensor_height],
            distance,
        ));
        frame.filtered = Some(super::exposure::Filtered::BinnedReadout(
            dimensions,
            pixels.clone(),
        ));
        let cost = linear.len();
        while !cache.is_empty()
            && (cache.len() >= 16
                || cache.iter().map(|p| p.linear.len()).sum::<usize>() + cost > CACHE_PIXELS)
        {
            cache.remove(0);
        }
        if cost <= CACHE_PIXELS {
            cache.push(BinnedPrepared {
                scene,
                grid,
                windows: windows.to_vec(),
                radius: whole,
                fraction,
                coordinates,
                linear,
                response_scale: frame.response_scale,
                distance,
                pixels,
            });
        }
        Ok(())
    }
    /// Eager full-scene projection for a consumer requesting every coordinate.
    pub fn prepare(&self, frame: &mut ExposedFrame, shift: f64) -> Result<(), String> {
        let dimensions = [frame.frame.width, frame.frame.height];
        self.prepare_readout(
            frame,
            shift,
            dimensions,
            [0, 0, dimensions[0], dimensions[1]],
        )?;
        if let Some(filtered) = &frame.filtered {
            frame.filtered = Some(super::exposure::Filtered::Full(filtered.materialize()));
        }
        Ok(())
    }
    /// Project only the source coordinates consumed by this declared readout.
    /// The immutable projection law also serves other samples and inspection.
    pub(crate) fn prepare_readout(
        &self,
        frame: &mut ExposedFrame,
        shift: f64,
        grid: [usize; 2],
        window: [usize; 4],
    ) -> Result<(), String> {
        Coordinates::validate(grid, window)?;
        if !shift.is_finite() {
            return Err("sampled focus shift is nonfinite".into());
        }
        if frame.focus_shift_mm == Some(shift) {
            match &frame.filtered {
                Some(super::exposure::Filtered::Full(_)) => return Ok(()),
                Some(
                    super::exposure::Filtered::Grid(_, _)
                    | super::exposure::Filtered::BinnedReadout(_, _),
                ) => {}
                Some(super::exposure::Filtered::Readout(p))
                    if p.coordinates.key == (grid, window) =>
                {
                    return Ok(());
                }
                _ => (),
            }
        }
        if !projects(&frame.frame) {
            return Ok(());
        }
        let image_distance = match self.spec.relative_illumination {
            Illumination::Uniform => None,
            Illumination::CosineFourth => {
                let d = self.spec.focal_length_mm + shift;
                if !d.is_finite() || d <= 0. {
                    return Err("thin-lens image distance must be finite and positive".into());
                }
                Some(d)
            }
        };
        frame.focus_shift_mm = Some(shift);
        frame.filtered = None;
        let diameter = (shift - self.subject_shift).abs() * self.spec.focal_length_mm
            / (libm::sqrt(frame.aperture_squared)
                * (self.spec.focal_length_mm + self.subject_shift));
        let radius = (diameter * frame.frame.width as f64 / (2. * self.spec.sensor_width_mm))
            .min(frame.frame.width.max(frame.frame.height) as f64);
        let whole = libm::floor(radius);
        let fraction = libm::rint((radius - whole) * 65536.).clamp(0., 65535.) as u32;
        let radius = whole as usize;
        if radius == 0 && fraction == 0 && image_distance.is_none() {
            return Ok(());
        }
        let dimensions = [frame.frame.width, frame.frame.height];
        let scene = self.full_scene(&frame.frame)?;
        let mut prepared = self.prepared.lock().map_err(|_| "optical cache poisoned")?;
        let coordinates = match &*prepared {
            Some(old)
                if Arc::ptr_eq(&old.scene, &scene) && old.coordinates.key == (grid, window) =>
            {
                old.coordinates.clone()
            }
            _ => Arc::new(Coordinates::new(dimensions, grid, window)?),
        };
        let linear = if let Some(old) = prepared.as_ref().filter(|old| {
            Arc::ptr_eq(&old.scene, &scene)
                && Arc::ptr_eq(&old.coordinates, &coordinates)
                && old.radius == radius
                && old.fraction == fraction
        }) {
            if old.response_scale == frame.response_scale && old.image_distance == image_distance {
                frame.filtered = Some(super::exposure::Filtered::projected(old.pixels.clone()));
                return Ok(());
            }
            old.linear.clone()
        } else {
            let horizontal = self
                .horizontal
                .lock()
                .map_err(|_| "horizontal optical cache poisoned")?
                .prepare(scene.clone(), coordinates.clone(), [radius; 2], fraction);
            Arc::from(readout::project(
                &scene,
                &coordinates,
                radius,
                fraction,
                horizontal.as_ref(),
            ))
        };
        let pixels = Arc::new(Projected::new(
            scene.clone(),
            coordinates.clone(),
            radius,
            fraction,
            &linear,
            frame.response_scale,
            self.spec.sensor_width_mm,
            image_distance,
        ));
        frame.filtered = Some(super::exposure::Filtered::projected(pixels.clone()));
        if dimensions[0] * dimensions[1] <= CACHE_PIXELS && linear.len() <= CACHE_PIXELS {
            *prepared = Some(Prepared {
                scene,
                coordinates,
                radius,
                fraction,
                linear,
                response_scale: frame.response_scale,
                image_distance,
                pixels,
            });
        } else {
            *prepared = None;
        }
        Ok(())
    }
}
// Farrell, Catrysse and Wandell, Applied Optics 51 (2012), section 2.B.2:
// https://web.stanford.edu/~wandell/data/papers/2012-DCS-OSA-Farrell.pdf
// For the selected ideal lens, cos(theta)^4 = (d^2/(d^2+x^2+y^2))^2.
fn relative_illumination(x: f64, y: f64, d: f64) -> f64 {
    let (x, y) = (x / d, y / d);
    let cos_squared = 1. / (1. + x * x + y * y);
    cos_squared * cos_squared
}
fn encode_response(
    linear: &[[u16; 3]],
    scale: [f64; 3],
    dimensions: [usize; 2],
    sensor_width_mm: f64,
    image_distance: Option<f64>,
) -> Arc<[u8]> {
    let mut pixels = vec![0; linear.len() * 6];
    let [width, height] = dimensions;
    let pitch = sensor_width_mm / width as f64;
    for (i, (pixel, packed)) in linear.iter().zip(pixels.chunks_exact_mut(6)).enumerate() {
        // Sample square pixels at their centers. Both RAW and metering consume
        // this same exposed frame; no statistics-specific brightness correction.
        let field = image_distance.map_or(1., |d| {
            relative_illumination(
                (i % width) as f64 * pitch + pitch * 0.5 - sensor_width_mm * 0.5,
                (i / width) as f64 * pitch + pitch * 0.5 - height as f64 * pitch * 0.5,
                d,
            )
        });
        for c in 0..3 {
            let value = libm::rint(f64::from(pixel[c]) * scale[c] / 65535. * field)
                .clamp(0., 16383.) as u16;
            packed[c * 2..c * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    pixels.into()
}

fn encode_response_rect(
    linear: &[[u16; 3]],
    scale: [f64; 3],
    dimensions: [usize; 2],
    sensor: [f64; 2],
    distance: Option<f64>,
) -> Arc<[u8]> {
    let mut pixels = vec![0; linear.len() * 6];
    for (i, (pixel, out)) in linear.iter().zip(pixels.chunks_exact_mut(6)).enumerate() {
        let field = distance.map_or(1., |d| {
            relative_illumination(
                ((i % dimensions[0]) as f64 + 0.5) * sensor[0] / dimensions[0] as f64
                    - sensor[0] * 0.5,
                ((i / dimensions[0]) as f64 + 0.5) * sensor[1] / dimensions[1] as f64
                    - sensor[1] * 0.5,
                d,
            )
        });
        for c in 0..3 {
            out[c * 2..c * 2 + 2].copy_from_slice(
                &(libm::rint(f64::from(pixel[c]) * scale[c] / 65535. * field).clamp(0., 16383.)
                    as u16)
                    .to_le_bytes(),
            );
        }
    }
    pixels.into()
}
fn encode_response_coordinates(
    linear: &[[u16; 3]],
    scale: [f64; 3],
    dimensions: [usize; 2],
    sensor: [f64; 2],
    distance: Option<f64>,
    coordinates: &Coordinates,
) -> Arc<[u8]> {
    let mut pixels: Arc<[u8]> = std::iter::repeat_n(0, linear.len() * 6).collect();
    let width = coordinates.xs.len();
    if width == 0 {
        return pixels;
    }
    // The normalized field axes do not depend on the other coordinate. Keep
    // their original operation order while sharing them across the readout.
    let axis = |at: usize, length: usize, size: f64| {
        let v = ((at as f64 + 0.5) / length as f64 - 0.5) * size;
        distance.map_or(0., |d| {
            let v = v / d;
            v * v
        })
    };
    let xs: Vec<_> = coordinates
        .xs
        .iter()
        .map(|&x| axis(x, dimensions[0], sensor[0]))
        .collect();
    let rows = coordinates
        .ys
        .iter()
        .zip(linear.chunks_exact(width))
        .zip(
            Arc::get_mut(&mut pixels)
                .unwrap()
                .chunks_exact_mut(width * 6),
        )
        .map(|((&y, linear), out)| (axis(y, dimensions[1], sensor[1]), linear, out));
    if distance.is_some() {
        encode_rows(rows, &xs, scale, field);
    } else {
        encode_rows(rows, &xs, scale, |_, _| 1.);
    }
    pixels
}
#[inline(always)]
fn field(xx: f64, yy: f64) -> f64 {
    let v = 1. / (1. + xx + yy);
    v * v
}
#[inline(always)]
fn encode_rows<'a>(
    rows: impl Iterator<Item = (f64, &'a [[u16; 3]], &'a mut [u8])>,
    xs: &[f64],
    scale: [f64; 3],
    field: impl Fn(f64, f64) -> f64,
) {
    for (yy, linear, out) in rows {
        for ((&xx, &[r, g, b]), out) in xs.iter().zip(linear).zip(out.chunks_exact_mut(6)) {
            let field = field(xx, yy);
            // Array maps are not inlined here; keep the channels explicit.
            let encode = |v: u16, scale: f64| {
                (f64::from(v) * scale / 65535. * field)
                    .round_ties_even()
                    .clamp(0., 16383.) as u16
            };
            let [r, g, b] = [
                encode(r, scale[0]),
                encode(g, scale[1]),
                encode(b, scale[2]),
            ];
            out[0] = r as u8;
            out[1] = (r >> 8) as u8;
            out[2] = g as u8;
            out[3] = (g >> 8) as u8;
            out[4] = b as u8;
            out[5] = (b >> 8) as u8;
        }
    }
}
pub(crate) fn bin_exposure(frame: &mut ExposedFrame, dimensions: [usize; 2]) -> Result<(), String> {
    if !projects(&frame.frame) {
        return Ok(());
    }
    let scene = Scene::new(frame.frame.clone())?.binned(dimensions);
    let linear: Vec<_> = (0..dimensions[1])
        .flat_map(|y| (0..dimensions[0]).map(move |x| (x, y)))
        .map(|(x, y)| scene.linear(x, y))
        .collect();
    frame.filtered = Some(super::exposure::Filtered::Grid(
        dimensions,
        encode_response_rect(&linear, frame.response_scale, dimensions, [1., 1.], None),
    ));
    Ok(())
}
/// RAW samples have no scene radiance and bypass the optical law. RGB and
/// mosaic frames are projected through the same lens.
fn projects(frame: &Frame) -> bool {
    !matches!(frame.pixels, FramePixels::Raw16(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        imaging::exposure::{Calibration, Exposure},
        scene::Frame,
    };
    fn frame(rgb: Vec<u8>, seconds: f64) -> ExposedFrame {
        ExposedFrame::from_frame(
            Arc::new(Frame::new(64, 8, FramePixels::Rgb24(rgb.into())).unwrap()),
            Exposure {
                seconds,
                analog_gain: 1.,
            },
            8.,
            Calibration {
                full_scale: 16380,
                reference_white: [256; 3],
                saturation_lux_seconds: 1.,
                color_sample_weights: [2, 5, 2],
            },
        )
        .unwrap()
    }
    fn projection() -> Projection {
        Projection::new(
            Spec {
                focus: "lens".into(),
                focal_length_mm: 18.5,
                subject_distance_mm: 2000.,
                sensor_width_mm: 1.,
                relative_illumination: Illumination::Uniform,
            },
            Default::default(),
        )
        .unwrap()
    }
    #[test]
    fn response_axis_hoisting_matches_scalar_rounding_and_field_extremes() {
        // Readouts above 65536 pixels look response terms up per value.
        for (dims, window) in [
            ([19, 13], [0, 0, 19, 13]),
            ([19, 13], [2, 3, 7, 5]),
            ([300, 230], [0, 0, 300, 230]),
        ] {
            let coordinates = Coordinates::new(dims, dims, window).unwrap();
            let linear: Vec<[u16; 3]> = (0..coordinates.xs.len() * coordinates.ys.len())
                .map(|i| [(i * 811) as u16, (i * 1301) as u16, (i * 2113) as u16])
                .collect();
            for (sensor, distance) in [
                ([23.6, 15.6], Some(18.5)),
                ([1e200, 1e200], Some(1e-200)),
                ([1., 3.], None),
            ] {
                for scale in [[0.; 3], [1e-100, 19371.5, 1e300], [65535.5, 16383.5, 0.5]] {
                    let mut expected = Vec::new();
                    for (row, &y) in coordinates.ys.iter().enumerate() {
                        for (col, &x) in coordinates.xs.iter().enumerate() {
                            let yy = ((y as f64 + 0.5) / dims[1] as f64 - 0.5) * sensor[1];
                            let xx = ((x as f64 + 0.5) / dims[0] as f64 - 0.5) * sensor[0];
                            let field = distance.map_or(1., |d| relative_illumination(xx, yy, d));
                            for c in 0..3 {
                                let value = libm::rint(
                                    f64::from(linear[row * coordinates.xs.len() + col][c])
                                        * scale[c]
                                        / 65535.
                                        * field,
                                )
                                .clamp(0., 16383.)
                                    as u16;
                                expected.extend_from_slice(&value.to_le_bytes());
                            }
                        }
                    }
                    assert_eq!(
                        &*encode_response_coordinates(
                            &linear,
                            scale,
                            dims,
                            sensor,
                            distance,
                            &coordinates
                        ),
                        expected
                    );
                }
            }
        }
    }
    #[test]
    fn binned_sparse_psf_matches_full_readout_with_rectangular_pixels() {
        let p = projection();
        let bytes = (0..64 * 8 * 3)
            .map(|i| ((i * 71 + 23) % 256) as u8)
            .collect();
        let mut base = frame(bytes, 0.00003);
        base.focus_shift_mm = Some(p.subject_shift + 0.03);
        let dims = [16, 5];
        let mut sparse = base.clone();
        p.prepare_binned_windows(&mut sparse, dims, dims, &[[2, 1, 3, 2]])
            .unwrap();
        assert_eq!(sparse.prepared_pixels(), 6);
        let grid = [32, 10];
        let window = [5, 3, 5, 3];
        let offsets = sparse.sample_offsets(grid, window, 2).unwrap();
        for (y, &row) in offsets.rows.iter().enumerate() {
            for (x, &column) in offsets.columns.iter().enumerate() {
                assert_eq!(
                    offsets.sample(row, column),
                    sparse.sample(window[0] + x * 2, window[1] + y, grid)
                );
            }
        }
        assert!(sparse.sample_offsets([16, 5], [0, 0, 1, 1], 1).is_none());

        let mut dense = base.clone();
        p.prepare_binned_windows(&mut dense, dims, dims, &[[0, 0, 16, 5]])
            .unwrap();
        assert_eq!(dense.prepared_pixels(), 80);
        // Includes pure fallback samples outside the requested rectangle.
        for y in 0..5 {
            for x in 0..16 {
                assert_eq!(sparse.sample(x, y, dims), dense.sample(x, y, dims));
            }
        }
        assert_eq!(
            sparse.filtered.as_ref().unwrap().materialize(),
            dense.filtered.as_ref().unwrap().materialize()
        );
        assert_eq!(p.binned.lock().unwrap().len(), 1);
        let scene = p.binned.lock().unwrap()[0].clone();
        let mut changed = base;
        changed.focus_shift_mm = Some(p.subject_shift + 0.07);
        p.prepare_binned_windows(&mut changed, dims, dims, &[[2, 1, 3, 2]])
            .unwrap();
        assert!(Arc::ptr_eq(&scene, &p.binned.lock().unwrap()[0]));
    }
    #[test]
    fn binned_psf_reuses_geometry_across_response_changes_without_mutating_exposures() {
        let p = projection();
        let mut first = frame(
            (0..64 * 8 * 3)
                .map(|i| ((i * 71 + 23) % 256) as u8)
                .collect(),
            0.00003,
        );
        first.focus_shift_mm = Some(p.subject_shift + 0.03);
        let grid = [16, 5];
        let windows = [[0, 0, 16, 5]];
        p.prepare_binned_windows(&mut first, grid, grid, &windows)
            .unwrap();
        let before = first.filtered.as_ref().unwrap().materialize();
        let linear = p.readouts.lock().unwrap()[0].linear.clone();
        let mut second = first.clone();
        second.filtered = None;
        second.response_scale = second.response_scale.map(|v| v * 2.);
        p.prepare_binned_windows(&mut second, grid, grid, &windows)
            .unwrap();
        assert!(Arc::ptr_eq(&linear, &p.readouts.lock().unwrap()[0].linear));
        assert_eq!(first.filtered.as_ref().unwrap().materialize(), before);
        assert_ne!(second.filtered.as_ref().unwrap().materialize(), before);
        assert!(second.covers(grid, &[[2, 1, 3, 2], [14, 4, 2, 1]]));
        let sparse = [[2, 1, 3, 2]];
        let mut third = second.clone();
        third.filtered = None;
        p.prepare_binned_windows(&mut third, grid, grid, &sparse)
            .unwrap();
        assert!(third.covers(grid, &[[3, 1, 1, 1]]));
        assert!(!third.covers(grid, &[[0, 0, 1, 1]]));
        assert!(!third.covers(grid, &[[16, 0, 1, 1]]));
    }
    #[test]
    fn thin_lens_field_illumination_matches_geometry_before_adc_clipping() {
        assert_eq!(relative_illumination(0., 0., 18.5), 1.);
        assert_eq!(relative_illumination(18.5, 0., 18.5), 0.25);
        assert_eq!(relative_illumination(1e200, 0., 1e200), 0.25);
        assert_eq!(relative_illumination(1e-200, 0., 1e-200), 0.25);
        assert!((relative_illumination(18.5, 18.5, 18.5) - 1. / 9.).abs() < 1e-15);
        let values = vec![[65535; 3]; 25];
        let pixels = encode_response(&values, [20000.; 3], [5, 5], 10., Some(2.));
        let sample = |x: usize, y: usize| {
            u16::from_le_bytes(
                pixels[(y * 5 + x) * 6..(y * 5 + x) * 6 + 2]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(sample(2, 2), 16383); // center clips
        assert_eq!(sample(3, 2), 5000); // edge field=1/4 before clipping
        assert_eq!(sample(2, 3), 5000); // square pixels, equal x/y distances
        assert_eq!(sample(4, 2), 800);
        assert_eq!(sample(4, 4), 247);
    }
    #[test]
    fn cosine_fourth_exposure_cache_tracks_image_distance_even_at_equal_blur() {
        let mut spec = projection().spec.clone();
        spec.sensor_width_mm = 23.6;
        spec.relative_illumination = Illumination::CosineFourth;
        let p = Projection::new(spec.clone(), Default::default()).unwrap();
        let base = frame(vec![128; 64 * 8 * 3], 0.0003);
        let mut first = base.clone();
        p.prepare(&mut first, p.subject_shift + 0.02).unwrap();
        let mut second = base.clone();
        p.prepare(&mut second, p.subject_shift - 0.02).unwrap();
        let fresh = Projection::new(spec, Default::default()).unwrap();
        let mut expected = base.clone();
        fresh
            .prepare(&mut expected, p.subject_shift - 0.02)
            .unwrap();
        assert_eq!(
            second.filtered.as_ref().map(|p| p.materialize()),
            expected.filtered.as_ref().map(|p| p.materialize())
        );
        assert_ne!(
            first.filtered.as_ref().map(|p| p.materialize()),
            second.filtered.as_ref().map(|p| p.materialize())
        );
        let mut sharp = base.clone();
        p.prepare(&mut sharp, p.subject_shift).unwrap();
        assert!(sharp.filtered.is_some());
        assert!(sharp.sample(0, 3, [64, 8])[1] < sharp.sample(31, 3, [64, 8])[1]);
        assert!(
            p.prepare(&mut base.clone(), -p.spec.focal_length_mm)
                .is_err()
        );
    }
    #[test]
    fn repeated_exposures_reuse_optics_and_keep_uncached_pixel_results() {
        // Full-raster FNV-1a values from the preceding uncached implementation.
        // These exercise scene identity, dimensions, iris, focus and response
        // changes, including reuse of one RGB allocation at a different shape.
        let bytes: Arc<[u8]> = (0..128u16)
            .flat_map(|i| {
                [
                    (i * 37 % 256) as u8,
                    (255 - i * 13 % 256) as u8,
                    (i * 17 % 256) as u8,
                ]
            })
            .collect::<Vec<_>>()
            .into();
        let p = projection();
        let mut previous: Option<Arc<[u8]>> = None;
        for (index, (width, height, seconds, aperture, shift, reverse, expected)) in [
            (32, 4, 0.0002, 8., 0.25, false, 0xd8439dc782e1a855u64),
            (32, 4, 0.0002, 8., 0.25, false, 0xd8439dc782e1a855),
            (32, 4, 0.0003, 8., 0.25, false, 0x250c110435798bb3),
            // Hold the quantized radius constant across the shape change.
            (
                16,
                8,
                0.0002,
                8.,
                0.5 - p.subject_shift,
                false,
                0x04bc09dc72e83919,
            ),
            (32, 4, 0.0002, 8., 0.25, false, 0xd8439dc782e1a855),
            (32, 4, 0.0002, 8., 0.25, true, 0x83e1ad50ffa54c3f),
            (32, 4, 0.0002, 16., 0.25, false, 0xf460a6a6bc55a381),
            (32, 4, 0.0002, 8., 0.27, false, 0x91494882efc4e2c0),
        ]
        .into_iter()
        .enumerate()
        {
            let pixels = if reverse {
                bytes.iter().rev().copied().collect::<Vec<_>>().into()
            } else {
                bytes.clone()
            };
            let input = Arc::new(Frame::new(width, height, FramePixels::Rgb24(pixels)).unwrap());
            let mut image = ExposedFrame::from_frame(
                input,
                Exposure {
                    seconds,
                    analog_gain: 1.5,
                },
                aperture,
                Calibration {
                    full_scale: 16380,
                    reference_white: [606, 302, 421],
                    saturation_lux_seconds: 1.,
                    color_sample_weights: [2, 5, 2],
                },
            )
            .unwrap();
            p.prepare(&mut image, shift).unwrap();
            let pixels = image.filtered.as_ref().unwrap().materialize();
            let hash = pixels.iter().fold(0xcbf29ce484222325u64, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(hash, expected);
            if index == 0 {
                previous = Some(pixels.clone());
            } else if index == 1 {
                assert!(Arc::ptr_eq(previous.as_ref().unwrap(), &pixels));
            }
        }
    }
    #[test]
    fn physical_focus_changes_pixels_and_preserves_flat_light_without_guest_decisions() {
        let p = projection();
        let source = (0..64 * 8)
            .flat_map(|i| [if i % 8 < 4 { 30 } else { 190 }; 3])
            .collect::<Vec<_>>();
        let original = frame(source, 0.0001);
        let energy = |f: &ExposedFrame| {
            (1..63)
                .map(|x| {
                    let d = i64::from(f.sample(x, 3, [64, 8])[1])
                        - i64::from(f.sample(x - 1, 3, [64, 8])[1]);
                    d * d
                })
                .sum::<i64>()
        };
        let mut near = original.clone();
        p.prepare(&mut near, p.subject_shift).unwrap();
        assert_eq!(energy(&near), energy(&original));
        let mut far = original.clone();
        p.prepare(&mut far, p.subject_shift + 0.3).unwrap();
        assert!(energy(&far) < energy(&near) / 4);
        assert!(Arc::ptr_eq(&near.frame, &original.frame));
        let mut flat = frame(vec![128; 64 * 8 * 3], 0.0001);
        let expected = flat.sample(0, 0, [64, 8]);
        p.prepare(&mut flat, p.subject_shift + 0.3).unwrap();
        for x in 0..64 {
            for c in 0..3 {
                assert!(
                    (i32::from(flat.sample(x, 3, [64, 8])[c]) - i32::from(expected[c])).abs() <= 1
                );
            }
        }
    }
    #[test]
    fn fractional_defocus_has_no_artificial_one_pixel_plateau_and_refocus_clears_blur() {
        let p = projection();
        let original = frame(
            (0..64 * 8)
                .flat_map(|i| [if i % 8 < 4 { 30 } else { 190 }; 3])
                .collect(),
            0.0001,
        );
        let energy = |f: &ExposedFrame| {
            (1..63)
                .map(|x| {
                    let d = i64::from(f.sample(x, 3, [64, 8])[1])
                        - i64::from(f.sample(x - 1, 3, [64, 8])[1]);
                    d * d
                })
                .sum::<i64>()
        };
        let mut values = vec![];
        for delta in [0., 0.01, 0.02, 0.03] {
            let mut f = original.clone();
            p.prepare(&mut f, p.subject_shift + delta).unwrap();
            values.push(energy(&f));
            p.prepare(&mut f, p.subject_shift).unwrap();
            assert_eq!(energy(&f), energy(&original));
        }
        assert!(values.windows(2).all(|v| v[0] > v[1]), "{values:?}");
    }
    #[test]
    fn optical_blur_averages_linear_radiance_before_adc_clipping() {
        let p = projection();
        let mut f = frame(
            (0..64 * 8)
                .flat_map(|i| [if i % 2 == 0 { 0 } else { 255 }; 3])
                .collect(),
            2. * 32. / (10000. * core::f64::consts::PI),
        );
        p.prepare(&mut f, p.subject_shift + 0.1).unwrap();
        assert_eq!(f.sample(2, 3, [64, 8]), [16383; 3]);
        assert!(f.sample(3, 3, [64, 8])[1] > 10_000);
    }
    #[test]
    fn filtered_exposure_roundtrip_retains_pixels_and_refuses_invalid_blobs() {
        let projection = projection();
        let mut image = frame(
            (0..64 * 8)
                .flat_map(|i| [if i % 8 < 4 { 30 } else { 190 }; 3])
                .collect(),
            0.0001,
        );
        projection
            .prepare(&mut image, projection.subject_shift + 0.03)
            .unwrap();
        assert!(image.filtered.is_some());
        let before = image.fingerprint();
        let saved = image.checkpoint().unwrap();
        assert_eq!(image.fingerprint(), before);
        let restored = ExposedFrame::from_checkpoint(&saved).unwrap();
        assert_eq!(restored.fingerprint(), before);
        assert!(Arc::ptr_eq(
            &restored.filtered.as_ref().unwrap().materialize(),
            &saved.blobs()[0]
        ));
        assert_eq!(restored.sample(9, 3, [64, 8]), image.sample(9, 3, [64, 8]));
        let malformed = saved.clone().with_blobs(vec![Arc::from([0u8; 7])]);
        assert!(ExposedFrame::from_checkpoint(&malformed).is_err());
        let mut invalid = saved.blobs()[0].to_vec();
        invalid[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(ExposedFrame::from_checkpoint(&saved.with_blobs(vec![invalid.into()])).is_err());
        assert_eq!(image.fingerprint(), before);
    }
}

#[cfg(test)]
#[path = "optics/readout_tests.rs"]
mod readout_tests;
