@group(0) @binding(0)
var inImage : texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1)
var running_total : texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationID : vec3<u32>) {
	let pixelCoord : vec2<u32> = GlobalInvocationID.xy;

    let pixel = vec4<f32>(textureLoad(inImage, vec2<i32>(pixelCoord)));
    let alpha = pixel.a;

    let cur = vec4<f32>(textureLoad(running_total, vec2<i32>(pixelCoord)));
    let cur_alpha = cur.a;
    let alpha_inv = 1.0 - alpha;

    let sum = (pixel * alpha + cur * alpha_inv * cur_alpha);

	textureStore(running_total, vec2<i32>(pixelCoord), vec4<f32>(sum));
}
