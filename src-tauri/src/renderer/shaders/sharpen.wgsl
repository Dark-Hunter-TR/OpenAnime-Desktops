@group(0) @binding(0) var r_input: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;
@group(0) @binding(2) var r_output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let size = textureDimensions(r_input);
    let coord = vec2<i32>(global_id.xy);

    if (coord.x >= i32(size.x) || coord.y >= i32(size.y)) {
        return;
    }

    let center = textureLoad(r_input, coord, 0);
    
    let left = textureLoad(r_input, clamp(coord + vec2<i32>(-1, 0), vec2<i32>(0), vec2<i32>(size) - 1), 0);
    let right = textureLoad(r_input, clamp(coord + vec2<i32>(1, 0), vec2<i32>(0), vec2<i32>(size) - 1), 0);
    let up = textureLoad(r_input, clamp(coord + vec2<i32>(0, -1), vec2<i32>(0), vec2<i32>(size) - 1), 0);
    let down = textureLoad(r_input, clamp(coord + vec2<i32>(0, 1), vec2<i32>(0), vec2<i32>(size) - 1), 0);

    // Laplacian filter: 4 * center - left - right - up - down
    let laplacian = 4.0 * center.rgb - (left.rgb + right.rgb + up.rgb + down.rgb);

    // Sharpening strength (e.g., 0.35)
    let strength = 0.35;
    let sharpened = center.rgb + laplacian * strength;

    textureStore(r_output, global_id.xy, vec4<f32>(clamp(sharpened, vec3<f32>(0.0), vec3<f32>(1.0)), center.a));
}
