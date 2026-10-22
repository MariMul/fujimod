use super::{
    encode_response,
    scene::{Scene, Span, blend},
};
use std::sync::Arc;

pub(super) struct Coordinates {
    pub key: ([usize; 2], [usize; 4]),
    pub xs: Vec<usize>,
    pub ys: Vec<usize>,
    x_index: Vec<u16>,
    y_index: Vec<u16>,
}
impl Coordinates {
    pub fn new(
        dimensions: [usize; 2],
        grid: [usize; 2],
        window: [usize; 4],
    ) -> Result<Self, String> {
        Self::validate(grid, window)?;
        let [left, top, width, height] = window;
        let axis = |begin: usize, count: usize, grid: usize, source: usize| {
            let mut points = Vec::with_capacity(count.min(source));
            let mut index = vec![u16::MAX; source];
            for p in begin..begin + count {
                let sample = ((2 * p + 1) * source / (2 * grid)).min(source - 1);
                if points.last() != Some(&sample) {
                    index[sample] = points.len() as u16;
                    points.push(sample);
                }
            }
            (points, index)
        };
        let (mut xs, mut x_index) = axis(left, width, grid[0], dimensions[0]);
        let (mut ys, mut y_index) = axis(top, height, grid[1], dimensions[1]);
        // Nearly complete readouts feed peers using different sampling grids.
        // Keep a dense projection here so those peers do not recompute a PSF
        // for every missing pixel. Small programmed windows remain sparse.
        let full = dimensions[0] * dimensions[1];
        let selected = xs.len() * ys.len();
        if selected < full && selected * 4 >= full * 3 {
            xs = (0..dimensions[0]).collect();
            ys = (0..dimensions[1]).collect();
            x_index = (0..dimensions[0]).map(|x| x as u16).collect();
            y_index = (0..dimensions[1]).map(|y| y as u16).collect();
        }
        Ok(Self {
            key: (grid, window),
            xs,
            ys,
            x_index,
            y_index,
        })
    }
    /// Union of programmed windows. Only selected rows and columns enter the
    /// PSF pass; its halo is supplied by the immutable binned scene.
    pub fn windows(
        dimensions: [usize; 2],
        grid: [usize; 2],
        windows: &[[usize; 4]],
    ) -> Result<Self, String> {
        let mut xs = vec![false; dimensions[0]];
        let mut ys = vec![false; dimensions[1]];
        for &[x, y, w, h] in windows {
            Self::validate(grid, [x, y, w, h])?;
            if w == 0 || h == 0 {
                continue;
            }
            for at in x..x + w {
                xs[((2 * at + 1) * dimensions[0] / (2 * grid[0])).min(dimensions[0] - 1)] = true;
            }
            for at in y..y + h {
                ys[((2 * at + 1) * dimensions[1] / (2 * grid[1])).min(dimensions[1] - 1)] = true;
            }
        }
        let axis = |selected: Vec<bool>| {
            let points: Vec<_> = selected
                .iter()
                .enumerate()
                .filter_map(|(i, &yes)| yes.then_some(i))
                .collect();
            let mut index = vec![u16::MAX; selected.len()];
            for (i, &p) in points.iter().enumerate() {
                index[p] = i as u16;
            }
            (points, index)
        };
        let (xs, x_index) = axis(xs);
        let (ys, y_index) = axis(ys);
        Ok(Self {
            key: (grid, [0, 0, grid[0], grid[1]]),
            xs,
            ys,
            x_index,
            y_index,
        })
    }
    pub fn validate(grid: [usize; 2], window: [usize; 4]) -> Result<(), String> {
        let [left, top, width, height] = window;
        if grid.contains(&0)
            || grid.iter().any(|&n| n > 16384)
            || left.checked_add(width).is_none_or(|end| end > grid[0])
            || top.checked_add(height).is_none_or(|end| end > grid[1])
        {
            return Err("optical readout window exceeds its sampling grid".into());
        }
        Ok(())
    }
    pub fn full(dimensions: [usize; 2]) -> Self {
        Self::new(dimensions, dimensions, [0, 0, dimensions[0], dimensions[1]]).unwrap()
    }
    pub fn complete(&self, dimensions: [usize; 2]) -> bool {
        self.xs.len() == dimensions[0] && self.ys.len() == dimensions[1]
    }
}

/// Pixels for one immutable exposure, with a pure full-scene law for other
/// coordinates. Inspection may materialize that law without changing state.
pub(crate) struct Projected {
    scene: Arc<Scene>,
    pub(super) coordinates: Arc<Coordinates>,
    radius: [usize; 2],
    fraction: [u32; 2],
    sensor_height: Option<f64>,
    scale: [f64; 3],
    sensor_width: f64,
    distance: Option<f64>,
    pixels: Arc<[u8]>,
}
impl Projected {
    pub(super) fn new(
        scene: Arc<Scene>,
        coordinates: Arc<Coordinates>,
        radius: usize,
        fraction: u32,
        linear: &[[u16; 3]],
        scale: [f64; 3],
        sensor_width: f64,
        distance: Option<f64>,
    ) -> Self {
        let mut pixels = vec![0; linear.len() * 6];
        let [width, height] = scene.dimensions;
        let pitch = sensor_width / width as f64;
        // Preserve the original expression's operation order, including the
        // normalized coordinate divisions. Only invariant axes move outwards.
        let xs: Vec<f64> = coordinates
            .xs
            .iter()
            .map(|&x| {
                let x = x as f64 * pitch + pitch * 0.5 - sensor_width * 0.5;
                distance.map_or(0., |d| {
                    let x = x / d;
                    x * x
                })
            })
            .collect();
        for (row, &y) in coordinates.ys.iter().enumerate() {
            let y = y as f64 * pitch + pitch * 0.5 - height as f64 * pitch * 0.5;
            let yy = distance.map_or(0., |d| {
                let y = y / d;
                y * y
            });
            for (column, &xx) in xs.iter().enumerate() {
                let field = if distance.is_some() {
                    let v = 1. / (1. + xx + yy);
                    v * v
                } else {
                    1.
                };
                let i = row * coordinates.xs.len() + column;
                for c in 0..3 {
                    let value = (f64::from(linear[i][c]) * scale[c] / 65535. * field)
                        .round_ties_even()
                        .clamp(0., 16383.) as u16;
                    pixels[i * 6 + c * 2..i * 6 + c * 2 + 2].copy_from_slice(&value.to_le_bytes());
                }
            }
        }
        Self {
            scene,
            coordinates,
            radius: [radius; 2],
            fraction: [fraction; 2],
            sensor_height: None,
            scale,
            sensor_width,
            distance,
            pixels: pixels.into(),
        }
    }
    pub(super) fn new_axes(
        scene: Arc<Scene>,
        coordinates: Arc<Coordinates>,
        radius: [usize; 2],
        fraction: [u32; 2],
        linear: &[[u16; 3]],
        scale: [f64; 3],
        sensor: [f64; 2],
        distance: Option<f64>,
    ) -> Self {
        let pixels = super::encode_response_coordinates(
            linear,
            scale,
            scene.dimensions,
            sensor,
            distance,
            &coordinates,
        );
        Self {
            scene,
            coordinates,
            radius,
            fraction,
            scale,
            sensor_width: sensor[0],
            sensor_height: Some(sensor[1]),
            distance,
            pixels,
        }
    }
    pub(crate) fn sample_offsets(
        &self,
        grid: [usize; 2],
        window: [usize; 4],
        step: usize,
    ) -> Option<super::super::exposure::SampleOffsets> {
        let [width, height] = self.scene.dimensions;
        let [x, y, w, h] = window;
        let columns: Option<Vec<_>> = (x..(x + w).min(grid[0]))
            .step_by(step)
            .map(|x| {
                let i =
                    self.coordinates.x_index[((2 * x + 1) * width / (2 * grid[0])).min(width - 1)];
                (i != u16::MAX).then_some(usize::from(i) * 6)
            })
            .collect();
        let rows: Option<Vec<_>> = (y..(y + h).min(grid[1]))
            .map(|y| {
                let i = self.coordinates.y_index
                    [((2 * y + 1) * height / (2 * grid[1])).min(height - 1)];
                (i != u16::MAX).then_some(usize::from(i) * self.coordinates.xs.len() * 6)
            })
            .collect();
        Some(super::super::exposure::SampleOffsets {
            pixels: self.pixels.clone(),
            columns: columns?,
            rows: rows?,
        })
    }
    pub fn covers(&self, grid: [usize; 2], windows: &[[usize; 4]]) -> bool {
        let [width, height] = self.scene.dimensions;
        windows.iter().all(|&[x, y, w, h]| {
            x + w <= grid[0]
                && y + h <= grid[1]
                && (x..x + w).all(|x| {
                    self.coordinates.x_index[((2 * x + 1) * width / (2 * grid[0])).min(width - 1)]
                        != u16::MAX
                })
                && (y..y + h).all(|y| {
                    self.coordinates.y_index[((2 * y + 1) * height / (2 * grid[1])).min(height - 1)]
                        != u16::MAX
                })
        })
    }
    pub fn prepared_pixels(&self) -> usize {
        self.coordinates.xs.len() * self.coordinates.ys.len()
    }
    pub fn complete(&self) -> bool {
        self.coordinates.complete(self.scene.dimensions)
    }
    #[inline(always)]
    pub fn sample(&self, x: usize, y: usize) -> [u16; 3] {
        let column = self.coordinates.x_index[x];
        let row = self.coordinates.y_index[y];
        if column != u16::MAX && row != u16::MAX {
            let at = (usize::from(row) * self.coordinates.xs.len() + usize::from(column)) * 6;
            return std::array::from_fn(|c| {
                u16::from_le_bytes([self.pixels[at + c * 2], self.pixels[at + c * 2 + 1]])
            });
        }
        self.other_sample(x, y)
    }
    #[cold]
    fn other_sample(&self, x: usize, y: usize) -> [u16; 3] {
        let height = self.scene.dimensions[1];
        let mean = |radius: usize| {
            let mut sum = [0u64; 3];
            for yy in y.saturating_sub(radius)..(y + radius + 1).min(height) {
                let v = self
                    .scene
                    .horizontal(x, yy, self.radius[0], self.fraction[0]);
                for c in 0..3 {
                    sum[c] += u64::from(v[c]);
                }
            }
            let first = self
                .scene
                .horizontal(x, 0, self.radius[0], self.fraction[0]);
            let last = self
                .scene
                .horizontal(x, height - 1, self.radius[0], self.fraction[0]);
            let count = (2 * radius + 1) as u64;
            std::array::from_fn::<_, 3, _>(|c| {
                (sum[c]
                    + u64::from(first[c]) * radius.saturating_sub(y) as u64
                    + u64::from(last[c]) * (y + radius + 1).saturating_sub(height) as u64
                    + count / 2)
                    / count
            })
        };
        let inner = if self.radius[1] == 0 {
            self.scene
                .horizontal(x, y, self.radius[0], self.fraction[0])
                .map(u64::from)
        } else {
            mean(self.radius[1])
        };
        let outer = if self.fraction[1] == 0 {
            inner
        } else {
            mean(self.radius[1] + 1)
        };
        let linear: [u16; 3] = std::array::from_fn(|c| blend(inner[c], self.fraction[1], outer[c]));
        let pitch = self.sensor_width / self.scene.dimensions[0] as f64;
        let field = self.distance.map_or(1., |d| {
            super::relative_illumination(
                self.sensor_height.map_or(
                    x as f64 * pitch + pitch * 0.5 - self.sensor_width * 0.5,
                    |_| {
                        ((x as f64 + 0.5) / self.scene.dimensions[0] as f64 - 0.5)
                            * self.sensor_width
                    },
                ),
                self.sensor_height.map_or(
                    y as f64 * pitch + pitch * 0.5 - height as f64 * pitch * 0.5,
                    |h| ((y as f64 + 0.5) / height as f64 - 0.5) * h,
                ),
                d,
            )
        });
        std::array::from_fn(|c| {
            libm::rint(f64::from(linear[c]) * self.scale[c] / 65535. * field).clamp(0., 16383.)
                as u16
        })
    }
    pub fn materialize(&self) -> Arc<[u8]> {
        if self.coordinates.complete(self.scene.dimensions) {
            return self.pixels.clone();
        }
        let all = Coordinates::full(self.scene.dimensions);
        if let Some(height) = self.sensor_height {
            let linear = project_axes(&self.scene, &all, self.radius, self.fraction);
            return super::encode_response_coordinates(
                &linear,
                self.scale,
                self.scene.dimensions,
                [self.sensor_width, height],
                self.distance,
                &all,
            );
        }
        let linear = project(&self.scene, &all, self.radius[0], self.fraction[0], None);
        encode_response(
            &linear,
            self.scale,
            self.scene.dimensions,
            self.sensor_width,
            self.distance,
        )
    }
}

/// The horizontal pass uses immutable scene row prefixes. Process nearby
/// columns together so vertical sums touch contiguous cache lines; only the
/// readout's source columns, rows and required PSF halo enter the work buffer.
pub(super) fn project(
    scene: &Scene,
    coordinates: &Coordinates,
    radius: usize,
    fraction: u32,
    horizontal: Option<&super::horizontal::Prepared>,
) -> Vec<[u16; 3]> {
    if radius == 0 && fraction == 0 {
        return project_axes(scene, coordinates, [0; 2], [0; 2]);
    }
    // Select the sample source once per projection, leaving each pixel loop
    // specialized for either cached means or the original scratch path.
    match horizontal {
        Some(prepared) => project_sampled(scene, coordinates, radius, fraction, |column, _, y| {
            prepared.sample(column, y)
        }),
        None => project_sampled(scene, coordinates, radius, fraction, |_, x, y| {
            scene.horizontal(x, y, radius, fraction)
        }),
    }
}
pub(super) fn project_axes(
    scene: &Scene,
    coordinates: &Coordinates,
    radius: [usize; 2],
    fraction: [u32; 2],
) -> Vec<[u16; 3]> {
    project_prepared_axes(scene, coordinates, radius, fraction, None)
}
pub(super) fn project_prepared_axes(
    scene: &Scene,
    coordinates: &Coordinates,
    radius: [usize; 2],
    fraction: [u32; 2],
    horizontal: Option<&super::horizontal::Prepared>,
) -> Vec<[u16; 3]> {
    if radius == [0; 2] && fraction == [0; 2] {
        return coordinates
            .ys
            .iter()
            .flat_map(|&y| coordinates.xs.iter().map(move |&x| scene.linear(x, y)))
            .collect();
    }
    match horizontal {
        Some(prepared) => project_sampled(
            scene,
            coordinates,
            radius[1],
            fraction[1],
            |column, _, y| prepared.sample(column, y),
        ),
        None => project_sampled(scene, coordinates, radius[1], fraction[1], |_, x, y| {
            scene.horizontal(x, y, radius[0], fraction[0])
        }),
    }
}
fn project_sampled(
    scene: &Scene,
    coordinates: &Coordinates,
    radius: usize,
    fraction: u32,
    sample: impl Fn(usize, usize, usize) -> [u16; 3],
) -> Vec<[u16; 3]> {
    let mut result = vec![[0; 3]; coordinates.xs.len() * coordinates.ys.len()];
    if coordinates.xs.is_empty() {
        return result;
    }
    let Some(&first_y) = coordinates.ys.first() else {
        return result;
    };
    let last_y = *coordinates.ys.last().unwrap();
    const LANES: usize = 8;
    let height = scene.dimensions[1];
    let begin = first_y.saturating_sub(radius + 1);
    let end = (last_y + radius + 2).min(height);
    let mut prefix = vec![[[0u64; 3]; LANES]; end - begin + 1];
    for (block, xs) in coordinates.xs.chunks(LANES).enumerate() {
        prefix[0] = [[0; 3]; LANES];
        for y in begin..end {
            for (lane, &x) in xs.iter().enumerate() {
                let value = sample(block * LANES + lane, x, y);
                prefix[y - begin + 1][lane] =
                    std::array::from_fn(|c| prefix[y - begin][lane][c] + u64::from(value[c]));
            }
        }
        let first: [[u16; 3]; LANES] = std::array::from_fn(|lane| {
            xs.get(lane)
                .map_or([0; 3], |&x| sample(block * LANES + lane, x, 0))
        });
        let last: [[u16; 3]; LANES] = std::array::from_fn(|lane| {
            xs.get(lane)
                .map_or([0; 3], |&x| sample(block * LANES + lane, x, height - 1))
        });
        for (row, &y) in coordinates.ys.iter().enumerate() {
            let inner = Span::new(y, height, radius);
            let outer = Span::new(y, height, radius + 1);
            for lane in 0..xs.len() {
                let at = row * coordinates.xs.len() + block * LANES + lane;
                for c in 0..3 {
                    let a = if radius == 0 {
                        prefix[y + 1 - begin][lane][c] - prefix[y - begin][lane][c]
                    } else {
                        inner.mean(
                            prefix[inner.first - begin][lane][c],
                            prefix[inner.last - begin][lane][c],
                            first[lane][c],
                            last[lane][c],
                        )
                    };
                    let b = if fraction == 0 {
                        a
                    } else {
                        outer.mean(
                            prefix[outer.first - begin][lane][c],
                            prefix[outer.last - begin][lane][c],
                            first[lane][c],
                            last[lane][c],
                        )
                    };
                    result[at][c] = blend(a, fraction, b);
                }
            }
        }
    }
    result
}
