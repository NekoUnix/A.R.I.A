struct Grid {
    point_offset: u32,
    columns: u32,
    rows: u32,
    flags: u32,
    center: vec4<f32>,
    basis_u: vec4<f32>,
    basis_v: vec4<f32>,
};

struct Sample {
    position: vec2<f32>,
    grid: u32,
    output_index: u32,
};

struct ExteriorAxis {
    lower: f32,
    upper: f32,
    fraction: f32,
    first: i32,
    second: i32,
};

@group(0) @binding(0) var<storage, read_write> control_points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> samples: array<Sample>;
@group(0) @binding(2) var<storage, read_write> output_points: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read> grids: array<Grid>;

fn affine(grid: Grid, uv: vec2<f32>) -> vec2<f32> {
    if (grid.flags & 4u) != 0u {
        let p00 = control_points[grid.point_offset];
        let p10 = control_points[grid.point_offset + grid.columns];
        let p01 = control_points[grid.point_offset + grid.rows * (grid.columns + 1u)];
        let p11 = control_points[grid.point_offset + grid.rows * (grid.columns + 1u) + grid.columns];
        let diagonal = p11 - p00;
        let opposite = p10 - p01;
        let basis_u = (diagonal + opposite) * 0.5;
        let basis_v = (diagonal - opposite) * 0.5;
        let center = (p00 + p10 + p01 + p11) * 0.25 - diagonal * 0.5;
        return center + basis_u * uv.x + basis_v * uv.y;
    }
    return grid.center.xy + grid.basis_u.xy * uv.x + grid.basis_v.xy * uv.y;
}

fn cell(corners: array<vec2<f32>, 4>, u: f32, v: f32, quad: bool) -> vec2<f32> {
    if quad {
        return mix(mix(corners[0], corners[1], u), mix(corners[2], corners[3], u), v);
    }
    if u + v <= 1.0 {
        return corners[0] * (1.0 - u - v) + corners[1] * u + corners[2] * v;
    }
    return corners[1] * (1.0 - v) + corners[3] * (u + v - 1.0) + corners[2] * (1.0 - u);
}

fn grid_point(grid: Grid, column: u32, row: u32) -> vec2<f32> {
    return control_points[grid.point_offset + row * (grid.columns + 1u) + column];
}

fn interior(grid: Grid, uv: vec2<f32>) -> vec2<f32> {
    let scaled = uv * vec2<f32>(f32(grid.columns), f32(grid.rows));
    let column = min(u32(scaled.x), grid.columns - 1u);
    let row = min(u32(scaled.y), grid.rows - 1u);
    let u = scaled.x - f32(column);
    let v = scaled.y - f32(row);
    let corners = array<vec2<f32>, 4>(
        grid_point(grid, column, row),
        grid_point(grid, column + 1u, row),
        grid_point(grid, column, row + 1u),
        grid_point(grid, column + 1u, row + 1u),
    );
    return cell(corners, u, v, (grid.flags & 1u) != 0u);
}

fn exterior_axis(value: f32, cells: u32) -> ExteriorAxis {
    if value <= 0.0 {
        return ExteriorAxis(-2.0, 0.0, (value + 2.0) * 0.5, -1, 0);
    }
    if value >= 1.0 {
        return ExteriorAxis(1.0, 3.0, (value - 1.0) * 0.5, i32(cells), -1);
    }
    let scaled = value * f32(cells);
    let lower = min(u32(scaled), cells - 1u);
    return ExteriorAxis(
        f32(lower) / f32(cells),
        f32(lower + 1u) / f32(cells),
        scaled - f32(lower),
        i32(lower),
        i32(lower + 1u),
    );
}

fn exterior_corner(grid: Grid, x: ExteriorAxis, y: ExteriorAxis, right: bool, bottom: bool) -> vec2<f32> {
    let column = select(x.first, x.second, right);
    let row = select(y.first, y.second, bottom);
    if column >= 0 && row >= 0 {
        return grid_point(grid, u32(column), u32(row));
    }
    return affine(grid, vec2<f32>(
        select(x.lower, x.upper, right),
        select(y.lower, y.upper, bottom),
    ));
}

fn sample_extended(grid: Grid, uv: vec2<f32>) -> vec2<f32> {
    if (grid.flags & 2u) != 0u {
        return affine(grid, uv);
    }
    if uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0 {
        return interior(grid, uv);
    }
    if uv.x <= -2.0 || uv.x >= 3.0 || uv.y <= -2.0 || uv.y >= 3.0 {
        return affine(grid, uv);
    }
    let x = exterior_axis(uv.x, grid.columns);
    let y = exterior_axis(uv.y, grid.rows);
    let corners = array<vec2<f32>, 4>(
        exterior_corner(grid, x, y, false, false),
        exterior_corner(grid, x, y, true, false),
        exterior_corner(grid, x, y, false, true),
        exterior_corner(grid, x, y, true, true),
    );
    return cell(corners, x.fraction, y.fraction, false);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index < arrayLength(&samples) {
        let sample = samples[index];
        output_points[index] = sample_extended(grids[sample.grid], sample.position);
    }
}

// Dispatch one parent-depth level at a time. Parent ranges are complete before
// the next dispatch, so children can update disjoint ranges in the same buffer.
@compute @workgroup_size(64)
fn resolve_level(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index < arrayLength(&samples) {
        let sample = samples[index];
        control_points[sample.output_index] = sample_extended(grids[sample.grid], sample.position);
    }
}

// The local positions were blended into control_points by blend_warp_keys.
// Parent and child ranges are disjoint within a depth pass.
@compute @workgroup_size(64)
fn resolve_points(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index < arrayLength(&samples) {
        let sample = samples[index];
        let local = control_points[sample.output_index];
        control_points[sample.output_index] = sample_extended(grids[sample.grid], local);
    }
}
