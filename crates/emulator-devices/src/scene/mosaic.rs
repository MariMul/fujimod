//! Sensor-site radiance of a CFA mosaic. Each site contributes its own color
//! exactly; the other two colors are the mean of that color's sites in the
//! site's 6×6 cell. Cell means are computed once per immutable frame.
use std::sync::Arc;

pub type Cfa = [[u8; 6]; 6];

/// Immutable mosaic radiance with its precomputed per-cell color means.
pub(crate) struct MosaicSites {
    cfa: Cfa,
    width: usize,
    cells_wide: usize,
    radiance: Arc<[f32]>,
    /// Per cell, the mean radiance of each color over the cell's sites.
    cell_means: Vec<[f32; 3]>,
    /// Largest site radiance in the frame. Scene normalization and the
    /// response scale both derive from it, so the two stay consistent.
    pub max: f32,
}

impl MosaicSites {
    /// Radiance must already be validated finite and non-negative, with
    /// `width * height` sites and every CFA entry in 0..=2.
    pub fn new(cfa: Cfa, width: usize, height: usize, radiance: Arc<[f32]>) -> Self {
        let cells_wide = width.div_ceil(6);
        let cells_high = height.div_ceil(6);
        let mut sums = vec![[0f64; 3]; cells_wide * cells_high];
        let mut counts = vec![[0u32; 3]; cells_wide * cells_high];
        let mut max = 0f32;
        for y in 0..height {
            for x in 0..width {
                let value = radiance[y * width + x];
                max = max.max(value);
                let color = usize::from(cfa[y % 6][x % 6]);
                let cell = (y / 6) * cells_wide + x / 6;
                sums[cell][color] += f64::from(value);
                counts[cell][color] += 1;
            }
        }
        let cell_means = sums
            .iter()
            .zip(&counts)
            .map(|(sum, count)| {
                std::array::from_fn(|c| {
                    if count[c] == 0 {
                        0.
                    } else {
                        (sum[c] / f64::from(count[c])) as f32
                    }
                })
            })
            .collect();
        Self {
            cfa,
            width,
            cells_wide,
            radiance,
            cell_means,
            max,
        }
    }

    /// Radiance seen at sensor site (x, y) for each color channel.
    #[inline(always)]
    pub fn site(&self, x: usize, y: usize) -> [f32; 3] {
        let mut values = self.cell_means[(y / 6) * self.cells_wide + x / 6];
        let own = usize::from(self.cfa[y % 6][x % 6]);
        if own < 3 {
            values[own] = self.radiance[y * self.width + x];
        }
        values
    }
}
