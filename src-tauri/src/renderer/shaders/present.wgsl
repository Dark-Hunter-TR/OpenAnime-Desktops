struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let x = select(-1.0, 1.0, in_vertex_index == 1u || in_vertex_index == 2u || in_vertex_index == 4u);
    let y = select(-1.0, 1.0, in_vertex_index == 2u || in_vertex_index == 3u || in_vertex_index == 4u);
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coords = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

@group(0) @binding(0) var r_texture: texture_2d<f32>;
@group(0) @binding(1) var r_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(r_texture, r_sampler, in.tex_coords);
}
