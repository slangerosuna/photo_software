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
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
    let pixelCoord : vec2<u32> = GlobalInvocationID.xy;

    var pixel = vec4<f32>(textureLoad(in_image, vec2<i32>(pixelCoord)));
    pixel.a *= opacity;
    let alpha = pixel.a;

    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));
    let cur_alpha = cur.a;

    let burn_result = vec3<f32>(
        1.0 - (1.0 - cur.r) / max(pixel.r, 0.001),
        1.0 - (1.0 - cur.g) / max(pixel.g, 0.001),
        1.0 - (1.0 - cur.b) / max(pixel.b, 0.001)
    );

    let blended_color = burn_result * alpha + cur.rgb * (1.0 - alpha);
    let out_alpha = alpha + cur_alpha * (1.0 - alpha);

    var sum = vec4<f32>(blended_color, out_alpha);

    let mask_texture_dimensions = textureDimensions(mask);

    if (mask_texture_dimensions.x != 0) && (mask_texture_dimensions.y != 0) {
        let mask_value = textureLoad(mask, vec2<i32>(pixelCoord)).r;
        sum = sum * mask_value + cur * (1.0 - mask_value);
    }

    textureStore(out_image, vec2<i32>(pixelCoord), sum);
}
