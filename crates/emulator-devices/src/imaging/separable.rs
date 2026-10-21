//! Declared integer component filters: replicated edges, rounded horizontal
//! and vertical passes, then pixel-center rational bilinear sampling. These
//! software numerical laws are not measured Fuji filter precision.

pub struct Geometry {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub output_width: usize,
    pub output_height: usize,
    pub scalar: bool,
}
pub fn filter(
    source: &[u8],
    g: Geometry,
    horizontal: &[u32],
    vertical: &[u32],
    ratio: [usize; 2],
    out: &mut [u8],
    a: &mut [u16],
    b: &mut [u16],
) -> Result<(), String> {
    let Geometry {
        width: w,
        height: h,
        pitch,
        output_width: dw,
        output_height: dh,
        scalar,
    } = g;
    let [num, den] = ratio;
    if w == 0
        || h == 0
        || dw == 0
        || dh == 0
        || pitch < w * 2
        || source.len() != pitch * h
        || out.len() != dw * dh * 2
        || a.len() < w * h
        || b.len() < w * h
        || !scalar && (!w.is_multiple_of(2) || !dw.is_multiple_of(2))
        || num == 0
        || den == 0
        || num > 512
        || den > 512
        || [horizontal, vertical].iter().any(|k| {
            k.is_empty()
                || k.len() % 2 == 0
                || k.iter().sum::<u32>() == 0
                || k.iter().sum::<u32>() > 256
        })
    {
        return Err("invalid bounded separable raster geometry/filter".into());
    }
    for channel in 0..if scalar { 1 } else { 3 } {
        let sw = if channel == 0 { w } else { w / 2 };
        let ow = if channel == 0 { dw } else { dw / 2 };
        for y in 0..h {
            for x in 0..sw {
                a[y * sw + x] = if scalar {
                    u16::from_le_bytes(
                        source[y * pitch + x * 2..y * pitch + x * 2 + 2]
                            .try_into()
                            .unwrap(),
                    )
                } else {
                    source[y * pitch
                        + if channel == 0 {
                            x / 2 * 4 + x % 2
                        } else {
                            x * 4 + channel + 1
                        }] as u16
                };
            }
        }
        for (kernel, along_x) in [(horizontal, true), (vertical, false)] {
            let sum: u32 = kernel.iter().sum();
            let radius = kernel.len() / 2;
            for y in 0..h {
                for x in 0..sw {
                    let mut total = 0;
                    for (n, &weight) in kernel.iter().enumerate() {
                        let sx = if along_x {
                            (x + n).saturating_sub(radius).min(sw - 1)
                        } else {
                            x
                        };
                        let sy = if along_x {
                            y
                        } else {
                            (y + n).saturating_sub(radius).min(h - 1)
                        };
                        total += a[sy * sw + sx] as u32 * weight;
                    }
                    b[y * sw + x] = ((total + sum / 2) / sum) as u16;
                }
            }
            a[..sw * h].copy_from_slice(&b[..sw * h]);
        }
        let axis = |n: usize, limit: usize| {
            let unit = 2 * den;
            let pos = ((2 * n + 1) * num)
                .saturating_sub(den)
                .min((limit - 1) * unit);
            (
                pos / unit,
                (pos / unit + 1).min(limit - 1),
                (pos % unit) as u64,
            )
        };
        let unit = (2 * den) as u64;
        for y in 0..dh {
            let (y0, y1, fy) = axis(y, h);
            for x in 0..ow {
                let (x0, x1, fx) = axis(x, sw);
                let sample =
                    |y: usize| a[y * sw + x0] as u64 * (unit - fx) + a[y * sw + x1] as u64 * fx;
                let value = ((sample(y0) * (unit - fy) + sample(y1) * fy + unit * unit / 2)
                    / (unit * unit)) as u16;
                if scalar {
                    out[(y * dw + x) * 2..(y * dw + x) * 2 + 2]
                        .copy_from_slice(&value.to_le_bytes());
                } else {
                    let at = y * dw * 2
                        + if channel == 0 {
                            x / 2 * 4 + x % 2
                        } else {
                            x * 4 + channel + 1
                        };
                    out[at] = value as u8;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replicated_edge_rounding_and_component_grids() {
        let source = [0, 100, 20, 200, 200, 240, 60, 100];
        let mut out = [0; 8];
        filter(
            &source,
            Geometry {
                width: 4,
                height: 1,
                pitch: 8,
                output_width: 4,
                output_height: 1,
                scalar: false,
            },
            &[1, 2, 1],
            &[1],
            [1, 1],
            &mut out,
            &mut [0; 4],
            &mut [0; 4],
        )
        .unwrap();
        assert_eq!(out, [25, 100, 30, 175, 185, 230, 50, 125]);
    }
    #[test]
    fn scalar_precision_and_pixel_center_rational_sampling() {
        let source = [0u16, 1000, 2000, 3000]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let mut out = [0; 4];
        filter(
            &source,
            Geometry {
                width: 4,
                height: 1,
                pitch: 8,
                output_width: 2,
                output_height: 1,
                scalar: true,
            },
            &[1],
            &[1],
            [2, 1],
            &mut out,
            &mut [0; 4],
            &mut [0; 4],
        )
        .unwrap();
        assert_eq!(out, [0xf4, 1, 0xc4, 9]);
    }
}
