var<private> DATA: array<vec4<f32>, 6> = array<vec4<f32>, 6>(
    vec4<f32>( -1.0,  1.0,  0.0, 1.0 ),
    vec4<f32>( -1.0, -1.0,  0.0, 0.0 ),
    vec4<f32>(  1.0, -1.0,  1.0, 0.0 ),
    vec4<f32>( -1.0,  1.0,  0.0, 1.0 ),
    vec4<f32>(  1.0, -1.0,  1.0, 0.0 ),
    vec4<f32>(  1.0,  1.0,  1.0, 1.0 )
);

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vertex(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(DATA[in_vertex_index].xy, 0.0, 1.0);
    out.tex_coords = DATA[in_vertex_index].zw;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
