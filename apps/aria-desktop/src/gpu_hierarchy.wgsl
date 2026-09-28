struct Node {
    kind: u32,
    parent: u32,
    point_offset: u32,
    columns: u32,
    rows: u32,
    flags: u32,
    base_angle: f32,
    padding: u32,
};

struct Task {
    node_index: u32,
    output_index: u32,
};

struct RotationLocal {
    origin: vec2<f32>,
    angle: f32,
    scale: f32,
    reflect: u32,
    padding: u32,
};

struct NodeState {
    origin: vec2<f32>,
    x_axis: vec2<f32>,
    y_axis: vec2<f32>,
    scale: f32,
    padding: f32,
};

struct ExteriorAxis {
    lower: f32,
    upper: f32,
    fraction: f32,
    first: i32,
    second: i32,
};

@group(0) @binding(0) var<storage, read_write> points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> tasks: array<Task>;
@group(0) @binding(2) var<storage, read> nodes: array<Node>;
@group(0) @binding(3) var<storage, read> rotation_locals: array<RotationLocal>;
@group(0) @binding(4) var<storage, read_write> states: array<NodeState>;

const ROOT_NODE: u32 = 0xffffffffu;

fn grid_point(grid: Node, column: u32, row: u32) -> vec2<f32> {
    return points[grid.point_offset + row * (grid.columns + 1u) + column];
}

fn affine(grid: Node, uv: vec2<f32>) -> vec2<f32> {
    let p00 = grid_point(grid, 0u, 0u);
    let p10 = grid_point(grid, grid.columns, 0u);
    let p01 = grid_point(grid, 0u, grid.rows);
    let p11 = grid_point(grid, grid.columns, grid.rows);
    let diagonal = p11 - p00;
    let opposite = p10 - p01;
    let basis_u = (diagonal + opposite) * 0.5;
    let basis_v = (diagonal - opposite) * 0.5;
    let center = (p00 + p10 + p01 + p11) * 0.25 - diagonal * 0.5;
    return center + basis_u * uv.x + basis_v * uv.y;
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

fn interior(grid: Node, uv: vec2<f32>) -> vec2<f32> {
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

fn exterior_corner(grid: Node, x: ExteriorAxis, y: ExteriorAxis, right: bool, bottom: bool) -> vec2<f32> {
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

fn sample_warp(grid: Node, uv: vec2<f32>) -> vec2<f32> {
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

fn apply_node(index: u32, point: vec2<f32>) -> vec2<f32> {
    let node = nodes[index];
    if node.kind == 0u {
        return sample_warp(node, point);
    }
    let state = states[index];
    return state.origin + state.x_axis * point.x + state.y_axis * point.y;
}

@compute @workgroup_size(64)
fn resolve_deformer(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&tasks) {
        return;
    }
    let task = tasks[index];
    let node = nodes[task.node_index];
    var parent_scale = 1.0;
    if node.parent != ROOT_NODE {
        parent_scale = states[node.parent].scale;
    }
    if node.kind == 0u {
        if task.output_index == ROOT_NODE || task.output_index == node.point_offset {
            states[task.node_index].scale = parent_scale;
        }
        if node.parent != ROOT_NODE && task.output_index != ROOT_NODE {
            points[task.output_index] = apply_node(node.parent, points[task.output_index]);
        }
        return;
    }
    let local = rotation_locals[task.node_index];
    var origin = local.origin;
    var angle = local.angle;
    if node.parent != ROOT_NODE {
        let transformed = apply_node(node.parent, origin);
        var step = select(-0.1, -10.0, nodes[node.parent].kind == 1u);
        var direction = vec2<f32>(0.0);
        for (var attempt = 0u; attempt < 16u; attempt++) {
            let probe = apply_node(node.parent, origin + vec2<f32>(0.0, step));
            direction = probe - transformed;
            if direction.x != 0.0 || direction.y != 0.0 {
                break;
            }
            step *= 0.1;
        }
        angle += degrees(atan2(direction.x, -direction.y));
        origin = transformed;
    }
    let scale = local.scale * parent_scale;
    let radians_value = radians(node.base_angle + angle);
    let sine = sin(radians_value);
    let cosine = cos(radians_value);
    let sx = select(scale, -scale, (local.reflect & 1u) != 0u);
    let sy = select(scale, -scale, (local.reflect & 2u) != 0u);
    states[task.node_index] = NodeState(
        origin,
        vec2<f32>(cosine * sx, sine * sx),
        vec2<f32>(-sine * sy, cosine * sy),
        scale,
        0.0,
    );
}

@compute @workgroup_size(64)
fn resolve_mesh(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&tasks) {
        return;
    }
    let task = tasks[index];
    points[task.output_index] = apply_node(task.node_index, points[task.output_index]);
}
