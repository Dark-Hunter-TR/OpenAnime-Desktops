@group(0) @binding(0) var r_current: texture_2d<f32>;
@group(0) @binding(1) var r_previous: texture_2d<f32>;
@group(0) @binding(2) var r_motion_vectors: texture_2d<f32>;
@group(0) @binding(3) var r_sampler: sampler;
@group(0) @binding(4) var r_output: texture_storage_2d<rgba8unorm, write>;

// Motion-Compensated Frame Interpolation.
// Samples the previous frame forward along the motion vector, and the
// current frame backward along the motion vector, then blends them.

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(r_output);
    let coord = vec2<i32>(global_id.xy);

    if (coord.x >= i32(size.x) || coord.y >= i32(size.y)) {
        return;
    }

    let norm_uv = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(size);

    // Read the motion vector at the current location
    let mv = textureLoad(r_motion_vectors, coord, 0).rg;

    // We interpolate at t = 0.5 (halfway between frames)
    let t = 0.5;

    // Displace UV coordinates along the motion vector
    let uv_prev = norm_uv + (mv * t) / vec2<f32>(size);
    let uv_curr = norm_uv - (mv * (1.0 - t)) / vec2<f32>(size);

    // Sample the frames
    let color_prev = textureSampleLevel(r_previous, r_sampler, clamp(uv_prev, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    let color_curr = textureSampleLevel(r_current, r_sampler, clamp(uv_curr, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);

    // Linear blend of both motion-compensated samples
    let interpolated = mix(color_prev, color_curr, t);

    textureStore(r_output, global_id.xy, vec4<f32>(clamp(interpolated.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
