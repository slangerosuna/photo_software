@group(0) @binding(0)
var inImage : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total : texture_storage_2d<rgba8unorm, read_write>;

fn rand(coord: vec2<u32>) -> f32 {
    let seed = f32(coord.x * 12345u + coord.y * 67890u);
    return fract(sin(seed) * 43758.5453123);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
    let pixelCoord : vec2<u32> = GlobalInvocationID.xy;

    let pixel = vec4<f32>(textureLoad(inImage, vec2<i32>(pixelCoord)));
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
