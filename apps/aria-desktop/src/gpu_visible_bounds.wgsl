@group(0) @binding(0) var<storage, read> positions: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read> mesh_ids: array<u32>;
@group(0) @binding(2) var<storage, read> visible: array<u32>;
@group(0) @binding(3) var<storage, read_write> groups: array<vec4<f32>>;

var<workgroup> partial: array<vec4<f32>, 64>;

fn empty_bounds() -> vec4<f32> {
    return vec4<f32>(1e30, 1e30, -1e30, -1e30);
}

fn combine(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(min(a.xy, b.xy), max(a.zw, b.zw));
}

@compute @workgroup_size(64)
fn mesh_bounds(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    var bounds = empty_bounds();
    if (id.x < arrayLength(&mesh_ids) && visible[mesh_ids[id.x]] != 0u) {
        let point = positions[id.x];
        bounds = vec4<f32>(point, point);
    }
    partial[local.x] = bounds;
    workgroupBarrier();
    var stride = 32u;
    loop {
        if (local.x < stride) {
            partial[local.x] = combine(partial[local.x], partial[local.x + stride]);
        }
        workgroupBarrier();
        if (stride == 1u) { break; }
        stride = stride / 2u;
    }
    if (local.x == 0u) { groups[group.x] = partial[0]; }
}

