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
    affine_center: Point,
    affine_u: Point,
    affine_v: Point,
    affine: bool,
}

/// Affine rotation about a model-space origin, with authored reflection and
/// scale. The coefficients are computed once for all vertices in a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationTransform {
    origin: Point,
    xx: f32,
    xy: f32,
    yx: f32,
    yy: f32,
}

impl RotationTransform {
    pub fn new(origin: Point, degrees: f32, scale: f32, reflect: [bool; 2]) -> Result<Self> {
        ensure!(
            origin.x.is_finite()
                && origin.y.is_finite()
                && degrees.is_finite()
                && scale.is_finite(),
            "Invalid rotation transform"
        );
        let (sine, cosine) = degrees.to_radians().sin_cos();
        let sx = if reflect[0] { -scale } else { scale };
        let sy = if reflect[1] { -scale } else { scale };
        Ok(Self {
            origin,
            xx: cosine * sx,
            xy: -sine * sy,
            yx: sine * sx,
            yy: cosine * sy,
        })
    }

    pub fn apply(self, point: Point) -> Point {
        Point {
            x: self.origin.x + self.xx * point.x + self.xy * point.y,
            y: self.origin.y + self.yx * point.x + self.yy * point.y,
        }
    }

    /// Affine columns for applying this transform to GPU-resident positions.
    pub fn affine_columns(self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        (
            [self.origin.x, self.origin.y],
            [self.xx, self.yx],
            [self.xy, self.yy],
        )
    }
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
        let p00 = points[0];
        let p10 = points[columns];
        let p01 = points[rows * (columns + 1)];
        let p11 = points[rows * (columns + 1) + columns];
        let diagonal = p11.sub(p00);
        let opposite = p10.sub(p01);
        let affine_v = diagonal.sub(opposite).scale(0.5);
        let affine_u = diagonal.add(opposite).scale(0.5);
        let affine_center = p00
            .add(p10)
            .add(p01)
            .add(p11)
            .scale(0.25)
            .sub(diagonal.scale(0.5));
        let affine = points.iter().enumerate().all(|(index, point)| {
            let u = (index % (columns + 1)) as f32 / columns as f32;
            let v = (index / (columns + 1)) as f32 / rows as f32;
            let expected = affine_center.add(affine_u.scale(u)).add(affine_v.scale(v));
            (point.x - expected.x).abs() <= 0.000001 && (point.y - expected.y).abs() <= 0.000001
        });
        Ok(Self {
            columns,
            rows,
            points,
            affine_center,
            affine_u,
            affine_v,
            affine,
        })
    }

    /// Evaluate the interior of a rectangular quadrilateral warp.
    /// Extrapolation and triangular cells require separate paths.
    pub fn sample_quad(&self, uv: Point) -> Result<Point> {
        self.sample_interior(uv, true)
    }

    /// Sample an interior grid cell using the authored interpolation mode.
    /// Exterior points are rejected until the separate extrapolation path is
    /// validated against local models.
    pub fn sample_interior(&self, uv: Point, quad: bool) -> Result<Point> {
        ensure!(
            uv.x.is_finite()
                && uv.y.is_finite()
                && (0.0..=1.0).contains(&uv.x)
                && (0.0..=1.0).contains(&uv.y),
            "Warp coordinate lies outside the supported grid interior"
        );
        Ok(self.sample_interior_finite(uv, quad))
    }

    fn sample_interior_finite(&self, uv: Point, quad: bool) -> Point {
        let scaled_x = uv.x * self.columns as f32;
        let scaled_y = uv.y * self.rows as f32;
        let col = (scaled_x as usize).min(self.columns - 1);
        let row = (scaled_y as usize).min(self.rows - 1);
        let local_x = scaled_x - col as f32;
        let local_y = scaled_y - row as f32;
        let top_left = row * (self.columns + 1) + col;
        let stride = self.columns + 1;
        let corners = [
            self.points[top_left],
            self.points[top_left + 1],
            self.points[top_left + stride],
            self.points[top_left + stride + 1],
        ];
        if quad {
            bilinear_cell(corners, local_x, local_y)
        } else {
            triangle_cell(corners, local_x, local_y)
        }
    }

    /// Extend the grid using a diagonal-derived affine boundary basis. Near
    /// the grid, virtual boundary cells meet the authored edge points; far
    /// outside, the same basis gives a bounded-cost affine continuation.
    pub fn sample_extended(&self, uv: Point, quad: bool) -> Result<Point> {
        ensure!(
            uv.x.is_finite() && uv.y.is_finite(),
            "Non-finite warp coordinate"
        );
        Ok(self.sample_extended_finite(uv, quad))
    }

    /// GeometryEvaluator has already validated decoded keyforms and finite
    /// parameter weights. It checks completed ArtMesh positions after use.
    pub(crate) fn sample_extended_finite(&self, uv: Point, quad: bool) -> Point {
        if self.affine {
            return self
                .affine_center
                .add(self.affine_u.scale(uv.x))
                .add(self.affine_v.scale(uv.y));
        }
        if (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y) {
            return self.sample_interior_finite(uv, quad);
        }
        let stride = self.columns + 1;
        let affine = |u: f32, v: f32| {
            self.affine_center
                .add(self.affine_u.scale(u))
                .add(self.affine_v.scale(v))
        };
        if uv.x <= -2.0 || uv.x >= 3.0 || uv.y <= -2.0 || uv.y >= 3.0 {
            return affine(uv.x, uv.y);
        }
        let x = exterior_axis(uv.x, self.columns);
        let y = exterior_axis(uv.y, self.rows);
        let corner = |xi: usize, yi: usize| {
            let column = if xi == 0 { x.3 } else { x.4 };
            let row = if yi == 0 { y.3 } else { y.4 };
            match (column, row) {
                (Some(column), Some(row)) => self.points[row * stride + column],
                _ => affine(
                    if xi == 0 { x.0 } else { x.1 },
                    if yi == 0 { y.0 } else { y.1 },
                ),
            }
        };
        let corners = [corner(0, 0), corner(1, 0), corner(0, 1), corner(1, 1)];
        triangle_cell(corners, x.2, y.2)
    }
}

/// Bounds, interpolation fraction, and real grid-point indexes for one axis.
fn exterior_axis(value: f32, cells: usize) -> (f32, f32, f32, Option<usize>, Option<usize>) {
    if value <= 0.0 {
        (-2.0, 0.0, (value + 2.0) * 0.5, None, Some(0))
    } else if value >= 1.0 {
        (1.0, 3.0, (value - 1.0) * 0.5, Some(cells), None)
    } else {
        let scaled = value * cells as f32;
        let lower = (scaled as usize).min(cells - 1);
        (
            lower as f32 / cells as f32,
            (lower + 1) as f32 / cells as f32,
            scaled - lower as f32,
            Some(lower),
            Some(lower + 1),
        )
    }
}

impl Point {
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
    fn scale(self, scalar: f32) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
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

/// Triangulated cell interpolation preserves the authored diagonal rather
/// than introducing the bilinear cross-term of a quadrilateral.
pub fn triangle_cell(corners: [Point; 4], u: f32, v: f32) -> Point {
    let [p00, p10, p01, p11] = corners;
    if u + v <= 1.0 {
        Point {
            x: p00.x * (1.0 - u - v) + p10.x * u + p01.x * v,
            y: p00.y * (1.0 - u - v) + p10.y * u + p01.y * v,
        }
    } else {
        Point {
            x: p10.x * (1.0 - v) + p11.x * (u + v - 1.0) + p01.x * (1.0 - u),
            y: p10.y * (1.0 - v) + p11.y * (u + v - 1.0) + p01.y * (1.0 - u),
        }
    }
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

    #[test]
    fn triangle_interpolation_uses_two_planar_cells() {
        let corners = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 2.0, y: 0.0 },
            Point { x: 0.0, y: 2.0 },
            Point { x: 3.0, y: 3.0 },
        ];
        assert_eq!(triangle_cell(corners, 0.25, 0.25), Point { x: 0.5, y: 0.5 });
        assert_eq!(triangle_cell(corners, 0.75, 0.75), Point { x: 2.0, y: 2.0 });
        assert_ne!(
            triangle_cell(corners, 0.25, 0.25),
            bilinear_cell(corners, 0.25, 0.25)
        );
    }

    #[test]
    fn rotation_transform_preserves_origin_and_reflection() {
        let transform =
            RotationTransform::new(Point { x: 2.0, y: -3.0 }, 90.0, 2.0, [false, false]).unwrap();
        let result = transform.apply(Point { x: 1.0, y: 0.0 });
        assert!((result.x - 2.0).abs() < 1e-6);
        assert!((result.y + 1.0).abs() < 1e-6);
        let reflected = RotationTransform::new(Point::default(), 0.0, 1.0, [true, false])
            .unwrap()
            .apply(Point { x: 3.0, y: 4.0 });
        assert_eq!(reflected, Point { x: -3.0, y: 4.0 });
        assert!(RotationTransform::new(Point::default(), f32::NAN, 1.0, [false; 2]).is_err());
    }

    #[test]
    fn exterior_warp_sampling_is_affine_for_an_affine_grid() {
        let grid = WarpGrid::new(
            1,
            1,
            vec![
                Point { x: 1.0, y: -2.0 },
                Point { x: 3.0, y: -2.0 },
                Point { x: 1.0, y: 1.0 },
                Point { x: 3.0, y: 1.0 },
            ],
        )
        .unwrap();
        assert!(grid.affine);
        for point in [
            Point { x: -0.5, y: 0.5 },
            Point { x: 0.5, y: 1.5 },
            Point { x: -4.0, y: 5.0 },
        ] {
            let result = grid.sample_extended(point, true).unwrap();
            assert!((result.x - (1.0 + 2.0 * point.x)).abs() < 1e-5);
            assert!((result.y - (-2.0 + 3.0 * point.y)).abs() < 1e-5);
        }
    }

    #[test]
    fn bent_grid_keeps_cell_interpolation() {
        let grid = WarpGrid::new(
            2,
            1,
            vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 0.25 },
                Point { x: 2.0, y: 0.0 },
                Point { x: 0.0, y: 1.0 },
                Point { x: 1.0, y: 1.25 },
                Point { x: 2.0, y: 1.0 },
            ],
        )
        .unwrap();
        assert!(!grid.affine);
        assert_eq!(
            grid.sample_extended(Point { x: 0.5, y: 0.5 }, true)
                .unwrap(),
            Point { x: 1.0, y: 0.75 }
        );
    }
}
