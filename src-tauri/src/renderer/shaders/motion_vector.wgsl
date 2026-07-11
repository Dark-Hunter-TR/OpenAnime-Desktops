@group(0) @binding(0) var r_current: texture_2d<f32>;
@group(0) @binding(1) var r_previous: texture_2d<f32>;
@group(0) @binding(2) var r_motion_vectors: texture_storage_2d<rg16float, write>;

// Block-Matching Motion Estimation.
// Searches a small window around each pixel to find the displacement vector
// that minimizes the Sum of Absolute Differences (SAD) with the previous frame.

const BLOCK_SIZE: i32 = 8;
const SEARCH_RANGE: i32 = 4;

fn get_luma(color: vec4<f32>) -> f32 {
    return dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(r_current);
    let coord = vec2<i32>(global_id.xy);

    if (coord.x >= i32(size.x) || coord.y >= i32(size.y)) {
        return;
    }

    var best_vector = vec2<f32>(0.0, 0.0);
    var min_sad = 1e9;

    // Load center block comparison values in current frame
    var current_luma: array<f32, 25>; // 5x5 sub-block for local matching
    var c_idx = 0;
    for (var dy: i32 = -2; dy <= 2; dy++) {
        for (var dx: i32 = -2; dx <= 2; dx++) {
            let sample_coord = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(size) - 1);
            current_luma[c_idx] = get_luma(textureLoad(r_current, sample_coord, 0));
            c_idx++;
        }
    }

    // Search range in the previous frame
    for (var sy: i32 = -SEARCH_RANGE; sy <= SEARCH_RANGE; sy++) {
        for (var sx: i32 = -SEARCH_RANGE; sx <= SEARCH_RANGE; sx++) {
            var sad = 0.0;
            var p_idx = 0;

            for (var dy: i32 = -2; dy <= 2; dy++) {
                for (var dx: i32 = -2; dx <= 2; dx++) {
                    let sample_coord = clamp(coord + vec2<i32>(dx + sx, dy + sy), vec2<i32>(0), vec2<i32>(size) - 1);
                    let prev_luma = get_luma(textureLoad(r_previous, sample_coord, 0));
                    sad += abs(current_luma[p_idx] - prev_luma);
                    p_idx++;
                }
            }

            if (sad < min_sad) {
                min_sad = sad;
                best_vector = vec2<f32>(f32(sx), f32(sy));
            }
        }
    }

    textureStore(r_motion_vectors, global_id.xy, vec4<f32>(best_vector, 0.0, 1.0));
}
