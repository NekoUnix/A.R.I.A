@group(0) @binding(0) var<storage, read_write> mesh_positions: array<vec2<f32>>;

@compute @workgroup_size(64)
fn orient_mesh(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= arrayLength(&mesh_positions)) {
        return;
    }
    let point = mesh_positions[id.x];
    mesh_positions[id.x] = vec2<f32>(point.x, -point.y);
}
