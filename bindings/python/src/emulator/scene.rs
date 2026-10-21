//! Declared virtual post-ISP image and parameterized analytic statistics.
//!
//! The analytic RGB scene uses normalized 1024x768 coordinates. Output pixels
//! sample a declared source crop by nearest-neighbor pixel centers. RGB becomes
//! BT.601-limited YCbCr using exact decimal Kr=.299/Kb=.114 and integer rational
//! arithmetic, round-nearest with ties away from zero. Each 4:2:2 pair stores
//! [Y0,Y1,mean(Cb0,Cb1),mean(Cr0,Cr1)] with half-up chroma averaging.
//! The virtual_scene_histogram bins use limited-range Y at every second horizontal
//! crop pixel, starting at crop_x, on every row. Crops require an even width.
//!
//! These functions have no machine, memory, MMIO, queue, callback or IRQ access.

use pyo3::exceptions::{PyMemoryError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyInt};

pub const PROFILE: &str = "declared-rgb-bars-ramp-moving-square-bt601-limited-v1";
const MAX_DIMENSION: u64 = 8192;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_HISTOGRAM_SAMPLES: u64 = 16 * 1024 * 1024;

#[derive(Debug, PartialEq)]
enum SceneError {
    Invalid(&'static str),
    Allocation,
}

impl From<SceneError> for PyErr {
    fn from(error: SceneError) -> Self {
        match error {
            SceneError::Invalid(message) => PyValueError::new_err(message),
            SceneError::Allocation => PyMemoryError::new_err("virtual scene allocation failed"),
        }
    }
}

#[derive(Clone, Copy)]
struct Scene {
    width: u64,
    height: u64,
    frame: u64,
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Crop {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

fn dimension(value: u64) -> Result<(), SceneError> {
    if value == 0 || value > MAX_DIMENSION {
        return Err(SceneError::Invalid("dimensions must be in 1..8192"));
    }
    Ok(())
}

fn validate_crop(scene: Scene, crop: Crop) -> Result<(), SceneError> {
    dimension(scene.width)?;
    dimension(scene.height)?;
    dimension(crop.width)?;
    dimension(crop.height)?;
    if crop
        .x
        .checked_add(crop.width)
        .is_none_or(|end| end > scene.width)
        || crop
            .y
            .checked_add(crop.height)
            .is_none_or(|end| end > scene.height)
    {
        return Err(SceneError::Invalid(
            "crop must be wholly inside the declared source raster",
        ));
    }
    Ok(())
}

/// Pure RGB pattern. All movement repeats after 1024 frames; frame arithmetic
/// is reduced before multiplication so every u64 frame index remains valid.
fn rgb(scene: Scene, x: u64, y: u64) -> [u8; 3] {
    let u = x * 1024 / scene.width;
    let v = y * 768 / scene.height;
    let phase = scene.frame % 1024;
    let moving_x = (u + phase * 13) % 1024;
    let mut color = if v < 384 {
        const BARS: [[u8; 3]; 8] = [
            [255, 255, 255],
            [255, 255, 0],
            [0, 255, 255],
            [0, 255, 0],
            [255, 0, 255],
            [255, 0, 0],
            [0, 0, 255],
            [0, 0, 0],
        ];
        BARS[(moving_x / 128) as usize]
    } else {
        let level = (moving_x * 255 / 1023) as u8;
        [level, level, level]
    };
    // A visible orange square crosses the lower-half ramp. Wrapping coordinates
    // keep its area consistent at boundaries; a white border marks its extent.
    let square_x = (u + 1024 - (phase * 7 % 1024)) % 1024;
    let square_y = (v + 768 - (448 + phase * 3 % 192)) % 768;
    if square_x < 160 && square_y < 128 {
        color = if square_x < 8 || square_y < 8 || square_x >= 152 || square_y >= 120 {
            [255, 255, 255]
        } else {
            [255, 96, 16]
        };
    }
    color
}

fn round_nearest(numerator: i64, denominator: i64) -> i64 {
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

fn ycbcr(color: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = color.map(i64::from);
    let luma = 299 * r + 587 * g + 114 * b;
    let y = 16 + round_nearest(219 * luma, 255_000);
    let cb = 128 + round_nearest(224 * (1000 * b - luma), 255 * 1772);
    let cr = 128 + round_nearest(224 * (1000 * r - luma), 255 * 1402);
    [
        y.clamp(16, 235) as u8,
        cb.clamp(16, 240) as u8,
        cr.clamp(16, 240) as u8,
    ]
}

fn pair(first: [u8; 3], second: [u8; 3]) -> [u8; 4] {
    let a = ycbcr(first);
    let b = ycbcr(second);
    [
        a[0],
        b[0],
        ((u16::from(a[1]) + u16::from(b[1]) + 1) / 2) as u8,
        ((u16::from(a[2]) + u16::from(b[2]) + 1) / 2) as u8,
    ]
}

fn output(
    scene: Scene,
    crop: Crop,
    width: u64,
    height: u64,
    stride: Option<u64>,
) -> Result<Vec<u8>, SceneError> {
    output_rgb(scene, crop, width, height, stride, |x, y| rgb(scene, x, y))
}

fn output_rgb(
    scene: Scene,
    crop: Crop,
    width: u64,
    height: u64,
    stride: Option<u64>,
    pixel: impl Fn(u64, u64) -> [u8; 3],
) -> Result<Vec<u8>, SceneError> {
    output_pairs(scene, crop, width, height, stride, |x0,x1,y| pair(pixel(x0,y),pixel(x1,y)))
}

fn output_pairs(scene: Scene, crop: Crop, width: u64, height: u64, stride: Option<u64>,
                pixels: impl Fn(u64,u64,u64)->[u8;4]) -> Result<Vec<u8>, SceneError> {
    validate_crop(scene, crop)?;
    dimension(width)?;
    dimension(height)?;
    if width % 2 != 0 {
        return Err(SceneError::Invalid("YYCbCr output width must be even"));
    }
    let row_bytes = width
        .checked_mul(2)
        .ok_or(SceneError::Invalid("row size overflow"))?;
    let stride = stride.unwrap_or(row_bytes);
    if stride < row_bytes {
        return Err(SceneError::Invalid(
            "stride must contain the complete packed row",
        ));
    }
    let length = stride
        .checked_mul(height)
        .filter(|&n| n <= MAX_OUTPUT_BYTES)
        .ok_or(SceneError::Invalid(
            "output span must be at most 64MiB without overflow",
        ))?;
    let length = usize::try_from(length)
        .map_err(|_| SceneError::Invalid("output span is not addressable"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| SceneError::Allocation)?;
    result.resize(length, 0); // Every padding byte, including the final row, is zero.
    for y in 0..height {
        let sy = crop.y + (2 * y + 1) * crop.height / (2 * height);
        for x in (0..width).step_by(2) {
            let sx0 = crop.x + (2 * x + 1) * crop.width / (2 * width);
            let sx1 = crop.x + (2 * (x + 1) + 1) * crop.width / (2 * width);
            let word = pixels(sx0, sx1, sy);
            let offset = (y * stride + x * 2) as usize;
            result[offset..offset + 4].copy_from_slice(&word);
        }
    }
    Ok(result)
}

fn histogram(scene: Scene, crop: Crop) -> Result<Vec<u8>, SceneError> {
    validate_crop(scene, crop)?;
    if crop.width % 2 != 0 {
        return Err(SceneError::Invalid(
            "half-horizontal histogram crop width must be even",
        ));
    }
    let samples = (crop.width / 2)
        .checked_mul(crop.height)
        .filter(|&n| n <= MAX_HISTOGRAM_SAMPLES)
        .ok_or(SceneError::Invalid(
            "histogram must contain at most 16Mi samples without overflow",
        ))?;
    // The cap also bounds every accumulator below u32::MAX. Count exact input
    // raster samples, rather than histogramming either resampled video output.
    let mut bins = [0u32; 256];
    for y in 0..crop.height {
        for x in (0..crop.width).step_by(2) {
            let bin = ycbcr(rgb(scene, crop.x + x, crop.y + y))[0] as usize;
            bins[bin] += 1;
        }
    }
    debug_assert_eq!(bins.iter().map(|&n| u64::from(n)).sum::<u64>(), samples);
    let mut result = Vec::with_capacity(1024);
    for bin in bins {
        result.extend_from_slice(&bin.to_le_bytes());
    }
    Ok(result)
}

fn integer(value: &Bound<'_, PyAny>, name: &'static str) -> PyResult<u64> {
    if !value.is_exact_instance_of::<PyInt>() {
        return Err(PyValueError::new_err(format!(
            "{name} must be an actual Python int, not bool or a coercible value"
        )));
    }
    value
        .extract::<u64>()
        .map_err(|_| PyValueError::new_err(format!("{name} must be in 0..2^64-1")))
}

/// Immutable host RGB24 raster mapped over a device's declared source grid.
/// A frame owns its pixels, so later webcam buffers cannot alter an exposure.
#[pyclass(frozen, module = "fujimod._native.emulator")]
pub(crate) struct HostImage {
    width: u64,
    height: u64,
    pixels: Vec<u8>,
}

#[pymethods]
impl HostImage {
    #[new]
    #[pyo3(signature = (width: "int", height: "int", rgb))]
    fn new(width: &Bound<'_, PyAny>, height: &Bound<'_, PyAny>, rgb: &Bound<'_, PyBytes>) -> PyResult<Self> {
        let width = integer(width, "width")?;
        let height = integer(height, "height")?;
        dimension(width)?;
        dimension(height)?;
        let length = width * height * 3;
        if length > MAX_OUTPUT_BYTES || rgb.as_bytes().len() as u64 != length {
            return Err(PyValueError::new_err("RGB24 requires exactly width*height*3 bytes, at most 64MiB"));
        }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(length as usize).map_err(|_| SceneError::Allocation)?;
        pixels.extend_from_slice(rgb.as_bytes());
        Ok(Self { width, height, pixels })
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (width: "int", height: "int", stride: "int | None", source_width: "int", source_height: "int", crop_x: "int", crop_y: "int", crop_width: "int", crop_height: "int"))]
    fn yycbcr<'py>(
        &self,
        py: Python<'py>,
        width: &Bound<'py, PyAny>,
        height: &Bound<'py, PyAny>,
        stride: &Bound<'py, PyAny>,
        source_width: &Bound<'py, PyAny>,
        source_height: &Bound<'py, PyAny>,
        crop_x: &Bound<'py, PyAny>,
        crop_y: &Bound<'py, PyAny>,
        crop_width: &Bound<'py, PyAny>,
        crop_height: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let scene = Scene { width: integer(source_width, "source_width")?,
            height: integer(source_height, "source_height")?, frame: 0 };
        let crop = Crop { x: integer(crop_x, "crop_x")?, y: integer(crop_y, "crop_y")?,
            width: integer(crop_width, "crop_width")?, height: integer(crop_height, "crop_height")? };
        let width = integer(width, "width")?;
        let height = integer(height, "height")?;
        let stride = if stride.is_none() { None } else { Some(integer(stride, "stride")?) };
        let bytes = py.detach(|| output_rgb(scene, crop, width, height, stride, |x, y| {
            let sx = ((2 * x + 1) * self.width / (2 * scene.width)).min(self.width - 1);
            let sy = ((2 * y + 1) * self.height / (2 * scene.height)).min(self.height - 1);
            let offset = ((sy * self.width + sx) * 3) as usize;
            [self.pixels[offset], self.pixels[offset + 1], self.pixels[offset + 2]]
        }))?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (film, width: "int", height: "int", stride: "int | None", source_width: "int", source_height: "int", crop_x: "int", crop_y: "int", crop_width: "int", crop_height: "int"))]
    fn film_yycbcr<'py>(
        &self,
        py: Python<'py>,
        film: &crate::emulator::film::FilmStage,
        width: &Bound<'py, PyAny>,
        height: &Bound<'py, PyAny>,
        stride: &Bound<'py, PyAny>,
        source_width: &Bound<'py, PyAny>,
        source_height: &Bound<'py, PyAny>,
        crop_x: &Bound<'py, PyAny>,
        crop_y: &Bound<'py, PyAny>,
        crop_width: &Bound<'py, PyAny>,
        crop_height: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let scene = Scene { width: integer(source_width, "source_width")?,
            height: integer(source_height, "source_height")?, frame: 0 };
        let crop = Crop { x: integer(crop_x, "crop_x")?, y: integer(crop_y, "crop_y")?,
            width: integer(crop_width, "crop_width")?, height: integer(crop_height, "crop_height")? };
        let width = integer(width, "width")?;
        let height = integer(height, "height")?;
        let stride = if stride.is_none() { None } else { Some(integer(stride, "stride")?) };
        validate_crop(scene, crop)?;
        dimension(width)?;
        dimension(height)?;
        let sample = |x: u64,y: u64| {
            let sx = ((2 * x + 1) * self.width / (2 * scene.width)).min(self.width - 1);
            let sy = ((2 * y + 1) * self.height / (2 * scene.height)).min(self.height - 1);
            let offset = ((sy * self.width + sx) * 3) as usize;
            [self.pixels[offset], self.pixels[offset + 1], self.pixels[offset + 2]]
        };
        let bytes = py.detach(|| {
            // Pointwise color commutes with nearest source-grid sampling. A
            // phone webcam raster is smaller than its two display outputs:
            // develop each input pixel once instead of repeating soft math
            // for every enlarged output pixel. Large photos sample directly.
            let developed: Option<Vec<[u8;3]>> = (self.width*self.height < width*height).then(||
                self.pixels.chunks_exact(3).map(|p| film.color([p[0],p[1],p[2]])).collect());
            let color = |x: u64,y: u64| {
                if let Some(ref raster) = developed {
                    let sx=((2*x+1)*self.width/(2*scene.width)).min(self.width-1);
                    let sy=((2*y+1)*self.height/(2*scene.height)).min(self.height-1);
                    raster[(sy*self.width+sx) as usize]
                } else {film.color(sample(x,y))}
            };
            output_pairs(scene,crop,width,height,stride,
                |x0,x1,y| crate::emulator::film::FilmStage::pack(color(x0,y),color(x1,y)))
        })?;
        Ok(PyBytes::new(py, &bytes))
    }
}

/// Return packed bytes only. Stride may be None for a tightly packed raster;
/// every other argument is an explicit, nonnegative, actual Python integer.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (width: "int", height: "int", stride: "int | None", frame: "int", source_width: "int", source_height: "int", crop_x: "int", crop_y: "int", crop_width: "int", crop_height: "int"))]
pub fn virtual_scene_yycbcr<'py>(
    py: Python<'py>,
    width: &Bound<'py, PyAny>,
    height: &Bound<'py, PyAny>,
    stride: &Bound<'py, PyAny>,
    frame: &Bound<'py, PyAny>,
    source_width: &Bound<'py, PyAny>,
    source_height: &Bound<'py, PyAny>,
    crop_x: &Bound<'py, PyAny>,
    crop_y: &Bound<'py, PyAny>,
    crop_width: &Bound<'py, PyAny>,
    crop_height: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let scene = Scene {
        width: integer(source_width, "source_width")?,
        height: integer(source_height, "source_height")?,
        frame: integer(frame, "frame")?,
    };
    let crop = Crop {
        x: integer(crop_x, "crop_x")?,
        y: integer(crop_y, "crop_y")?,
        width: integer(crop_width, "crop_width")?,
        height: integer(crop_height, "crop_height")?,
    };
    let stride = if stride.is_none() {
        None
    } else {
        Some(integer(stride, "stride")?)
    };
    let width = integer(width, "width")?;
    let height = integer(height, "height")?;
    let bytes = py.detach(move || output(scene, crop, width, height, stride))?;
    Ok(PyBytes::new(py, &bytes))
}

/// Return256 little-endian U32 bins under the declared half-horizontal law.
#[pyfunction]
#[pyo3(signature = (source_width: "int", source_height: "int", crop_x: "int", crop_y: "int", crop_width: "int", crop_height: "int", frame: "int"))]
pub fn virtual_scene_histogram<'py>(
    py: Python<'py>,
    source_width: &Bound<'py, PyAny>,
    source_height: &Bound<'py, PyAny>,
    crop_x: &Bound<'py, PyAny>,
    crop_y: &Bound<'py, PyAny>,
    crop_width: &Bound<'py, PyAny>,
    crop_height: &Bound<'py, PyAny>,
    frame: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let scene = Scene {
        width: integer(source_width, "source_width")?,
        height: integer(source_height, "source_height")?,
        frame: integer(frame, "frame")?,
    };
    let crop = Crop {
        x: integer(crop_x, "crop_x")?,
        y: integer(crop_y, "crop_y")?,
        width: integer(crop_width, "crop_width")?,
        height: integer(crop_height, "crop_height")?,
    };
    let bytes = py.detach(move || histogram(scene, crop))?;
    Ok(PyBytes::new(py, &bytes))
}

pub(crate) const STATISTICS_PROFILE: &str = "declared-analytic-statistics-quadrature-v1";

struct SceneFrameNumber(u64);

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(transparent)]
struct CellCount(usize);

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SamplingGrid {
    cells: Crop,
    columns: CellCount,
    rows: CellCount,
    gap_x: u64,
    gap_y: u64,
}

impl SamplingGrid {
    fn cell(self, column: usize, row: usize) -> Crop {
        Crop {
            x: self.cells.x + column as u64 * (self.cells.width + self.gap_x),
            y: self.cells.y + row as u64 * (self.cells.height + self.gap_y),
            ..self.cells
        }
    }

    fn extent(self) -> Crop {
        Crop {
            width: self.columns.0 as u64 * self.cells.width
                + (self.columns.0 - 1) as u64 * self.gap_x,
            height: self.rows.0 as u64 * self.cells.height
                + (self.rows.0 - 1) as u64 * self.gap_y,
            ..self.cells
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SamplingProgram {
    color: SamplingGrid,
    color_regions: [Crop; 10],
    contrast: SamplingGrid,
    contrast_regions: [Crop; 10],
    phase: SamplingGrid,
    totals: Crop,
    columns: SamplingGrid,
    rows: SamplingGrid,
    histogram: Option<Crop>,
}

impl SamplingProgram {
    fn scene(&self, frame: SceneFrameNumber) -> Result<Scene, SceneError> {
        if self.columns.rows.0 != 1 || self.rows.columns.0 != 1 {
            return Err(SceneError::Invalid("axis sampling grids require one cell on the other axis"));
        }
        for grid in [self.color, self.contrast, self.phase, self.columns, self.rows] {
            if !(1..=16).contains(&grid.columns.0) || !(1..=16).contains(&grid.rows.0)
                || (grid.columns.0 > 1 && grid.gap_x > MAX_DIMENSION)
                || (grid.rows.0 > 1 && grid.gap_y > MAX_DIMENSION) {
                return Err(SceneError::Invalid("statistics grids require 1..16 cells per axis and bounded gaps"));
            }
            validate_region(grid.cells)?;
        }
        let extents = [self.color.extent(), self.contrast.extent(), self.phase.extent(),
            self.totals, self.columns.extent(), self.rows.extent()];
        let mut width = 1;
        let mut height = 1;
        for region in extents.iter().chain(&self.color_regions)
            .chain(&self.contrast_regions).chain(self.histogram.iter()) {
            validate_region(*region)?;
            width = width.max(region.x + region.width);
            height = height.max(region.y + region.height);
        }
        dimension(width)?;
        dimension(height)?;
        Ok(Scene { width, height, frame: frame.0 })
    }
}

fn validate_region(region: Crop) -> Result<(), SceneError> {
    dimension(region.width)?;
    dimension(region.height)?;
    if region.x > MAX_DIMENSION || region.y > MAX_DIMENSION {
        return Err(SceneError::Invalid("statistics region origin exceeds the scene bound"));
    }
    Ok(())
}

struct RegionMeasurements {
    colors: [u64; 3],
    luma: u64,
    contrast: [u64; 2],
    mean: u16,
    population: u64,
}

// This numerical law is unmeasured; phase splitting and row/column field meanings remain unrecovered.
fn measure(scene: Scene, region: Crop) -> RegionMeasurements {
    let columns = region.width.min(8);
    let rows = region.height.min(8);
    let samples = columns * rows;
    let population = region.width * region.height;
    let mut colors = [0u64; 3];
    let mut luma = 0u64;
    let mut contrast = [0u64; 2];
    let mut row_luma = [0i64; 8];
    for row in 0..rows {
        let mut previous = 0i64;
        for column in 0..columns {
            let x = region.x + ((2 * column + 1) * region.width / (2 * columns));
            let y = region.y + ((2 * row + 1) * region.height / (2 * rows));
            let color = rgb(scene, x, y);
            let value = i64::from(ycbcr(color)[0]);
            for (sum, component) in colors.iter_mut().zip(color) {
                *sum += u64::from(component) * 64;
            }
            luma += value as u64 * 64;
            if column != 0 {
                contrast[0] += (value - previous).unsigned_abs().pow(2);
            }
            if row != 0 {
                contrast[1] += (value - row_luma[column as usize]).unsigned_abs().pow(2);
            }
            previous = value;
            row_luma[column as usize] = value;
        }
    }
    RegionMeasurements {
        colors: std::array::from_fn(|channel| {
            let numerator = [2, 5, 2][channel];
            (population * numerator / 9) * colors[channel] / samples
        }),
        luma: population * luma / samples,
        mean: (luma / samples) as u16,
        contrast: contrast.map(|energy| energy * population / samples),
        population,
    }
}

fn u32_word(value: u64) -> u32 {
    value.min(u64::from(u32::MAX)) as u32
}

fn words32<'py>(
    py: Python<'py>,
    words: &[u32],
) -> PyResult<Bound<'py, PyBytes>> {
    PyBytes::new_with(py, words.len() * 4, |output| {
        for (bytes, word) in output.chunks_exact_mut(4).zip(words) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        Ok(())
    })
}

fn words64<'py>(
    py: Python<'py>,
    words: &[u64],
) -> PyResult<Bound<'py, PyBytes>> {
    PyBytes::new_with(py, words.len() * 8, |output| {
        for (bytes, word) in output.chunks_exact_mut(8).zip(words) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        Ok(())
    })
}

type StatisticsBytes<'py> = (
    Bound<'py, PyBytes>, Bound<'py, PyBytes>, Bound<'py, PyBytes>,
    Bound<'py, PyBytes>, Bound<'py, PyBytes>, Bound<'py, PyBytes>,
    Bound<'py, PyBytes>, Bound<'py, PyBytes>, Bound<'py, PyBytes>,
    Option<Bound<'py, PyBytes>>,
);

#[pyfunction]
#[pyo3(signature = (frame: "int", configuration))]
pub(crate) fn virtual_scene_statistics<'py>(
    py: Python<'py>,
    frame: &Bound<'py, PyAny>,
    configuration: &str,
) -> PyResult<StatisticsBytes<'py>> {
    let program: SamplingProgram = serde_json::from_str(configuration)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let frame = integer(frame, "frame")?;
    if frame == 0 {
        return Err(PyValueError::new_err("statistics frame must be positive"));
    }
    let scene = program.scene(SceneFrameNumber(frame))?;
    let mut contrast = [[0u32; 256]; 2];
    let mut colors = [[0u32; 256]; 4];
    let mut phase = [0u32; 256];
    let mut scalar = [0u32; 256];
    for row in 0..program.color.rows.0 {
        for column in 0..program.color.columns.0 {
            let measurement = measure(scene, program.color.cell(column, row));
            for channel in 0..4 {
                colors[channel][row * 16 + column] = u32_word(measurement.colors[[0, 1, 2, 1][channel]]);
            }
        }
    }
    for row in 0..program.contrast.rows.0 {
        for column in 0..program.contrast.columns.0 {
            let measurement = measure(scene, program.contrast.cell(column, row));
            for channel in 0..2 {
                contrast[channel][row * 16 + column] = u32_word(measurement.contrast[channel]);
            }
        }
    }
    for row in 0..program.phase.rows.0 {
        for column in 0..program.phase.columns.0 {
            let measurement = measure(scene, program.phase.cell(column, row));
            let mean = u32::from(measurement.mean);
            phase[row * 16 + column] = (mean << 16) | mean;
            scalar[row * 16 + column] = mean;
        }
    }
    let bands = program.contrast_regions.map(|region| {
        let measurement = measure(scene, region);
        u64::from(u32_word(measurement.contrast[0] + measurement.contrast[1]))
    });
    let color_regions = program.color_regions.map(|region| {
        let measurement = measure(scene, region);
        [measurement.colors[0], measurement.colors[1], measurement.colors[2], measurement.colors[1]]
    });
    let total = measure(scene, program.totals);
    let totals = [u32_word(total.luma), u32_word(total.luma), u32_word(total.luma), u32_word(total.luma),
        u32_word(total.population), u32_word(total.population), u32_word(total.population), u32_word(total.population)];
    let mut columns = [[0u32; 16]; 8];
    let mut rows = [[0u32; 16]; 8];
    for column in 0..program.columns.columns.0 {
        let measurement = measure(scene, program.columns.cell(column, 0));
        for channel in 0..8 {
            columns[channel][column] = u32_word(if channel < 4 { measurement.luma } else { measurement.population });
        }
    }
    for row in 0..program.rows.rows.0 {
        let measurement = measure(scene, program.rows.cell(0, row));
        for channel in 0..8 {
            rows[channel][row] = u32_word(if channel < 4 { measurement.luma } else { measurement.population });
        }
    }
    let histogram = if let Some(region) = program.histogram {
        let mut bins = [0u32; 256];
        let columns = region.width.min(8);
        let rows = region.height.min(8);
        let samples = columns * rows;
        let population = region.width * region.height;
        for row in 0..rows {
            for column in 0..columns {
                let x = region.x + ((2 * column + 1) * region.width / (2 * columns));
                let y = region.y + ((2 * row + 1) * region.height / (2 * rows));
                let index = row * columns + column;
                let weight = population / samples + u64::from(index < population % samples);
                bins[ycbcr(rgb(scene, x, y))[0] as usize] += weight as u32;
            }
        }
        Some(words32(py, &bins)?)
    } else {
        None
    };
    Ok((
        words32(py, contrast.as_flattened())?,
        words64(py, &bands)?,
        words32(py, colors.as_flattened())?,
        words64(py, color_regions.as_flattened())?,
        words32(py, &phase)?,
        words32(py, &scalar)?,
        words32(py, &totals)?,
        words32(py, columns.as_flattened())?,
        words32(py, rows.as_flattened())?,
        histogram,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(width: u64, height: u64, frame: u64) -> Scene {
        Scene {
            width,
            height,
            frame,
        }
    }
    fn crop(width: u64, height: u64) -> Crop {
        Crop {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn known_bt601_limited_pixels_and_neutral_pair() {
        assert_eq!(ycbcr([0, 0, 0]), [16, 128, 128]);
        assert_eq!(ycbcr([255, 255, 255]), [235, 128, 128]);
        assert_eq!(ycbcr([255, 0, 0]), [81, 90, 240]);
        assert_eq!(ycbcr([0, 255, 0]), [145, 54, 34]);
        assert_eq!(ycbcr([0, 0, 255]), [41, 240, 110]);
        assert_eq!(pair([0, 0, 0], [255, 255, 255]), [16, 235, 128, 128]);
    }

    #[test]
    fn packed_order_resampling_and_padding_are_exact() {
        let s = scene(8, 4, 0);
        let tight = output(s, crop(8, 4), 8, 4, None).unwrap();
        let padded = output(s, crop(8, 4), 8, 4, Some(20)).unwrap();
        assert_eq!(tight.len(), 64);
        assert_eq!(padded.len(), 80);
        assert_eq!(&tight[..4], &[235, 210, 72, 137]); // White/yellow, shared chroma.
        for row in 0..4 {
            assert_eq!(
                &padded[row * 20..row * 20 + 16],
                &tight[row * 16..row * 16 + 16]
            );
            assert_eq!(&padded[row * 20 + 16..row * 20 + 20], &[0; 4]);
        }
        // Nearest-neighbor pixel centers select source pixels(2,2)/(6,2),
        // gray levels63/191 in the lower-half ramp.
        let downsample = output(s, crop(8, 4), 2, 1, None).unwrap();
        assert_eq!(downsample, [70, 180, 128, 128]);
    }

    #[test]
    fn scene_is_deterministic_moving_and_frame_overflow_safe() {
        let a = output(scene(256, 192, 0), crop(256, 192), 128, 96, None).unwrap();
        assert_eq!(
            a,
            output(scene(256, 192, 0), crop(256, 192), 128, 96, None).unwrap()
        );
        assert_ne!(
            a,
            output(scene(256, 192, 1), crop(256, 192), 128, 96, None).unwrap()
        );
        assert_eq!(
            a,
            output(scene(256, 192, 1024), crop(256, 192), 128, 96, None).unwrap()
        );
        assert!(output(scene(2, 1, u64::MAX), crop(2, 1), 2, 1, None).is_ok());
    }

    #[test]
    fn histogram_known_bins_and_total_use_half_horizontal_input() {
        let bytes = histogram(
            scene(8, 4, 0),
            Crop {
                x: 0,
                y: 0,
                width: 8,
                height: 1,
            },
        )
        .unwrap();
        assert_eq!(bytes.len(), 1024);
        let bins: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(bins.iter().sum::<u32>(), 4);
        for bin in [235, 170, 106, 41] {
            assert_eq!(bins[bin], 1);
        }
        let roi = Crop {
            x: 36,
            y: 39,
            width: 2400,
            height: 800,
        };
        let bytes = histogram(scene(2496, 853, 1), roi).unwrap();
        let sum: u64 = bytes
            .chunks_exact(4)
            .map(|b| u64::from(u32::from_le_bytes(b.try_into().unwrap())))
            .sum();
        assert_eq!(sum, 960000);
        assert_eq!(bytes, histogram(scene(2496, 853, 1), roi).unwrap());
        assert_ne!(bytes, histogram(scene(2496, 853, 2), roi).unwrap());
        let current_roi = Crop {
            x: 12,
            y: 31,
            width: 2448,
            height: 809,
        };
        let bytes = histogram(scene(2496, 853, 1), current_roi).unwrap();
        let sum: u64 = bytes
            .chunks_exact(4)
            .map(|b| u64::from(u32::from_le_bytes(b.try_into().unwrap())))
            .sum();
        assert_eq!(sum, 990216);
    }

    #[test]
    fn invalid_geometry_spans_and_overflow_are_rejected() {
        let s = scene(8, 4, 0);
        let c = crop(8, 4);
        for (w, h, stride) in [
            (0, 1, None),
            (3, 1, None),
            (2, 0, None),
            (8194, 1, None),
            (8, 4, Some(15)),
            (8, 4, Some(u64::MAX)),
            (8192, 8192, None),
        ] {
            assert!(output(s, c, w, h, stride).is_err());
        }
        assert!(output(scene(0, 4, 0), c, 8, 4, None).is_err());
        assert!(output(s, Crop { x: u64::MAX, ..c }, 8, 4, None).is_err());
        assert!(
            output(
                s,
                Crop {
                    y: 3,
                    height: 2,
                    ..c
                },
                8,
                4,
                None
            )
            .is_err()
        );
        assert!(histogram(s, Crop { width: 3, ..c }).is_err());
        assert!(histogram(scene(8192, 8192, 0), crop(8192, 8192)).is_err());
    }
}
