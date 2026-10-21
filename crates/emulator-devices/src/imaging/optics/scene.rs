use std::sync::Arc;

/// Immutable scene irradiance. Row prefixes are independent of the lens,
/// exposure and readout window; each scene is linearized once per projection.
pub(super) struct Scene {
    pub source: Arc<[u8]>,
    pub dimensions: [usize; 2],
    prefix: Vec<[u32; 3]>,
}
impl Scene {
    pub fn new(source: Arc<[u8]>, dimensions: [usize; 2]) -> Self {
        let linear: [u16; 256] = std::array::from_fn(|i| {
            let v = i as f64 / 255.;
            let v = if v <= 0.04045 {
                v / 12.92
            } else {
                libm::pow((v + 0.055) / 1.055, 2.4)
            };
            libm::rint(v * 65535.) as u16
        });
        let [width, height] = dimensions;
        let mut prefix = vec![[0; 3]; (width + 1) * height];
        for (row, pixels) in prefix
            .chunks_exact_mut(width + 1)
            .zip(source.chunks_exact(width * 3))
        {
            let mut sum = [0u32; 3];
            for (out, rgb) in row[1..].iter_mut().zip(pixels.chunks_exact(3)) {
                for c in 0..3 {
                    sum[c] += u32::from(linear[rgb[c] as usize]);
                }
                *out = sum;
            }
        }
        Self {
            source,
            dimensions,
            prefix,
        }
    }
    /// Integrate linear scene cells over each readout pixel before optical
    /// filtering. Integer overlap weights preserve a flat field exactly.
    pub fn binned(&self, dimensions: [usize; 2]) -> Self {
        let [width, height] = dimensions;
        let [sw, sh] = self.dimensions;
        let mut prefix = vec![[0u32; 3]; (width + 1) * height];
        for y in 0..height {
            let top = y * sh;
            let bottom = (y + 1) * sh;
            let mut sum = [0u32; 3];
            for x in 0..width {
                let left = x * sw;
                let right = (x + 1) * sw;
                let mut color = [0u64; 3];
                for sy in top / height..bottom.div_ceil(height) {
                    let wy = bottom.min((sy + 1) * height) - top.max(sy * height);
                    let row = sy * (sw + 1);
                    let first = left / width;
                    let last = (right - 1) / width;
                    for c in 0..3 {
                        let horizontal = if first == last {
                            u64::from(self.prefix[row + first + 1][c] - self.prefix[row + first][c])
                                * sw as u64
                        } else {
                            let a = u64::from(
                                self.prefix[row + first + 1][c] - self.prefix[row + first][c],
                            );
                            let b = u64::from(
                                self.prefix[row + last + 1][c] - self.prefix[row + last][c],
                            );
                            a * ((first + 1) * width - left) as u64
                                + b * (right - last * width) as u64
                                + u64::from(
                                    self.prefix[row + last][c] - self.prefix[row + first + 1][c],
                                ) * width as u64
                        };
                        color[c] += horizontal * wy as u64;
                    }
                }
                let area = (sw * sh) as u64;
                for c in 0..3 {
                    sum[c] += ((color[c] + area / 2) / area) as u32;
                }
                prefix[y * (width + 1) + x + 1] = sum;
            }
        }
        Self {
            source: self.source.clone(),
            dimensions,
            prefix,
        }
    }
    #[inline(always)]
    pub fn linear(&self, x: usize, y: usize) -> [u16; 3] {
        let row = y * (self.dimensions[0] + 1);
        std::array::from_fn(|c| (self.prefix[row + x + 1][c] - self.prefix[row + x][c]) as u16)
    }
    #[inline(always)]
    pub fn horizontal(&self, x: usize, y: usize, radius: usize, fraction: u32) -> [u16; 3] {
        if radius == 0 && fraction == 0 {
            return self.linear(x, y);
        }
        let width = self.dimensions[0];
        let row = &self.prefix[y * (width + 1)..(y + 1) * (width + 1)];
        let inner = Span::new(x, width, radius);
        let outer = Span::new(x, width, radius + 1);
        let mut result = [0; 3];
        for c in 0..3 {
            let first = row[1][c] as u16;
            let last = (row[width][c] - row[width - 1][c]) as u16;
            let a = if radius == 0 {
                u64::from(row[x + 1][c] - row[x][c])
            } else {
                inner.mean(
                    u64::from(row[inner.first][c]),
                    u64::from(row[inner.last][c]),
                    first,
                    last,
                )
            };
            let b = if fraction == 0 {
                a
            } else {
                outer.mean(
                    u64::from(row[outer.first][c]),
                    u64::from(row[outer.last][c]),
                    first,
                    last,
                )
            };
            result[c] = blend(a, fraction, b);
        }
        result
    }
}
/// A clamped box interval, shared across channels and readout columns.
#[derive(Clone, Copy)]
pub(super) struct Span {
    pub first: usize,
    pub last: usize,
    before: u64,
    after: u64,
    count: u64,
}
impl Span {
    #[inline(always)]
    pub fn new(point: usize, length: usize, radius: usize) -> Self {
        Self {
            first: point.saturating_sub(radius),
            last: (point + radius + 1).min(length),
            before: radius.saturating_sub(point) as u64,
            after: (point + radius + 1).saturating_sub(length) as u64,
            count: (2 * radius + 1) as u64,
        }
    }
    #[inline(always)]
    pub fn mean(self, a: u64, b: u64, first: u16, last: u16) -> u64 {
        (b - a + u64::from(first) * self.before + u64::from(last) * self.after + self.count / 2)
            / self.count
    }
}

#[inline(always)]
pub(super) fn blend(inner: u64, fraction: u32, outer: u64) -> u16 {
    if fraction == 0 {
        inner as u16
    } else {
        ((inner * u64::from(65536 - fraction) + outer * u64::from(fraction) + 32768) >> 16) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binning_integrates_linear_light_and_preserves_flat_fields() {
        let flat = Scene::new(vec![91; 7 * 5 * 3].into(), [7, 5]);
        let small = flat.binned([3, 2]);
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(small.linear(x, y), flat.linear(0, 0));
            }
        }
        let alternating: Vec<_> = (0..8 * 6)
            .flat_map(|i| [if i % 2 == 0 { 0 } else { 255 }; 3])
            .collect();
        let scene = Scene::new(alternating.into(), [8, 6]);
        let small = scene.binned([4, 3]);
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(small.linear(x, y), [32768; 3]);
            }
        }
        assert_ne!(
            small.linear(0, 0),
            Scene::new(vec![128; 3].into(), [1, 1]).linear(0, 0)
        );
    }
    #[test]
    fn nonintegral_bins_match_independent_cell_overlap_integration() {
        let bytes: Vec<_> = (0..7 * 5 * 3)
            .map(|i| ((i * 71 + 23) % 256) as u8)
            .collect();
        let scene = Scene::new(bytes.into(), [7, 5]);
        for dims in [[3, 2], [4, 3], [1, 1], [7, 5]] {
            let small = scene.binned(dims);
            for y in 0..dims[1] {
                for x in 0..dims[0] {
                    let mut sum = [0u64; 3];
                    for sy in 0..5 {
                        for sx in 0..7 {
                            let wx = ((x + 1) * 7)
                                .min((sx + 1) * dims[0])
                                .saturating_sub((x * 7).max(sx * dims[0]));
                            let wy = ((y + 1) * 5)
                                .min((sy + 1) * dims[1])
                                .saturating_sub((y * 5).max(sy * dims[1]));
                            let raw = scene.linear(sx, sy);
                            for c in 0..3 {
                                sum[c] += u64::from(raw[c]) * (wx * wy) as u64;
                            }
                        }
                    }
                    assert_eq!(small.linear(x, y), sum.map(|s| ((s + 17) / 35) as u16));
                }
            }
        }
    }
}
