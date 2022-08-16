//blackboxy
//[TODO] understand and comment

//vertex shader

//stores the output of our vertex shaders
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(1 - i32(in_vertex_index)) * 0.5;
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1) * 0.5;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}


//fragment shader

//@location(0) refers to the first colour target
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    //sets colour of current fragment to brown
    return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}

