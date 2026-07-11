@group(0) @binding(0) var r_input: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;
@group(0) @binding(2) var r_output: texture_storage_2d<rgba8unorm, write>;

// Bilateral Denoise Filter
// Blurs flat areas while preserving sharp edges.
const SIGMA_S: f32 = 2.0; // Spatial sigma
const SIGMA_R: f32 = 0.1; // Range/color similarity sigma

fn get_spatial_weight(dx: f32, dy: f32) -> f32 {
    return exp(-(dx*dx + dy*dy) / (2.0 * SIGMA_S * SIGMA_S));
}

fn get_range_weight(c1: vec3<f32>, c2: vec3<f32>) -> f32 {
    let diff = c1 - c2;
    let dist_sq = dot(diff, diff);
    return exp(-dist_sq / (2.0 * SIGMA_R * SIGMA_R));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(r_input);
    let coord = vec2<i32>(global_id.xy);

    if (coord.x >= i32(size.x) || coord.y >= i32(size.y)) {
        return;
    }

    let center_color = textureLoad(r_input, coord, 0);
    var filtered_color = vec3<f32>(0.0);
    var total_weight = 0.0;

    // 5x5 bilateral filter window
    for (var dy: i32 = -2; dy <= 2; dy++) {
        for (var dx: i32 = -2; dx <= 2; dx++) {
            let sample_coord = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(size) - 1);
            let texel = textureLoad(r_input, sample_coord, 0).rgb;

            let w_s = get_spatial_weight(f32(dx), f32(dy));
            let w_r = get_range_weight(center_color.rgb, texel);
            let weight = w_s * w_r;

            filtered_color += texel * weight;
            total_weight += weight;
        }
    }

    let final_color = select(filtered_color / total_weight, center_color.rgb, total_weight <= 0.0);
    textureStore(r_output, global_id.xy, vec4<f32>(clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0)), center_color.a));
}
