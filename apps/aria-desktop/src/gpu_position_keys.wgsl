struct Work {
    output_index: u32,
    local_index: u32,
    active_start: u32,
    node_index: u32,
};

struct ActiveKey {
    source_offset: u32,
    weight: f32,
};

@group(0) @binding(0) var<storage, read> source_points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> work: array<Work>;
@group(0) @binding(2) var<storage, read> active_keys: array<ActiveKey>;
@group(0) @binding(3) var<storage, read> active_counts: array<u32>;
@group(0) @binding(4) var<storage, read_write> output_points: array<vec2<f32>>;

@compute @workgroup_size(64)
fn blend_positions(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&work) {
        return;
    }
    let item = work[index];
    let count = active_counts[item.node_index];
    var position = vec2<f32>(0.0);
    for (var key = 0u; key < count; key++) {
        let selected = active_keys[item.active_start + key];
        position += source_points[selected.source_offset + item.local_index] * selected.weight;
    }
    output_points[item.output_index] = position;
}
