@group(0) @binding(0)
var inImage : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total : texture_storage_2d<rgba8unorm, read_write>;
@group(0) @binding(2)
var<uniform> opacity : f32;

fn rand(p : vec2<u32>) -> f32 {
    let p2 = 2246822519u; let p3 = 3266489917u;
    let p4 = 668265263u; let p5 = 374761393u;
    var h32 = p.y + p5 + p.x * p3;
    h32 = p4 * ((h32 << 17) | (h32 >> (32 - 17)));
    h32 = p2 * (h32^(h32 >> 15));
    h32 = p3 * (h32^(h32 >> 13));
    return fract(sin(f32(h32^(h32 >> 16))));
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
    let pixelCoord : vec2<u32> = GlobalInvocationID.xy;

    var pixel = vec4<f32>(textureLoad(inImage, vec2<i32>(pixelCoord)));
    pixel.a *= opacity;
    let alpha = pixel.a;

    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));

    let random_value = rand(pixelCoord);

    var blended_color: vec4<f32>;
    if (random_value < alpha) {
        blended_color = pixel;
    } else {
        blended_color = cur;
    };

    textureStore(running_total, vec2<i32>(pixelCoord), blended_color);
}
