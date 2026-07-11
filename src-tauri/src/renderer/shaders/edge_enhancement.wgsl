@group(0) @binding(0) var r_input: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;
@group(0) @binding(2) var r_output: texture_storage_2d<rgba8unorm, write>;

fn get_luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(r_input);
    let coord = vec2<i32>(global_id.xy);

    if (coord.x >= i32(size.x) || coord.y >= i32(size.y)) {
        return;
    }

    // Sobel operator to find edges
    var luma: array<f32, 9>;
    var idx = 0;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            let sample_coord = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(size) - 1);
            let texel = textureLoad(r_input, sample_coord, 0).rgb;
            luma[idx] = get_luma(texel);
            idx++;
        }
    }

    let gx = -luma[0] + luma[2] - 2.0 * luma[3] + 2.0 * luma[5] - luma[6] + luma[8];
    let gy = -luma[0] - 2.0 * luma[1] - luma[2] + luma[6] + 2.0 * luma[7] + luma[8];
    let edge = sqrt(gx * gx + gy * gy);

    // Center pixel
    let center_color = textureLoad(r_input, coord, 0);

    // High pass detail enhancement along edges
    let kernel_center = 5.0;
    let kernel_edge = -1.0;
    
    // Simple laplacian detail extraction
    let laplacian = (
        - luma[1] 
        - luma[3] + 4.0 * luma[4] - luma[5] 
        - luma[7]
    );

    // If edge is detected, apply a mild boost
    let enhancement_factor = 0.25;
    let edge_boost = clamp(edge * enhancement_factor, 0.0, 0.5);
    
    let enhanced_rgb = center_color.rgb + vec3<f32>(laplacian * edge_boost);
    textureStore(r_output, global_id.xy, vec4<f32>(clamp(enhanced_rgb, vec3<f32>(0.0), vec3<f32>(1.0)), center_color.a));
}
