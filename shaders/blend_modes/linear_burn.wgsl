@group(0) @binding(0)
var inImage : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total : texture_storage_2d<rgba8unorm, read_write>;
@group(0) @binding(2)
var<uniform> opacity : f32;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
    let pixelCoord : vec2<u32> = GlobalInvocationID.xy;

    var pixel = vec4<f32>(textureLoad(inImage, vec2<i32>(pixelCoord)));
    pixel.a *= opacity;
    let alpha = pixel.a;

    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));
    let cur_alpha = cur.a;

    let blend_result = max(vec4<f32>(0.0), cur + pixel * alpha - vec4<f32>(alpha));

    let out_alpha = alpha + cur_alpha * (1.0 - alpha);

    textureStore(running_total, vec2<i32>(pixelCoord), vec4<f32>(blend_result.rgb, out_alpha));
}