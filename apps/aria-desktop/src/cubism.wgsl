struct Style { multiply: vec4<f32>, screen: vec4<f32>, control: vec4<f32> }
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(1) @binding(0) var mask: texture_2d<f32>;
@group(1) @binding(1) var<uniform> style: Style;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vertex(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}
// A mask uses texture alpha even when its drawable is invisible/zero opacity.
@fragment fn mask_fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas, linear_sampler, in.uv).a;
    return vec4<f32>(alpha);
}
@fragment fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, linear_sampler, in.uv);
    let multiplied = texel.rgb * style.multiply.rgb;
    let color = multiplied + style.screen.rgb - multiplied * style.screen.rgb;
    var coverage = 1.0;
    if style.control.y > 0.5 {
        let mask_alpha = textureSample(mask, linear_sampler, in.position.xy / style.control.zw).a;
        coverage = select(mask_alpha, 1.0 - mask_alpha, style.control.y > 1.5);
    }
    let alpha = texel.a * style.control.x * coverage;
    return vec4<f32>(color * alpha, alpha);
}
