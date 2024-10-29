@group(0) @binding(0)
var inImage: texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total: texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID: vec3<u32>) {
    let pixelCoord: vec2<u32> = GlobalInvocationID.xy;

    let pixel = vec4<f32>(textureLoad(inImage, vec2<i32>(pixelCoord)));
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
    textureStore(running_total, vec2<i32>(pixelCoord), vec4<f32>(blended_color, out_alpha));
}
