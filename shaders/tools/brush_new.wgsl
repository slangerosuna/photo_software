@group(0) @binding(0)
var mask_texture : texture_storage_2d<r8unorm, read_write>;
@group(0) @binding(1)
var<uniform> opacity : f32;
@group(0) @binding(2)
var<uniform> brush_size : f32;
@group(0) @binding(3)
var<uniform> brush_hardness : f32;
@group(0) @binding(4)
var<uniform> brush_rotation : f32; // in radians
@group(1) @binding(0)
var<storage> path : array<vec2<f32>>;
@group(1) @binding(1)
var<uniform> path_length : u32;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationId: vec3<u32>) {
    let pixel = vec2<u32>(GlobalInvocationId.xy);

    for (var i = 0u; i < path_length + 1u; i = i + 1u) {
        let start = path[i];
        let end = path[(i + 1u) % path_length];
        let start_to_pixel = vec2<f32>(pixel) - start;
        let start_to_end = end - start;
        let start_to_end_length = length(start_to_end);
        let start_to_end_normalized = start_to_end / start_to_end_length;
        let start_to_end_normal = vec2<f32>(-start_to_end_normalized.y, start_to_end_normalized.x);
        let start_to_end_normal_scaled = start_to_end_normal * brush_size;
        let start_to_pixel_length = dot(start_to_pixel, start_to_end_normal_scaled) / start_to_end_length;
        let start_to_pixel_normal = start_to_pixel - start_to_pixel_length * start_to_end_normalized;
        let distance = length(start_to_pixel_normal);
        let alpha = opacity * apply_hardness_circle_brush(1.0 - distance / brush_size, brush_hardness);
        let cur = textureLoad(mask_texture, vec2<i32>(pixel));

        if (alpha > cur.r) {
            textureStore(mask_texture, vec2<i32>(pixel), vec4<f32>(alpha));
        }
    }
}

fn apply_hardness_circle_brush(input: f32, hardness: f32) -> f32 {
    return smoothstep(0.0, 1.0 - hardness, input);
}