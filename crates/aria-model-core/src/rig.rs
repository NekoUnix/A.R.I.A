//! Pure Rust building blocks for ARIA's model evaluator.

use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyWeight {
    pub index: usize,
    pub weight: f32,
}

/// Resolve one authored parameter axis into one or two adjacent key weights.
/// The returned weights sum to one for all finite inputs.
pub fn linear_keys(keys: &[f32], value: f32) -> Result<Vec<KeyWeight>> {
    ensure!(!keys.is_empty() && value.is_finite(), "Invalid key axis");
    ensure!(keys.iter().all(|v| v.is_finite()), "Non-finite key");
    ensure!(
        keys.windows(2).all(|pair| pair[0] < pair[1]),
        "Key axis must be strictly increasing"
    );
    if value <= keys[0] {
        return Ok(vec![KeyWeight {
            index: 0,
            weight: 1.0,
        }]);
    }
    if value >= keys[keys.len() - 1] {
        return Ok(vec![KeyWeight {
            index: keys.len() - 1,
            weight: 1.0,
        }]);
    }
    let upper = keys.partition_point(|&key| key < value);
    if keys[upper] == value {
        return Ok(vec![KeyWeight {
            index: upper,
            weight: 1.0,
        }]);
    }
    let t = (value - keys[upper - 1]) / (keys[upper] - keys[upper - 1]);
    Ok(vec![
        KeyWeight {
            index: upper - 1,
            weight: 1.0 - t,
        },
        KeyWeight {
            index: upper,
            weight: t,
        },
    ])
}

#[derive(Clone, Debug, PartialEq)]
pub struct WarpGrid {
    columns: usize,
    rows: usize,
    points: Vec<Point>,
}

impl WarpGrid {
    pub fn new(columns: usize, rows: usize, points: Vec<Point>) -> Result<Self> {
        ensure!(
            columns > 0 && rows > 0 && columns <= 256 && rows <= 256,
            "Invalid warp grid dimensions"
        );
        let expected = (columns + 1)
            .checked_mul(rows + 1)
            .ok_or_else(|| anyhow::anyhow!("Warp grid size overflow"))?;
        ensure!(points.len() == expected, "Warp grid point count differs");
        ensure!(
            points.iter().all(|p| p.x.is_finite() && p.y.is_finite()),
            "Non-finite warp control point"
        );
        Ok(Self {
            columns,
            rows,
            points,
        })
    }

    /// Evaluate the interior of a rectangular quadrilateral warp.
    /// Extrapolation and triangular cells require separate paths.
    pub fn sample_quad(&self, uv: Point) -> Result<Point> {
        ensure!(
            uv.x.is_finite()
                && uv.y.is_finite()
                && (0.0..=1.0).contains(&uv.x)
                && (0.0..=1.0).contains(&uv.y),
            "Warp coordinate lies outside the supported grid interior"
        );
        let scaled_x = uv.x * self.columns as f32;
        let scaled_y = uv.y * self.rows as f32;
        let col = (scaled_x as usize).min(self.columns - 1);
        let row = (scaled_y as usize).min(self.rows - 1);
        let local_x = scaled_x - col as f32;
        let local_y = scaled_y - row as f32;
        let top_left = row * (self.columns + 1) + col;
        let stride = self.columns + 1;
        Ok(bilinear_cell(
            [
                self.points[top_left],
                self.points[top_left + 1],
                self.points[top_left + stride],
                self.points[top_left + stride + 1],
            ],
            local_x,
            local_y,
        ))
    }
}

impl Point {
    fn mix(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

/// Blend corresponding control points into a caller-owned output buffer.
/// Source count and point count are checked before any write occurs.
pub fn blend_points(sources: &[&[Point]], weights: &[f32], out: &mut [Point]) -> Result<()> {
    ensure!(
        sources.len() == weights.len() && !sources.is_empty(),
        "Keyform and weight counts differ"
    );
    ensure!(
        weights.iter().all(|w| w.is_finite()),
        "Non-finite keyform weight"
    );
    ensure!(
        sources.iter().all(|s| s.len() == out.len()),
        "Keyform point counts differ"
    );
    out.fill(Point::default());
    for (&source, &weight) in sources.iter().zip(weights) {
        for (dst, src) in out.iter_mut().zip(source) {
            dst.x += src.x * weight;
            dst.y += src.y * weight;
        }
    }
    Ok(())
}

/// Sample a quadrilateral warp cell at local coordinates. Grid lookup and
/// out-of-grid extrapolation will be layered over this primitive.
pub fn bilinear_cell(corners: [Point; 4], u: f32, v: f32) -> Point {
    corners[0]
        .mix(corners[1], u)
        .mix(corners[2].mix(corners[3], u), v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blended_keyforms_and_warp_cell_have_expected_geometry() {
        let a = [Point { x: 0.0, y: 0.0 }, Point { x: 2.0, y: 0.0 }];
        let b = [Point { x: 0.0, y: 2.0 }, Point { x: 2.0, y: 2.0 }];
        let mut out = [Point::default(); 2];
        blend_points(&[&a, &b], &[0.25, 0.75], &mut out).unwrap();
        assert_eq!(out, [Point { x: 0.0, y: 1.5 }, Point { x: 2.0, y: 1.5 }]);
        assert_eq!(
            bilinear_cell([a[0], a[1], b[0], b[1]], 0.5, 0.5),
            Point { x: 1.0, y: 1.0 }
        );
    }

    #[test]
    fn invalid_blend_is_rejected_without_writing_output() {
        let a = [Point { x: 5.0, y: 5.0 }];
        let mut out = [Point { x: 9.0, y: 9.0 }];
        assert!(blend_points(&[&a], &[], &mut out).is_err());
        assert_eq!(out[0].x, 9.0);
    }

    #[test]
    fn linear_parameter_keys_interpolate_and_clamp() {
        let keys = [-1.0, 0.0, 2.0];
        assert_eq!(
            linear_keys(&keys, -5.0).unwrap(),
            vec![KeyWeight {
                index: 0,
                weight: 1.0
            }]
        );
        assert_eq!(
            linear_keys(&keys, 0.0).unwrap(),
            vec![KeyWeight {
                index: 1,
                weight: 1.0
            }]
        );
        assert_eq!(
            linear_keys(&keys, 1.0).unwrap(),
            vec![
                KeyWeight {
                    index: 1,
                    weight: 0.5
                },
                KeyWeight {
                    index: 2,
                    weight: 0.5
                }
            ]
        );
        assert!(linear_keys(&[0.0, 0.0], 0.0).is_err());
    }

    #[test]
    fn quadrilateral_grid_samples_edges_and_middle() {
        let grid = WarpGrid::new(
            1,
            1,
            vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 2.0, y: 0.0 },
                Point { x: 0.0, y: 2.0 },
                Point { x: 2.0, y: 2.0 },
            ],
        )
        .unwrap();
        assert_eq!(
            grid.sample_quad(Point { x: 0.5, y: 0.5 }).unwrap(),
            Point { x: 1.0, y: 1.0 }
        );
        assert_eq!(
            grid.sample_quad(Point { x: 1.0, y: 1.0 }).unwrap(),
            Point { x: 2.0, y: 2.0 }
        );
        assert!(grid.sample_quad(Point { x: 1.1, y: 0.0 }).is_err());
    }
}
