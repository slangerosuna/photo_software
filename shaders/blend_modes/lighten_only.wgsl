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
    let alpha_inv = 1.0 - alpha;

    var sum = (pixel * alpha + cur * alpha_inv * cur_alpha);

    let cur_high = max(cur.r, max(cur.g, cur.b));
    let sum_high = max(sum.r, max(sum.g, sum.b));
    if (cur_high > sum_high) { sum = cur; }

	textureStore(running_total, vec2<i32>(pixelCoord), vec4<f32>(sum));
}
