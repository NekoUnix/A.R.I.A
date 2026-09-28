struct Work {
    output_index: u32,
    local_index: u32,
    active_start: u32,
    active_count: u32,
};

struct ActiveKey {
    source_offset: u32,
    weight: f32,
};

@group(0) @binding(0) var<storage, read> delta_points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> work: array<Work>;
@group(0) @binding(2) var<storage, read> active_keys: array<ActiveKey>;
@group(0) @binding(3) var<storage, read_write> positions: array<vec2<f32>>;

@compute @workgroup_size(64)
fn apply_blend_deltas(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&work) {
        return;
    }
    let item = work[index];
    var point = positions[item.output_index];
    for (var key = 0u; key < item.active_count; key++) {
        let selected = active_keys[item.active_start + key];
        point += delta_points[selected.source_offset + item.local_index] * selected.weight;
    }
    positions[item.output_index] = point;
}
