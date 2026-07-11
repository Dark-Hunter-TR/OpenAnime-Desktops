@group(0) @binding(0) var r_input: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;
@group(0) @binding(2) var r_output: texture_storage_2d<rgba8unorm, write>;

const PI: f32 = 3.14159265359;

fn sinc(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 0.0001) {
        return 1.0;
    }
    return sin(PI * ax) / (PI * ax);
}

fn lanczos_weight(x: f32) -> f32 {
    let ax = abs(x);
    if (ax < 3.0) {
        return sinc(ax) * sinc(ax / 3.0);
    }
    return 0.0;
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

    let f_coord = floor(in_coord);
    let fraction = in_coord - f_coord;

    var color = vec4<f32>(0.0);
    var total_weight = 0.0;

    // Lanczos-3 needs a 6x6 support window (-2 to +3)
    for (var dy: i32 = -2; dy <= 3; dy++) {
        let wy = lanczos_weight(f32(dy) - fraction.y);
        for (var dx: i32 = -2; dx <= 3; dx++) {
            let wx = lanczos_weight(f32(dx) - fraction.x);
            let weight = wx * wy;

            let sample_coord = vec2<i32>(f_coord) + vec2<i32>(dx, dy);
            let clamped_coord = clamp(sample_coord, vec2<i32>(0), vec2<i32>(input_size) - 1);
            let texel = textureLoad(r_input, clamped_coord, 0);

            color += texel * weight;
            total_weight += weight;
        }
    }

    let final_color = select(color / total_weight, color, total_weight <= 0.0);
    textureStore(r_output, global_id.xy, vec4<f32>(clamp(final_color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
