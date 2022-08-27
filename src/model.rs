//for loading models into the engine

use std::ops::Range;

use crate::texture;

//only a trait as there can be many types of vertices, and this would still work
pub trait Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

#[repr(C)]
//needs Pod and Zeroable to be able to cast it to a &[u8]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
//stores relevant data for a single vertex in  model
pub struct ModelVertex {
    pub position: [f32; 3],
    //texture coordinates - 2 f32's as textures are 2d only (for now)
    pub tex_coords: [f32; 2],
    //for lighting (will be used later)
    pub normal: [f32; 3],
}

impl Vertex for ModelVertex {
    //very like the original VertexBufferLayout, but there is a layout for the "normal", used for lighting
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            //defines the width of our ModelVertex
            array_stride: std::mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            //how often to move to the next vertex - can be wgpu::VertexStepMode::Instance if we want to only change vertices when we start drawing an instance
            step_mode: wgpu::VertexStepMode::Vertex,
            //describe the individual parts of a vertex - generally the same structure as the shader (could use the vertex_attr_array![] macro but it requires some jankyness so will keep with this for now
            //[TODO] replace with vertex_attr_array![] at end of tutorial
            attributes: &[
                //position
                wgpu::VertexAttribute {
                    //the offset before the attribute starts - 0 for now, as we should have no data before our vertexes
                    offset: 0,
                    //tells the shader where to store this attribute at - shader_location: 0 is for the position and 1 is for the colour (at least currently)
                    shader_location: 0,
                    //the shape of the the attribute (Float32x3 is vec3<f32> in shader code, Float32x4 is vec4<f32> and is the max value we can store)
                    format: wgpu::VertexFormat::Float32x3,
                },
                //colour
                wgpu::VertexAttribute {
                    //the sum of the size_of the previous attributes' data
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    //the colour attribute of the shader
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                //normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    //the lighting/normal attribute of the shader
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

//the object's vertices and it's texture/s
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

//just the texture and its name (for debug)
pub struct Material {
    pub label: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

//all the vertices and indices data of the model
pub struct Mesh {
    pub label: String,
    //buffers are used to store all the data we want to draw (so we don't have to expensively recomplie the shader on every update)
    //to store all the individual vertices in our elements
    pub vertex_buffer: wgpu::Buffer,
    //to store the order of indices to render our vertices correctly
    pub index_buffer: wgpu::Buffer,
    //how many elements there are
    pub num_elements: u32,
    //the list index of the material texture for our elements
    pub material: usize,
}

//components needed to render our models to the screen
pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: std::ops::Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        //tells wgpu what slice of the vertex buffer to use - here it's .. which means all of it
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        //tells wgpu where our index buffer is and what parts of it to use
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        //tells wgu how to access textures
        self.set_bind_group(0, &material.bind_group, &[]);
        //tells wgu how to use apply the camera matrix
        self.set_bind_group(1, camera_bind_group, &[]);
        //tells wgpu how to use our light
        self.set_bind_group(2, light_bind_group, &[]);
        //tells wgpu to draw something using our indices and vertices
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }
    fn draw_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material: &Material = &model.materials[mesh.material];
            self.draw_mesh_instanced(
                mesh,
                material,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}
pub trait DrawLight<'a> {
    fn draw_light_mesh(
        &mut self,
        mesh: &'a Mesh,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_light_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }
    fn draw_light_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(
                mesh,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}
