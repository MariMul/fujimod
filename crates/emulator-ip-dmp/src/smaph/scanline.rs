//! Coverage on the DMP's eighth-pixel grid.
//! Fill crossings and stroke distances use integer arithmetic so CPU and GPU
//! paths classify shared and collinear sample points identically.

type Point = [f64; 2];
type GridPoint = [i32; 2];

const GRID_SCALE: f64 = 8.0;
const MAX_GRID_COORDINATE: i32 = 16_383;

pub(super) fn grid_coordinate(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (value * GRID_SCALE).round().clamp(
        -f64::from(MAX_GRID_COORDINATE),
        f64::from(MAX_GRID_COORDINATE),
    ) as i32
}

pub(super) fn sample_grid(pixel: usize, index: u32, samples: u32) -> i64 {
    let numerator = (u64::from(index) * 2 + 1) * 4;
    let offset = (numerator + u64::from(samples) / 2) / u64::from(samples);
    pixel as i64 * 8 + offset as i64
}

fn stroke_covers(point: [i64; 2], a: GridPoint, b: GridPoint, radius: i32) -> bool {
    if radius < 0 {
        return false;
    }
    let dx = i64::from(b[0]) - i64::from(a[0]);
    let dy = i64::from(b[1]) - i64::from(a[1]);
    let qx = point[0] - i64::from(a[0]);
    let qy = point[1] - i64::from(a[1]);
    let length_squared = dx * dx + dy * dy;
    let radius_squared = i128::from(radius) * i128::from(radius);
    if length_squared == 0 {
        return i128::from(qx) * i128::from(qx) + i128::from(qy) * i128::from(qy) <= radius_squared;
    }

    let projection = qx * dx + qy * dy;
    if projection <= 0 {
        return i128::from(qx) * i128::from(qx) + i128::from(qy) * i128::from(qy) <= radius_squared;
    }
    if projection >= length_squared {
        let ex = point[0] - i64::from(b[0]);
        let ey = point[1] - i64::from(b[1]);
        return i128::from(ex) * i128::from(ex) + i128::from(ey) * i128::from(ey) <= radius_squared;
    }

    let cross = i128::from(dx) * i128::from(qy) - i128::from(dy) * i128::from(qx);
    cross * cross <= radius_squared * i128::from(length_squared)
}

fn crossing_at_or_left(crossing: (i64, i64, i32), x: i64) -> bool {
    i128::from(crossing.0) <= i128::from(x) * i128::from(crossing.1)
}

pub fn coverage(
    contours: &[Vec<Point>],
    x0: usize,
    x1: usize,
    y: usize,
    samples: u32,
    stroke: Option<f64>,
    nonzero: bool,
) -> Vec<u32> {
    let mut out = vec![0; x1.saturating_sub(x0)];
    if samples == 0 {
        return out;
    }

    let grid_contours: Vec<Vec<GridPoint>> = contours
        .iter()
        .map(|points| {
            points
                .iter()
                .map(|point| [grid_coordinate(point[0]), grid_coordinate(point[1])])
                .collect()
        })
        .collect();
    let radius = grid_coordinate(stroke.unwrap_or(0.0));

    if stroke.is_some() {
        let edges: Vec<_> = grid_contours
            .iter()
            .flat_map(|points| points.windows(2))
            .map(|pair| (pair[0], pair[1]))
            .collect();
        for sy in 0..samples {
            let py = sample_grid(y, sy, samples);
            let row_edges: Vec<_> = edges
                .iter()
                .copied()
                .filter(|(a, b)| {
                    py >= i64::from(a[1].min(b[1])) - i64::from(radius)
                        && py <= i64::from(a[1].max(b[1])) + i64::from(radius)
                })
                .collect();
            for (n, covered) in out.iter_mut().enumerate() {
                for sx in 0..samples {
                    let point = [sample_grid(x0 + n, sx, samples), py];
                    *covered += u32::from(
                        row_edges
                            .iter()
                            .any(|(a, b)| stroke_covers(point, *a, *b, radius)),
                    );
                }
            }
        }
        return out;
    }

    for sy in 0..samples {
        let py = sample_grid(y, sy, samples);
        let mut crossings = Vec::<(i64, i64, i32)>::new();
        for points in grid_contours.iter().filter(|points| points.len() >= 3) {
            for index in 0..points.len() {
                let a = points[index];
                let b = points[(index + 1) % points.len()];
                let ay = i64::from(a[1]);
                let by = i64::from(b[1]);
                if (ay <= py && by > py) || (by <= py && ay > py) {
                    let dx = i64::from(b[0]) - i64::from(a[0]);
                    let dy = by - ay;
                    let raw_numerator = i64::from(a[0]) * dy + (py - ay) * dx;
                    let denominator = dy.abs();
                    let numerator = if dy > 0 {
                        raw_numerator
                    } else {
                        -raw_numerator
                    };
                    crossings.push((numerator, denominator, if dy > 0 { 1 } else { -1 }));
                }
            }
        }
        crossings.sort_unstable_by(|a, b| {
            (i128::from(a.0) * i128::from(b.1)).cmp(&(i128::from(b.0) * i128::from(a.1)))
        });
        let mut winding: i32 = crossings.iter().map(|crossing| crossing.2).sum();
        let mut edge = 0;
        for (n, covered) in out.iter_mut().enumerate() {
            for sx in 0..samples {
                let px = sample_grid(x0 + n, sx, samples);
                while edge < crossings.len() && crossing_at_or_left(crossings[edge], px) {
                    winding -= crossings[edge].2;
                    edge += 1;
                }
                let inside = if nonzero {
                    winding != 0
                } else {
                    winding & 1 != 0
                };
                *covered += u32::from(inside);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(
        contours: &[Vec<Point>],
        point: [i64; 2],
        stroke: Option<f64>,
        nonzero: bool,
    ) -> bool {
        if let Some(radius) = stroke {
            let radius = grid_coordinate(radius);
            return contours.iter().any(|points| {
                let grid: Vec<GridPoint> = points
                    .iter()
                    .map(|point| [grid_coordinate(point[0]), grid_coordinate(point[1])])
                    .collect();
                grid.windows(2)
                    .any(|edge| stroke_covers(point, edge[0], edge[1], radius))
            });
        }

        let mut winding = 0i32;
        for points in contours {
            let grid: Vec<GridPoint> = points
                .iter()
                .map(|point| [grid_coordinate(point[0]), grid_coordinate(point[1])])
                .collect();
            if grid.len() >= 3 {
                for index in 0..grid.len() {
                    let a = grid[index];
                    let b = grid[(index + 1) % grid.len()];
                    let ay = i64::from(a[1]);
                    let by = i64::from(b[1]);
                    if (ay <= point[1] && by > point[1]) || (by <= point[1] && ay > point[1]) {
                        let dx = i64::from(b[0]) - i64::from(a[0]);
                        let dy = by - ay;
                        let relative = dx * (point[1] - ay) - dy * (point[0] - i64::from(a[0]));
                        if (dy > 0 && relative > 0) || (dy < 0 && relative < 0) {
                            winding += if dy > 0 { 1 } else { -1 };
                        }
                    }
                }
            }
        }
        if nonzero {
            winding != 0
        } else {
            winding & 1 != 0
        }
    }

    fn compare(contours: &[Vec<Point>]) {
        for samples in [1, 2, 4] {
            for stroke in [None, Some(0.0), Some(0.5), Some(2.0)] {
                for nonzero in [false, true] {
                    for y in 0..12 {
                        let got = coverage(contours, 1, 14, y, samples, stroke, nonzero);
                        let want: Vec<u32> = (1..14)
                            .map(|x| {
                                (0..samples)
                                    .flat_map(|sy| (0..samples).map(move |sx| (sy, sx)))
                                    .map(|(sy, sx)| {
                                        u32::from(reference(
                                            contours,
                                            [
                                                sample_grid(x, sx, samples),
                                                sample_grid(y, sy, samples),
                                            ],
                                            stroke,
                                            nonzero,
                                        ))
                                    })
                                    .sum()
                            })
                            .collect();
                        assert_eq!(
                            got, want,
                            "y={y} samples={samples} stroke={stroke:?} nonzero={nonzero}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn holes_shared_vertices_open_paths_and_degenerate_strokes() {
        compare(&[
            vec![[1.5, 1.5], [11.5, 1.5], [11.5, 9.5], [1.5, 9.5]],
            vec![[3.5, 3.5], [8.5, 3.5], [8.5, 7.5], [3.5, 7.5]],
        ]);
        compare(&[
            vec![[2.0, 2.0], [10.0, 10.0], [2.0, 10.0], [10.0, 2.0]],
            vec![[5.0, 5.0], [5.0, 5.0]],
            vec![[0.0, 4.0], [13.0, 4.0]],
        ]);
        compare(&[
            vec![[6.125, 0.125], [6.125, 11.125]],
            vec![[0.125, 6.125], [13.125, 6.125]],
            vec![[6.0, 6.0]],
        ]);
    }

    #[test]
    fn varied_concave_and_intersecting_contours_match_point_law() {
        let mut seed = 123456789u32;
        for _ in 0..24 {
            let mut contours = Vec::new();
            for _ in 0..3 {
                let mut points = Vec::new();
                for _ in 0..8 {
                    let mut point = [0.0; 2];
                    for coordinate in &mut point {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        *coordinate = (seed % 128) as f64 / 8.0 - 2.0;
                    }
                    points.push(point);
                }
                contours.push(points);
            }
            compare(&contours);
        }
    }
}
