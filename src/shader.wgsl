//blackboxy
//[TODO] understand and comment

//vertex shader

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

//determined by render_pipeline_layout - textures are listed first, so they are group 0, and the camera matrix is listed second, so its group 1
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

//stores the input from wgpu for creating vertices
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
};

//stores the output of our vertex shaders
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(
    // @builtin(vertex_index) in_vertex_index: u32,
    model: VertexInput
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    //when multiplying matrices, the vector goes on the right and matrices go on the left in order of importance
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    return out;
}


//fragment shader

//group() corresponds to first parameter in set_bind_group(), binding() to the binding specified in BindGroupLayout and BindGroup
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

//@location(0) refers to the first colour target
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
