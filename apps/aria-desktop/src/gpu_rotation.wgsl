struct Rotation {
    origin: vec2<f32>,
    x_axis: vec2<f32>,
    y_axis: vec2<f32>,
};

struct Sample {
    position: vec2<f32>,
    rotation: u32,
    output_index: u32,
};

@group(0) @binding(0) var<storage, read_write> points: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> samples: array<Sample>;
@group(0) @binding(2) var<storage, read> rotations: array<Rotation>;

@compute @workgroup_size(64)
fn resolve_rotation(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= arrayLength(&samples) {
        return;
    }
    let sample = samples[index];
    let rotation = rotations[sample.rotation];
    let local = points[sample.output_index];
    points[sample.output_index] = rotation.origin
        + rotation.x_axis * local.x
        + rotation.y_axis * local.y;
}
