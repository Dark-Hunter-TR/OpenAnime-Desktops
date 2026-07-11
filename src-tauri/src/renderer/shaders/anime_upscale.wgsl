@group(0) @binding(0) var r_input: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;
@group(0) @binding(2) var r_output: texture_storage_2d<rgba8unorm, write>;

// Anime line-art guided upscale shader.
// Interpolates along the edge direction to avoid jaggy outlines on line art.

fn get_luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let output_size = textureDimensions(r_output);
    let input_size = textureDimensions(r_input);

    if (global_id.x >= output_size.x || global_id.y >= output_size.y) {
        return;
    }

    let out_uv = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(output_size);
    let in_coord = out_uv * vec2<f32>(input_size) - 0.5;
    let f_coord = vec2<i32>(floor(in_coord));
    let frac = in_coord - floor(in_coord);

    // Sample 2x2 neighborhood
    let c00 = textureLoad(r_input, clamp(f_coord + vec2<i32>(0, 0), vec2<i32>(0), vec2<i32>(input_size) - 1), 0).rgb;
    let c10 = textureLoad(r_input, clamp(f_coord + vec2<i32>(1, 0), vec2<i32>(0), vec2<i32>(input_size) - 1), 0).rgb;
    let c01 = textureLoad(r_input, clamp(f_coord + vec2<i32>(0, 1), vec2<i32>(0), vec2<i32>(input_size) - 1), 0).rgb;
    let c11 = textureLoad(r_input, clamp(f_coord + vec2<i32>(1, 1), vec2<i32>(0), vec2<i32>(input_size) - 1), 0).rgb;

    let l00 = get_luma(c00);
    let l10 = get_luma(c10);
    let l01 = get_luma(c01);
    let l11 = get_luma(c11);

    // Compute edge directions (gradients)
    let g_x = (l10 - l00) * (1.0 - frac.y) + (l11 - l01) * frac.y;
    let g_y = (l01 - l00) * (1.0 - frac.x) + (l11 - l10) * frac.x;

    var color = vec3<f32>(0.0);
    
    // If gradient is strong (edge detected), interpolate along the direction perpendicular to the gradient
    let grad_len = sqrt(g_x * g_x + g_y * g_y);
    if (grad_len > 0.05) {
        let dir = vec2<f32>(-g_y, g_x) / grad_len; // perpendicular direction
        
        // Sample slightly offset along the line direction
        let offset = dir * 0.5;
        let uv_offset_a = (in_coord + offset + 0.5) / vec2<f32>(input_size);
        let uv_offset_b = (in_coord - offset + 0.5) / vec2<f32>(input_size);

        let sample_a = textureSampleLevel(r_input, r_sampler, uv_offset_a, 0.0).rgb;
        let sample_b = textureSampleLevel(r_input, r_sampler, uv_offset_b, 0.0).rgb;

        color = (sample_a + sample_b) * 0.5;
    } else {
        // Standard bilinear interpolation for flat/textured regions
        let color_top = mix(c00, c10, frac.x);
        let color_bot = mix(c01, c11, frac.x);
        color = mix(color_top, color_bot, frac.y);
    }

    textureStore(r_output, global_id.xy, vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
