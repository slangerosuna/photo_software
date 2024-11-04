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

fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let c_max = max(max(rgb.r, rgb.g), rgb.b);
    let c_min = min(min(rgb.r, rgb.g), rgb.b);
    let delta = c_max - c_min;

    var h = 0.0;
    if (delta == 0.0) {
        h = 0.0; // Undefined
    } else if (c_max == rgb.r) {
        h = ((rgb.g - rgb.b) / delta) % 6.0;
    } else if (c_max == rgb.g) {
        h = (rgb.b - rgb.r) / delta + 2.0;
    } else {
        h = (rgb.r - rgb.g) / delta + 4.0;
    }

    let v = c_max;
    var s : f32;
    if c_max == 0.0 {
        s = 0.0;
    } else {
        s = delta / c_max;
    }
    return vec3<f32>(h * 60.0, s, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let c = hsv.z * hsv.y;
    let x = c * (1.0 - abs(((hsv.x / 60.0) % 2.0) - 1.0));
    let m = hsv.z - c;

    var rgb = vec3<f32>(0.0, 0.0, 0.0);

    if (hsv.x < 60.0) {
        rgb = vec3<f32>(c, x, 0.0);
    } else if (hsv.x < 120.0) {
        rgb = vec3<f32>(x, c, 0.0);
    } else if (hsv.x < 180.0) {
        rgb = vec3<f32>(0.0, c, x);
    } else if (hsv.x < 240.0) {
        rgb = vec3<f32>(0.0, x, c);
    } else if (hsv.x < 300.0) {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return rgb + vec3<f32>(m, m, m);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID: vec3<u32>) {
    let pixelCoord: vec2<u32> = GlobalInvocationID.xy;

    var pixel = vec4<f32>(textureLoad(in_image, vec2<i32>(pixelCoord)));
    pixel.a *= opacity;
    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));

    let cur_hsv = rgb_to_hsv(cur.rgb);
    let pixel_hsv = rgb_to_hsv(pixel.rgb);

    let blended_hue = vec3<f32>(pixel_hsv.x, cur_hsv.y, cur_hsv.z);
    let blended_color = hsv_to_rgb(blended_hue);
    let out_alpha = pixel.a + cur.a * (1.0 - pixel.a);
    var sum = vec4<f32>(blended_color, out_alpha);

    let mask_texture_dimensions = textureDimensions(mask);

    let mask_value = textureLoad(mask, vec2<i32>(pixelCoord)).r;
    sum = sum * mask_value + cur * (1.0 - mask_value);

    textureStore(out_image, vec2<i32>(pixelCoord), sum);
}
