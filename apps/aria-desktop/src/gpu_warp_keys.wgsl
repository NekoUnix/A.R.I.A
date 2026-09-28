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

@group(0) @binding(0) var<storage, read> keyform_points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> work: array<Work>;
@group(0) @binding(2) var<storage, read> active_keys: array<ActiveKey>;
@group(0) @binding(3) var<storage, read_write> control_points: array<vec2<f32>>;

@compute @workgroup_size(64)
fn blend_warp_keys(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&work) {
        return;
    }
    let item = work[index];
    var position = vec2<f32>(0.0);
    for (var key = 0u; key < item.active_count; key++) {
        let selected = active_keys[item.active_start + key];
        position += keyform_points[selected.source_offset + item.local_index] * selected.weight;
    }
    control_points[item.output_index] = position;
}
