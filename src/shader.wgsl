//blackboxy
//[TODO] understand and comment

//vertex shader

//stores the input from wgpu for creating vertex's
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

//stores the output of our vertex shaders
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    // @builtin(vertex_index) in_vertex_index: u32,
    model: VertexInput
) -> VertexOutput {
    var out: VertexOutput;
    // let x = f32(1 - i32(in_vertex_index)) * 0.5;
    // let y = f32(i32(in_vertex_index & 1u) * 2 - 1) * 0.5;
    out.color = model.color;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}


//fragment shader

//@location(0) refers to the first colour target
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}

