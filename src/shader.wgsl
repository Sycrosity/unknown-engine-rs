//still quite blackboxy

//the instance configuration matrix (pos and rotation)
struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
};


//vertex shader

//the camera projection matrix
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

//determined by render_pipeline_layout - textures are listed first, so they are group 0, and the camera matrix is listed second, so its group 1
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

//stores the input from wgpu for creating vertices
struct VertexInput {
    //where a vertex is in 3d
    @location(0) position: vec3<f32>,
    //between where and where a texture
    @location(1) tex_coords: vec2<f32>,
    //the normal mapping on top of our vertex
    @location(2) normal: vec3<f32>,
};

//stores the output of our vertex shaders
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

//
struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}

@group(2) @binding(0)
var<uniform> light: Light;

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    //re-assemble our matrix
    let model_matrix: mat4x4<f32> = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;

    out.world_normal = model.normal;
    
    //when multiplying matrices, the vector goes on the right and matrices go on the left in order of importance
    var world_position: vec4<f32> = model_matrix * vec4<f32>(model.position, 1.0);
    out.world_position = world_position.xyz;
    out.clip_position = camera.view_proj * world_position;

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

    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);

    //we don't need (or want) much ambient light, so 0.1 is fine
    let ambient_strength: f32 = 0.1;
    let ambient_color: vec3<f32> = light.color * ambient_strength;

    let light_dir: vec3<f32> = normalize(light.position - in.world_position);
    let diffuse_strength: f32 = max(dot(in.world_normal, light_dir), 0.0);
    let diffuse_color: vec3<f32>  = light.color * diffuse_strength;

    let result: vec3<f32>  = (ambient_color + diffuse_color) * object_color.xyz;

    return vec4<f32>(result, object_color.a);
}
