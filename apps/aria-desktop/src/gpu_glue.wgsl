struct GluePair {
    left: u32,
    right: u32,
    left_weight: f32,
    right_weight: f32,
    glue_index: u32,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> pairs: array<GluePair>;
@group(0) @binding(2) var<storage, read> intensities: array<f32>;

@compute @workgroup_size(64)
fn apply_glue(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&pairs) {
        return;
    }
    let pair = pairs[index];
    let left = positions[pair.left];
    let right = positions[pair.right];
    let delta = right - left;
    let intensity = intensities[pair.glue_index];
    positions[pair.left] = left + delta * (intensity * pair.left_weight);
    positions[pair.right] = right - delta * (intensity * pair.right_weight);
}
