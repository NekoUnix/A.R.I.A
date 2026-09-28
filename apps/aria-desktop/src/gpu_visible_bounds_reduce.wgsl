@group(0) @binding(0) var<storage, read> input_groups: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> final_bounds: array<vec4<f32>>;

var<workgroup> partial: array<vec4<f32>, 64>;

fn empty_bounds() -> vec4<f32> {
    return vec4<f32>(1e30, 1e30, -1e30, -1e30);
}

fn combine(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(min(a.xy, b.xy), max(a.zw, b.zw));
}

@compute @workgroup_size(64)
fn reduce_bounds(@builtin(local_invocation_id) local: vec3<u32>) {
    var bounds = empty_bounds();
    for (var i = local.x; i < arrayLength(&input_groups); i += 64u) {
        bounds = combine(bounds, input_groups[i]);
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
    if (local.x == 0u) { final_bounds[0] = partial[0]; }
}
