@group(0) @binding(0)
var in_image : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(2)
var out_image : texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3)
var mask : texture_storage_2d<r8unorm, read>;
@group(0) @binding(4)
var<uniform> opacity : f32;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID: vec3<u32>) {
    let pixelCoord: vec2<u32> = GlobalInvocationID.xy;

    var pixel = vec4<f32>(textureLoad(in_image, vec2<i32>(pixelCoord)));
    pixel.a *= opacity;
    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));

    var blended_color: vec3<f32>;

    for (var i: u32 = 0; i < 3; i = i + 1) {
        if (pixel[i] < 0.5) {
            blended_color[i] = cur[i] * (pixel[i] * 2.0);
        } else {
            blended_color[i] = 1.0 - (1.0 - cur[i]) * (1.0 - pixel[i]);
        }
    }

    let out_alpha = pixel.a + cur.a * (1.0 - pixel.a);

    var sum = vec4<f32>(blended_color, out_alpha);

    let mask_texture_dimensions = textureDimensions(mask);

    let mask_value = textureLoad(mask, vec2<i32>(pixelCoord)).r;
    sum = sum * mask_value + cur * (1.0 - mask_value);

    textureStore(out_image, vec2<i32>(pixelCoord), sum);
}
