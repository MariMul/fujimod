//! Declared still development and scaling of the X-Processor Pro.
//!
//! X-T20 2.11 develops a still in two register-programmed steps (bootemu
//! write log of one JPEG shot, S2 to the end of development):
//!
//! - The development engine 0xfff98000 (GIC 130, step 0x06023320) reads the
//!   RAW at +0x08 (the read-out buffer + 16 lines), stride +0x0c in 16-bit
//!   units, and writes a picture of the size at +0x14 ((lines - 1) << 16 |
//!   (pixels - 1): 4057 x 6033) to +0x1c, stride +0x20 (0xb35a000, 0x1810).
//!   +0x18 (0x1666602) is a mode word whose fields are not decoded.
//! - The resizer 0xfffc3000 reads a packed 4:2:2 picture of the size at +0x08
//!   from +0xc0, stride +0xc4, and writes the size at +0x14 to +0xc8, stride
//!   +0xcc (strides in 16-bit units). +0x18/+0x1c are the horizontal and
//!   vertical steps in source pixels per output pixel: 2^(bits 31..16) x
//!   (bits 15..0) / 0x1000. A shot runs four jobs: the 6000x4000 crop of the
//!   development output into the main picture 0x80000000 (step 1), 6000x2000
//!   (every second line, stride 0x2ee0) to 752x250, 752x250 to 320x213 (13
//!   lines into the 320x240 preview) and 320x240 to 160x120. +0x100 reads
//!   0x11000002 in all of them. Playback magnifies the decoded picture to
//!   the display with 0x11110002, whose two extra bits are not decoded.
//!
//! The JPEG encoder then reads the main picture as packed 4:2:2 (Y0 Y1 Cb
//! Cr). [`PROFILE`] is the declared approximation of both steps, for the
//! declared sensor response of [`crate::imaging::SENSOR_PROFILE`]: each
//! output pixel takes the nearest X-Trans site of each colour, the response
//! is inverted to sRGB (black [`crate::imaging::BLACK`], white
//! [`crate::imaging::WHITE`], unity white balance) and encoded as BT.601
//! full-range (JFIF) YCbCr with the pixel pair's chroma averaged. The
//! resizer averages the source pixels each output pixel covers. Lens
//! corrections, noise reduction, sharpening, film simulation, the camera's
//! white balance and tone curve, and the analysis passes (0xfff7e000,
//! 0xfffc4000) are not modeled.

/// Declared development and scaling approximation (see the module notes).
pub const PROFILE: &str = "xpp-declared-xtrans-nearest-srgb-jfif-422-v1";

/// X-Trans 6×6 colour sites: 0 red, 1 green, 2 blue.
pub const XTRANS: [[u8; 6]; 6] = [
    [0, 2, 1, 2, 0, 1],
    [1, 1, 0, 1, 1, 2],
    [1, 1, 2, 1, 1, 0],
    [2, 0, 1, 0, 2, 1],
    [1, 1, 2, 1, 1, 0],
    [1, 1, 0, 1, 1, 2],
];

/// Picture geometry: lines and pixels of a size word.
pub fn size(word: u32) -> (usize, usize) {
    ((word >> 16) as usize + 1, (word & 0xffff) as usize + 1)
}

/// Source pixels per output pixel of a step word, if it is one.
pub fn step(word: u32) -> Option<f64> {
    let (shift, fraction) = (word >> 16, word & 0xffff);
    (shift <= 8 && fraction != 0).then(|| f64::from(1u32 << shift) * f64::from(fraction) / 4096.)
}

/// (line, sample) offset of the nearest site of each colour from each site
/// of the tile.
fn nearest() -> [[[(i64, i64); 3]; 6]; 6] {
    let mut offsets = [[[(0, 0); 3]; 6]; 6];
    for (y, row) in offsets.iter_mut().enumerate() {
        for (x, site) in row.iter_mut().enumerate() {
            for (colour, offset) in site.iter_mut().enumerate() {
                let mut best = (i64::MAX, 0, 0);
                for dy in -3i64..=3 {
                    for dx in -3i64..=3 {
                        let (sy, sx) =
                            ((y as i64 + dy).rem_euclid(6), (x as i64 + dx).rem_euclid(6));
                        if usize::from(XTRANS[sy as usize][sx as usize]) == colour {
                            best = best.min((dy * dy + dx * dx, dy, dx));
                        }
                    }
                }
                *offset = (best.1, best.2);
            }
        }
    }
    offsets
}

/// sRGB byte of each sample of the declared response.
fn inverse_response() -> Vec<u8> {
    let (black, white) = (
        f64::from(crate::imaging::BLACK),
        f64::from(crate::imaging::WHITE),
    );
    (0..=u16::MAX)
        .map(|sample| {
            let linear = ((f64::from(sample) - black) / (white - black)).clamp(0., 1.);
            let srgb = if linear <= 0.0031308 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1. / 2.4) - 0.055
            };
            (srgb * 255.).round() as u8
        })
        .collect()
}

/// BT.601 full-range (JFIF) Y, Cb and Cr of an sRGB pixel.
fn ycbcr([r, g, b]: [u8; 3]) -> [i32; 3] {
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
    [
        (19595 * r + 38470 * g + 7471 * b + 32768) >> 16,
        ((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16) + 128,
        ((32768 * r - 27439 * g - 5329 * b + 32768) >> 16) + 128,
    ]
}

fn byte(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Packed 4:2:2 lines (Y0 Y1 Cb Cr, `2 * width` bytes rounded up to a pair)
/// developed from `lines` lines of little-endian u16 X-Trans samples, `stride`
/// bytes apart, whose first sample is the tile site `phase` (line, sample).
pub fn develop(
    raw: &[u8],
    stride: usize,
    lines: usize,
    width: usize,
    phase: (usize, usize),
) -> Result<Vec<Vec<u8>>, String> {
    if lines < 6 || width < 6 || stride < 2 * width || raw.len() < stride * (lines - 1) + 2 * width
    {
        return Err("Unsupported still development geometry".into());
    }
    let (offsets, response) = (nearest(), inverse_response());
    let sample = |y: i64, x: i64| {
        // Extend by whole tiles so border sites keep their colour.
        let y = if y < 0 {
            y + 6
        } else if y >= lines as i64 {
            y - 6
        } else {
            y
        } as usize;
        let x = if x < 0 {
            x + 6
        } else if x >= width as i64 {
            x - 6
        } else {
            x
        } as usize;
        let i = y * stride + 2 * x;
        response[usize::from(u16::from_le_bytes([raw[i], raw[i + 1]]))]
    };
    let pairs = width.div_ceil(2);
    Ok((0..lines)
        .map(|y| {
            let row = &offsets[(y + phase.0) % 6];
            let mut line = Vec::with_capacity(4 * pairs);
            for pair in 0..pairs {
                let mut out = [[0; 3]; 2];
                for (i, pixel) in out.iter_mut().enumerate() {
                    let x = (2 * pair + i).min(width - 1);
                    let site = &row[(x + phase.1) % 6];
                    let rgb = [0, 1, 2].map(|c| sample(y as i64 + site[c].0, x as i64 + site[c].1));
                    *pixel = ycbcr(rgb);
                }
                line.extend_from_slice(&[
                    byte(out[0][0]),
                    byte(out[1][0]),
                    byte((out[0][1] + out[1][1] + 1) >> 1),
                    byte((out[0][2] + out[1][2] + 1) >> 1),
                ]);
            }
            line
        })
        .collect())
}

/// Packed 4:2:2 picture geometry of a resizer job.
#[derive(Clone, Copy, Debug)]
pub struct Scale {
    pub stride: usize,
    pub lines: usize,
    pub width: usize,
    pub out_lines: usize,
    pub out_width: usize,
    pub step_x: f64,
    pub step_y: f64,
}

/// Output lines (`2 * out_width` bytes) of a box-filtered packed 4:2:2
/// picture: each output pixel averages the source pixels its steps cover.
pub fn resample(source: &[u8], s: Scale) -> Result<Vec<Vec<u8>>, String> {
    if s.width < 2
        || s.width & 1 != 0
        || s.out_width & 1 != 0
        || s.out_width == 0
        || s.lines == 0
        || s.out_lines == 0
        || s.stride < 2 * s.width
        || source.len() < s.stride * (s.lines - 1) + 2 * s.width
    {
        return Err("Unsupported resizer geometry".into());
    }
    let span = |i: usize, step: f64, limit: usize| {
        let start = ((i as f64 * step) as usize).min(limit - 1);
        let end = (((i + 1) as f64 * step) as usize).clamp(start + 1, limit);
        start..end
    };
    if s.width == s.out_width && s.lines == s.out_lines && s.step_x == 1. && s.step_y == 1. {
        return Ok((0..s.lines)
            .map(|y| source[y * s.stride..y * s.stride + 2 * s.width].to_vec())
            .collect());
    }
    let columns: Vec<_> = (0..s.out_width)
        .map(|x| span(x, s.step_x, s.width))
        .collect();
    Ok((0..s.out_lines)
        .map(|y| {
            let rows = span(y, s.step_y, s.lines);
            let mut line = Vec::with_capacity(2 * s.out_width);
            for pair in columns.chunks_exact(2) {
                let mut out = [[0u32; 3]; 2];
                for (pixel, xs) in out.iter_mut().zip(pair) {
                    let mut n = 0;
                    for row in rows.clone() {
                        let line = &source[row * s.stride..];
                        for x in xs.clone() {
                            let p = &line[(x / 2) * 4..(x / 2) * 4 + 4];
                            pixel[0] += u32::from(p[x & 1]);
                            pixel[1] += u32::from(p[2]);
                            pixel[2] += u32::from(p[3]);
                            n += 1;
                        }
                    }
                    for v in pixel.iter_mut() {
                        *v = (*v + n / 2) / n;
                    }
                }
                line.extend_from_slice(&[
                    out[0][0] as u8,
                    out[1][0] as u8,
                    ((out[0][1] + out[1][1] + 1) / 2) as u8,
                    ((out[0][2] + out[1][2] + 1) / 2) as u8,
                ]);
            }
            line
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_step_words_decode_as_the_shot_programs_them() {
        assert_eq!(size(0xfd8_1790), (4057, 6033));
        assert_eq!(size(0xf9f_176f), (4000, 6000));
        assert_eq!(step(0x1000), Some(1.));
        assert_eq!(step(0x1_1000), Some(2.));
        assert!((step(0x2_2005).unwrap() - 8.0049).abs() < 1e-3);
        assert_eq!(step(0x2_0000), None);
    }

    #[test]
    fn a_uniform_colour_develops_to_its_jfif_ycbcr() {
        // Linear red 1, green 0.5, blue 0 through the declared response.
        let (black, white) = (crate::imaging::BLACK, crate::imaging::WHITE);
        let level = |c: u8| match c {
            0 => white,
            1 => black + ((0.5 * f64::from(white - black)) as u16),
            _ => black,
        };
        let (lines, width, stride) = (12, 12, 32);
        let mut raw = vec![0; stride * lines];
        for y in 0..lines {
            for x in 0..width {
                let v = level(XTRANS[(y + 2) % 6][(x + 2) % 6]);
                raw[y * stride + 2 * x..][..2].copy_from_slice(&v.to_le_bytes());
            }
        }
        let out = develop(&raw, stride, lines, width, (2, 2)).unwrap();
        assert_eq!((out.len(), out[0].len()), (12, 24));
        let [y, cb, cr] = ycbcr([255, 188, 0]);
        for line in &out {
            for p in line.chunks_exact(4) {
                assert_eq!(p, [y as u8, y as u8, cb as u8, cr as u8]);
            }
        }
        assert!(develop(&raw, stride, lines, width + 8, (2, 2)).is_err());
    }

    #[test]
    fn the_wrong_tile_phase_mixes_the_colours() {
        let (lines, width, stride) = (12, 12, 24);
        let mut raw = vec![0; stride * lines];
        for y in 0..lines {
            for x in 0..width {
                let v = if XTRANS[y % 6][x % 6] == 0 {
                    crate::imaging::WHITE
                } else {
                    0
                };
                raw[y * stride + 2 * x..][..2].copy_from_slice(&v.to_le_bytes());
            }
        }
        let red = develop(&raw, stride, lines, width, (0, 0)).unwrap();
        assert!(red.iter().flatten().skip(3).step_by(4).all(|&cr| cr == 255));
        let shifted = develop(&raw, stride, lines, width, (0, 1)).unwrap();
        assert!(
            shifted
                .iter()
                .flatten()
                .skip(3)
                .step_by(4)
                .any(|&cr| cr != 255)
        );
    }

    #[test]
    fn the_resizer_copies_at_step_one_and_averages_when_scaling() {
        let s = Scale {
            stride: 16,
            lines: 2,
            width: 4,
            out_lines: 2,
            out_width: 4,
            step_x: 1.,
            step_y: 1.,
        };
        let mut source = vec![0; 32];
        source[..8].copy_from_slice(&[10, 20, 100, 200, 30, 40, 110, 210]);
        source[16..24].copy_from_slice(&[50, 60, 120, 220, 70, 80, 130, 230]);
        let copy = resample(&source, s).unwrap();
        assert_eq!(copy, vec![source[..8].to_vec(), source[16..24].to_vec()]);
        let half = resample(
            &source,
            Scale {
                out_lines: 1,
                out_width: 2,
                step_x: 2.,
                step_y: 2.,
                ..s
            },
        )
        .unwrap();
        // Y: (10 + 20 + 50 + 60) / 4 and (30 + 40 + 70 + 80) / 4; chroma of all four pixels each.
        assert_eq!(half, vec![vec![35, 55, 115, 215]]);
        assert!(resample(&source, Scale { stride: 4, ..s }).is_err());
    }
}
