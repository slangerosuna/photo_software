@group(0) @binding(0)
var mask_texture : texture_storage_2d<r8unorm, read_write>;
@group(1) @binding(0)
var<uniform> opacity : f32;
@group(1) @binding(1)
var<uniform> brush_size : f32;
@group(1) @binding(2)
var<uniform> brush_hardness : f32;
@group(1) @binding(3)
var<uniform> brush_rotation : f32; // in radians
@group(1) @binding(4)
var brush_texture : texture_storage_2d<r8unorm, read>;
@group(1) @binding(5)
var<uniform> use_texture : u32; 
@group(2) @binding(0)
var<uniform> brush_center : vec2<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) GlobalInvocationId: vec3<u32>) {
    let execution_corner = vec2<u32>(brush_center - vec2<f32>(brush_size, brush_size));
    let pixel = vec2<u32>(GlobalInvocationId.xy) + execution_corner;

    if (use_texture == 0) {
        // No brush texture, just draw with hardness and size
        var distance : f32;
        if (brush_size < 1.0) {
            // round to nearest pixel so that it works to draw a single pixel
            distance = 0.0;
        } else {
            distance = distance(brush_center, vec2<f32>(pixel));
        }
        let alpha = opacity * apply_hardness_circle_brush(1.0 - distance / brush_size, brush_hardness);
        let cur = textureLoad(mask_texture, vec2<i32>(pixel));

        if (alpha > cur.r) {
            textureStore(mask_texture, vec2<i32>(pixel), vec4<f32>(alpha));
        }
    } else {
        let dimensions = textureDimensions(brush_texture).xy;
        let brush_tex_size_vec = textureDimensions(brush_texture).xy;
        let brush_tex_center = vec2<f32>(brush_tex_size_vec) / 2.0;
        let brush_tex_size_scalar : u32 = max(brush_tex_size_vec.x, brush_tex_size_vec.y);
        let brush_pixels_per_mask_pixel = f32(brush_tex_size_scalar) / brush_size;

        let brush_tex_coord = (vec2<f32>(pixel) - brush_center) * brush_pixels_per_mask_pixel;
        let brush_tex_coord_rotated = vec2<f32>(
            brush_tex_coord.x * cos(brush_rotation) - brush_tex_coord.y * sin(brush_rotation),
            brush_tex_coord.x * sin(brush_rotation) + brush_tex_coord.y * cos(brush_rotation)
        ) + brush_tex_center;

        // TODO: make this bicubic
        let sampled_opacity = textureLoad(brush_texture, vec2<i32>(brush_tex_coord_rotated));

        let hardened_opacity =
            apply_hardness_texture_brush(sampled_opacity.r, brush_hardness) * opacity;
    }
}

fn apply_hardness_circle_brush(input: f32, hardness: f32) -> f32 {
    return smoothstep(0.0, 1.0 - hardness, input);
}

fn apply_hardness_texture_brush(input: f32, hardness: f32) -> f32 {
    let half_hardness = hardness * 0.5;
    return smoothstep(half_hardness, 1.0 - half_hardness, input);
}