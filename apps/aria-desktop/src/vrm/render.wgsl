struct Frame {
    vp: mat4x4<f32>, front: mat4x4<f32>, view: mat4x4<f32>,
    settings: vec4<f32>,
};
struct Material {
    color:vec4<f32>, shade:vec4<f32>, emission:vec4<f32>, rim:vec4<f32>,
    params:vec4<f32>, extra:vec4<f32>, uv:vec4<f32>, outline:vec4<f32>, mode:vec4<f32>,
};
struct Joint {position:mat4x4<f32>,normal:mat4x4<f32>};
@group(0) @binding(0) var<uniform> frame:Frame;
@group(1) @binding(0) var base:texture_2d<f32>;
@group(1) @binding(1) var shade:texture_2d<f32>;
@group(1) @binding(2) var emission:texture_2d<f32>;
@group(1) @binding(3) var normal_map:texture_2d<f32>;
@group(1) @binding(4) var matcap:texture_2d<f32>;
@group(1) @binding(5) var sam:sampler;
@group(1) @binding(6) var<uniform> material:Material;
@group(2) @binding(0) var<storage,read> joints:array<Joint>;
struct Input {
    @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>,
    @location(2) uv:vec2<f32>, @location(3) color:vec4<f32>,
    @location(4) joint:vec4<u32>, @location(5) weight:vec4<f32>,
};
struct Out {
    @builtin(position) position:vec4<f32>, @location(0) normal:vec3<f32>,
    @location(1) uv:vec2<f32>, @location(2) color:vec4<f32>, @location(3) world:vec3<f32>,
};
fn vertex(i:Input, outline:bool)->Out {
    let m=joints[i.joint.x].position*i.weight.x+joints[i.joint.y].position*i.weight.y+joints[i.joint.z].position*i.weight.z+joints[i.joint.w].position*i.weight.w;
    let n=joints[i.joint.x].normal*i.weight.x+joints[i.joint.y].normal*i.weight.y+joints[i.joint.z].normal*i.weight.z+joints[i.joint.w].normal*i.weight.w;
    var o:Out;
    let world=frame.front*m*vec4(i.position,1);
    o.normal=normalize((frame.front*n*vec4(i.normal,0)).xyz);
    o.world=world.xyz;
    o.position=frame.vp*(world+vec4(o.normal*select(0.0,material.mode.x,outline),0));
    let uv=i.uv*material.uv.xy;
    o.uv=vec2(uv.x*material.mode.z-uv.y*material.mode.w,uv.x*material.mode.w+uv.y*material.mode.z)+material.uv.zw;
    o.color=i.color;
    return o;
}
@vertex fn vs(i:Input)->Out {return vertex(i,false);}
@vertex fn vs_outline(i:Input)->Out {return vertex(i,true);}
fn alpha(value:f32)->f32 {
    if(material.mode.y<1.5) {return 1.0;}
    return value;
}
@fragment fn fs(i:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
    let c=textureSample(base,sam,i.uv)*material.color*i.color;
    let dark=textureSample(shade,sam,i.uv).rgb*material.shade.rgb*i.color.rgb;
    let emit=textureSample(emission,sam,i.uv).rgb*material.emission.rgb;
    let map=textureSample(normal_map,sam,i.uv).xyz*2.0-1.0;
    let normal=select(-i.normal,i.normal,front);
    let p1=dpdx(i.world);let p2=dpdy(i.world);let t1=dpdx(i.uv);let t2=dpdy(i.uv);
    let t=cross(p2,normal)*t1.x+cross(normal,p1)*t2.x;
    let b=cross(p2,normal)*t1.y+cross(normal,p1)*t2.y;
    let scale=inverseSqrt(max(max(dot(t,t),dot(b,b)),0.0000001));
    let n=normalize(normal*map.z+(t*map.x+b*map.y)*scale*material.extra.x);
    let vn=normalize((frame.view*vec4(n,0)).xyz);
    let cap=textureSample(matcap,sam,vn.xy*vec2(0.5,-0.5)+vec2(0.5)).rgb;
    if(material.mode.y>0.5&&((material.mode.y<1.5&&c.a<material.params.z)||c.a<0.004)){discard;}
    let brightness=dot(n,normalize(vec3(-0.35,0.6,0.8)));
    let toon=clamp((brightness-material.params.x)/max(0.01,1.0-material.params.y),0.0,1.0);
    let lit=mix(dark,c.rgb,toon)*frame.settings.x;
    let rim=pow(clamp(1.0-abs(vn.z)+material.extra.z,0.0,1.0),max(0.01,material.extra.y))*material.rim.rgb;
    let rgb=mix(lit,c.rgb,material.params.w)+emit+cap*material.extra.w+rim;
    let a=alpha(c.a);
    return vec4(rgb*a,a);
}
@fragment fn fs_outline(i:Out)->@location(0) vec4<f32> {
    let c=textureSample(base,sam,i.uv)*material.color*i.color;
    if(material.mode.x<=0.0||(material.mode.y>0.5&&c.a<max(0.01,material.params.z))){discard;}
    let a=alpha(c.a)*material.outline.a;
    return vec4(material.outline.rgb*a,a);
}
