use super::*;
use fujimod_emulator_devices::{
    imaging::exposure::{Calibration, Exposure},
    scene::{Frame, FramePixels},
};
use std::sync::Arc;

fn exposed(frame: Arc<Frame>, seconds: f64) -> ExposedFrame {
    ExposedFrame::from_frame(
        frame,
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

fn registers([x, y, w, h]: [usize; 4]) -> Vec<u32> {
    let mut r = vec![0; 1024];
    r[0x100 / 4] = ((x as u32) << 16) | (w - 1) as u32;
    r[0x104 / 4] = ((y as u32) << 16) | (h - 1) as u32;
    r[0x200 / 4] = r[0x100 / 4];
    r[0x204 / 4] = (((y + 1) as u32) << 16) | (h - 1) as u32;
    for i in 0..10 {
        r[(0x10c + i * 8) / 4] = r[0x100 / 4];
        r[(0x110 + i * 8) / 4] = r[0x104 / 4];
        r[(0x208 + i * 8) / 4] = r[0x200 / 4];
        r[(0x20c + i * 8) / 4] = r[0x204 / 4];
    }
    r
}

#[test]
fn repeated_source_runs_match_scalar_sampling_with_clips_and_per_row_rounding() {
    for [width, height] in [[1, 1], [3, 2], [7, 5], [23, 17]] {
        for raw in [false, true] {
            let pixels = if raw {
                FramePixels::Raw16(
                    (0..width * height)
                        .map(|i| ((i * 733 + i / width * 59) % 16384) as u16)
                        .collect::<Vec<_>>()
                        .into(),
                )
            } else {
                FramePixels::Rgb24(
                    (0..width * height * 3)
                        .map(|i| ((i * 73 + i / (width * 3) * 59) % 256) as u8)
                        .collect::<Vec<_>>()
                        .into(),
                )
            };
            let frame = exposed(Arc::new(Frame::new(width, height, pixels).unwrap()), 0.001);
            for grid in [[1, 1], [3, 4], [17, 13], [64, 41]] {
                for rect @ [x, y, w, h] in [
                    [0, 0, grid[0], grid[1]],
                    [grid[0] / 2, grid[1] / 3, 7, 5],
                    [grid[0] + 2, grid[1] + 1, 3, 4],
                    [0, 0, 1, 1],
                ] {
                    let mut colors = [0u64; 3];
                    let mut energy = [0u64; 2];
                    for yy in y.min(grid[1])..(y + h).min(grid[1]) {
                        let mut row = [0u64; 2];
                        let end = (x + w).min(grid[0]);
                        for xx in x.min(grid[0])..end {
                            let sample = frame.sample(xx, yy, grid);
                            for c in 0..3 {
                                colors[c] += u64::from(sample[c]);
                            }
                            for (c, distance) in [3, 1].into_iter().enumerate() {
                                if xx + distance < end {
                                    let d = i64::from(frame.sample(xx + distance, yy, grid)[1])
                                        - i64::from(sample[1]);
                                    row[c] += (d * d) as u64;
                                }
                            }
                        }
                        energy[0] += row[0] / 2304;
                        energy[1] += row[1] / 256;
                    }
                    let r = registers(rect);
                    let color = color_spans(&frame, grid, &r).unwrap();
                    for c in 0..3 {
                        let expected = colors[c] * [2, 5, 2][c] / 9;
                        assert_eq!(
                            u32::from_le_bytes(color[c].bytes[..4].try_into().unwrap()) as u64,
                            expected
                        );
                        assert_eq!(
                            u64::from_le_bytes(
                                color[3].bytes[c * 8..c * 8 + 8].try_into().unwrap()
                            ),
                            expected
                        );
                    }
                    let contrast = contrast_spans(&frame, grid, &r).unwrap();
                    for c in 0..2 {
                        assert_eq!(
                            u32::from_le_bytes(contrast[c].bytes[..4].try_into().unwrap()) as u64,
                            energy[c]
                        );
                    }
                    assert_eq!(
                        u64::from_le_bytes(contrast[2].bytes[..8].try_into().unwrap()),
                        energy[1]
                    );
                }
            }
        }
    }
}

fn bytes(spans: &[fujimod_emulator_devices::scene::StatisticsSpan]) -> Vec<(usize, u32, Vec<u8>)> {
    spans
        .iter()
        .map(|s| (s.destination, s.offset, s.bytes.clone()))
        .collect()
}

#[test]
fn cached_color_density_changes_without_changing_sample_pixels() {
    let first = exposed(
        Arc::new(Frame::new(3, 2, FramePixels::Rgb24([20, 50, 100].repeat(6).into())).unwrap()),
        0.001,
    );
    let mut changed = first.clone();
    changed.color_sample_weights = [1; 3];
    let r = registers([0, 0, 3, 2]);
    let mut cache = Cache::default();
    let old = bytes(cache.measure(&first, [3, 2], &r).unwrap());
    let new = cache.measure(&changed, [3, 2], &r).unwrap();
    assert_eq!(first.sample(0, 0, [3, 2]), changed.sample(0, 0, [3, 2]));
    assert_ne!(old, bytes(new));
    for c in 0..3 {
        assert_eq!(
            u32::from_le_bytes(new[c].bytes[..4].try_into().unwrap()),
            u32::from(changed.sample(0, 0, [3, 2])[c]) * 2
        );
    }
}

#[test]
fn cached_measurements_follow_registers_grid_scene_and_exposure_after_eviction() {
    let frame = Arc::new(
        Frame::new(
            3,
            2,
            FramePixels::Rgb24(vec![0, 31, 97, 255, 100, 18, 71, 201, 255].repeat(2).into()),
        )
        .unwrap(),
    );
    let first = exposed(frame.clone(), 0.001);
    let second = exposed(frame, 0.002);
    let mut cache = Cache::default();
    let mut r = registers([0, 0, 9, 7]);
    let mut previous = None;
    for (index, (frame, grid)) in [
        (&first, [9, 7]),
        (&first, [9, 7]),
        (&first, [13, 11]),
        (&second, [13, 11]),
        (&second, [13, 11]),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 4 {
            r[0x200 / 4] = 1 << 16 | 8;
            r[0x100 / 4] = 1 << 16 | 8;
        }
        let actual = bytes(cache.measure(frame, grid, &r).unwrap());
        let mut reference = color_spans(frame, grid, &r).unwrap();
        reference.extend(contrast_spans(frame, grid, &r).unwrap());
        assert_eq!(actual, bytes(&reference));
        if index >= 2 {
            assert_ne!(Some(&actual), previous.as_ref());
        }
        previous = Some(actual.clone());
        cache.clear();
        assert_eq!(bytes(cache.measure(frame, grid, &r).unwrap()), actual);
    }
    let other = exposed(
        Arc::new(Frame::new(3, 2, FramePixels::Rgb24(vec![9; 18].into())).unwrap()),
        0.002,
    );
    assert_ne!(
        bytes(cache.measure(&other, [13, 11], &r).unwrap()),
        previous.unwrap()
    );
}
