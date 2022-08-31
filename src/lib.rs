//for now, before everything is implimented, we will allow unused/dead code to exist without warnings
#![allow(dead_code)]

mod camera;
mod model;
mod resources;
mod texture;

use wgpu::util::DeviceExt;

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
//wasm specific dependencies
use cgmath::prelude::*;

use model::Vertex;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

//wgsl doesn't have a representation for quarterons, so we convert the instance into just a matrix
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl InstanceRaw {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            //we need to switch from using a step mode of Vertex to Instance - this means that our shaders will only change to use the next instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            //[TODO] replace with the wgpu::vertex_attr_array![] macro
            attributes: &[
                //a wgsl mat4 takes up 4 vertex slots as it is technically 4 vec4s - shince we need to define a slot for each vec4, we'll have to reassemble the mat4 in the shader
                wgpu::VertexAttribute {
                    offset: 0,
                    //while our vertex shader only uses locations 0, and 1 now, in later tutorials we'll be using 2, 3, and 4, for vertex - we'll start at slot 5 to not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

//allows us to draw the same object multiple times with different properties
struct Instance {
    position: cgmath::Vector3<f32>,
    //really very complicated black box, but is a mathematical structure often used to represent rotation
    //[TODO] read https://mathworld.wolfram.com/Quaternion.html to try and vaguely understand what this is doing
    rotation: cgmath::Quaternion<f32>,
}

impl Instance {
    //convert to a wgsl interpretable InstanceRaw
    fn to_raw(&self) -> InstanceRaw {
        let model: cgmath::Matrix4<f32> =
            cgmath::Matrix4::from_translation(self.position) * cgmath::Matrix4::from(self.rotation);
        InstanceRaw {
            model: model.into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
        }
    }
}

//we need this for Rust to store our data correctly for the shaders
#[repr(C)]
//this is so we can store this in a buffer (aka have it turned into a &[u8])
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
//the camera matrix data for use in the buffer
struct CameraUniform {
    //needed to calculate specular lighting
    view_position: [f32; 4],
    //we can't use cgmath with bytemuck directly so we'll have to convert the Matrix4 into a 4x4 f32 array
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    //convert a Camera into a CameraUniform so it can be used in a uniform buffer
    fn update_view_proj(&mut self, camera: &camera::Camera, projection: &camera::Projection) {
        self.view_position = camera.position.to_homogeneous().into();
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    //due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
    //we need to use a padding field here too
    _padding2: u32,
}

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    //creates a shader from our shader file (in this case, shader.wgsl)
    let shader: wgpu::ShaderModule = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            //specifies which shader function should be our entrypoint
            entry_point: "vs_main",
            //the types of vertices we want to pass to the vertex shader
            buffers: vertex_layouts,
        },
        //technically optional, so has to be wrapped in a Some enum
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            //for now, only need one for surface
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                //for now, blending should just replace old pixel data with new pixel data
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                //for now, we write to all colours (rgba)
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        //how to interpret converting vertices to triangles
        primitive: wgpu::PrimitiveState {
            //every 3 vertices corrisponds to one triange - no overlapping triangles or lines ect
            topology: wgpu::PrimitiveTopology::TriangleList,
            //doesn't apply
            strip_index_format: None,
            //front_face + cull_face - tells wgpu how to decide whether a triangle is facing forwards or not
            //dictates a right-handed coordinates system (which we will use for now)
            front_face: wgpu::FrontFace::Ccw,
            //the back of a trianges face will not be included in the render
            cull_mode: Some(wgpu::Face::Back),
            //setting this to anything other than fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            //requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            //requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        //how depth is rendered (so elements are properly on top of one another)
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            //pixels will be drawn from front to back
            depth_compare: wgpu::CompareFunction::Less,
            //will be used later, so for now is just default
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        //[TODO] learn what multisampling is and add comments for it
        multisample: wgpu::MultisampleState {
            //determines how many samples should be active
            count: 1,
            //specifies which samples should be active - in this case all of them ( represented by !0 )
            mask: !0,
            //for anti-aliasing - doesn't apply for now
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}

//the state of the everything related to the program - the window, device, buffers, textures, models, ect
struct State {
    //the part of the window that we actually draw to
    surface: wgpu::Surface,
    //connection to the graphics/compute device
    device: wgpu::Device,
    //the command queue for the device
    queue: wgpu::Queue,
    //defines how our surface will create the underlying SurfaceTextures
    config: wgpu::SurfaceConfiguration,
    //size of our window
    size: winit::dpi::PhysicalSize<u32>,
    //describes the actions our gpu will perform when acting on a set of data (like a set of verticies)
    render_pipeline: wgpu::RenderPipeline,
    //our imported model
    obj_model: model::Model,
    //a view into our scene that can move and look around
    camera: camera::Camera,
    //a set of settings relating to how the camera looks and percieves the scene
    projection: camera::Projection,
    //how the camera is controlled
    camera_controller: camera::CameraController,
    //whether the mouse is pressed or not (both scroll wheel and buttons)
    mouse_pressed: bool,
    //the camera matrix data for use in the buffer
    camera_uniform: CameraUniform,
    //to store the matrix data associated with the camera
    camera_buffer: wgpu::Buffer,
    //describes how the camera can be accessed by the shader
    camera_bind_group: wgpu::BindGroup,

    //the list of our instances
    instances: Vec<Instance>,
    //to store the model and matrix data associated with our instances
    instance_buffer: wgpu::Buffer,
    //how depth is percieved by the renderer
    depth_texture: texture::Texture,
    //the position and colour of light data
    light_uniform: LightUniform,
    //to store the
    light_buffer: wgpu::Buffer,
    //describes how our light should be accessed by the shader
    light_bind_group: wgpu::BindGroup,
    //describes the actions our gpu will perform to render our light into our scene
    light_render_pipeline: wgpu::RenderPipeline,
}

impl State {
    // creating some of the wgpu types requires async code
    async fn new(window: &Window) -> Self {
        //find the safe size of the current window
        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();

        //instance is a handle to a GPU
        //Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance: wgpu::Instance = wgpu::Instance::new(wgpu::Backends::all());

        //the part of the window that we actually draw to
        //has to be unsafe as it interfaces with the gpu (which is not neccesarily safe)
        let surface: wgpu::Surface = unsafe { instance.create_surface(window) };

        //the handler to our actual gpu/other graphics medium
        let adapter: wgpu::Adapter = instance
            //should work for most devices,
            .request_adapter(&wgpu::RequestAdapterOptions {
                //can be LowPower or HighPower - LowPower will try and use an adapter that favours battery life, HighPower will target a more power consuming but higher performance gpu
                //[TODO] allow the user to choose a performance mode
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                //will force wgpu to use an adapter that works on all hardware, rendering with software on the cpu instead of using dedicated graphics processing renderers
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // device: opens a connection to the graphics/compute device
        // queue: handles the command queue for the device
        let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    //here we can choose extra features we want from wgpu (currently none) - not all gpus can support these extra features, so we would have to limit the allowed gpus
                    features: wgpu::Features::empty(),
                    //WebGL doesn't support all of wgpu's features, so if we're building for the web we'll have to disable some of them
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, //trace path
            )
            .await
            .unwrap();

        //defines how our surface will create the underlying SurfaceTextures
        let config: wgpu::SurfaceConfiguration = wgpu::SurfaceConfiguration {
            //specifies that the textures will be used to draw on the screen
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            //defines how the SurfaceTextures will be stored on our gpu - we will choose the best format based on what display is being used
            format: surface.get_supported_formats(&adapter)[0],
            //typically width and height are the size of the window
            //[WARNING] if either width or height is 0, the program will crash
            //[TODO] allow the user to choose a screen resolution
            width: size.width,
            height: size.height,
            //essentially Vsync, and will cap the display rate to the display's frame rate - there are other options to choose from https://docs.rs/wgpu/latest/wgpu/enum.PresentMode.html
            //[TODO] allow the user to choose what mode they want (probably between AutoNoVsync and AutoVsync)
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        surface.configure(&device, &config);

        //[TODO] really very black box
        //used to create a bind group with the specified config, so that bind groups can be swapped in and out (as long as they share the same BindGroupLayout)
        let texture_bind_group_layout: wgpu::BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                //our bind group needs two entries - a sampled texure, and a sampler
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        //texure binding
                        binding: 0,
                        //visible to only the fragment shader
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        //sampler binding
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        //this should match the filterable field of the corresponding Texture entry above
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        //we only have one so this isn't needed
                        count: None,
                    },
                    // normal map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        //how depth is percieved by the renderer
        let depth_texture: texture::Texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let camera: camera::Camera = camera::Camera::new(
            // position the camera one unit up and 2 units back - the +z coordinate is out of the screen (coord ranges are 1.0 to -1.0)
            (0.0, 5.0, 10.0),
            cgmath::Deg(-90.0),
            cgmath::Deg(-20.0),
        );

        let projection: camera::Projection = camera::Projection::new(
            config.width,
            config.height,
            //a basic, random value
            //[TODO] allow user to change in settings
            cgmath::Deg(45.0),
            0.1,
            100.0,
        );

        //how the camera is controlled
        let camera_controller: camera::CameraController = camera::CameraController::new(4.0, 0.4);

        //convert our camera matrix into a CameraUniform
        let mut camera_uniform: CameraUniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        //the uniform buffer for our camera - a &[u8] representation of the camera matrix
        let camera_buffer: wgpu::Buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                //COPY_DST allows us to copy data to the buffer
                //UNIFORM allows our buffer to be inside a bind_group
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        //config for our camera bind group
        let camera_bind_group_layout: wgpu::BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    //the camera  needs to be visible to the both shaders
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        //the size of data won't change, so it doesn't need to be dynamic
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    //None means its not an array (aka just one binding)
                    count: None,
                }],
            });

        //describes how the camera can be accessed by the shader
        let camera_bind_group: wgpu::BindGroup =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("camera_bind_group"),
                layout: &camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            });

        //how far away each model should be from one another
        const SPACE_BETWEEN: f32 = 3.0;
        //how many instances of our model are we going to display
        const NUM_INSTANCES_PER_ROW: u32 = 10;
        //define our instances (should be 100 pentagons in a 10x10 grid, each rotated based on an axis)
        let instances: Vec<Instance> = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let position: cgmath::Vector3<f32> = cgmath::Vector3 {
                        x: SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0),
                        y: 0.0,
                        z: SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0),
                    };

                    let rotation: cgmath::Quaternion<f32> = if position.is_zero() {
                        //this is needed so an object at (0, 0, 0) won't get scaled to zero as Quaternions can effect scale if they're not created correctly
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        //the instances created turned into InstanceRaw's so they can be interpreted by the shader
        let instance_data: Vec<InstanceRaw> =
            instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

        //to store the model and matrix data associated with our instances
        let instance_buffer: wgpu::Buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let light_uniform: LightUniform = LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };

        let light_buffer: wgpu::Buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light VB"),
                contents: bytemuck::cast_slice(&[light_uniform]),
                // we'll want to update our lights position, so we use COPY_DST
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let light_bind_group_layout: wgpu::BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let light_bind_group: wgpu::BindGroup =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &light_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_buffer.as_entire_binding(),
                }],
            });

        let light_render_pipeline: wgpu::RenderPipeline = {
            let layout: wgpu::PipelineLayout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Light Pipeline Layout"),
                    bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
                    push_constant_ranges: &[],
                });
            //creates a shader from our shader file
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                //the include_wgsl!() macro makes it so we don't have to write really dumb boilerplate code to create the shader
                source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[model::ModelVertex::desc()],
                shader,
            )
        };

        //setup for our rendering pipeline
        let render_pipeline_layout: wgpu::PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                //the list of bind groups being used
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        //describes the actions our gpu will perform when acting on a set of data
        let render_pipeline: wgpu::RenderPipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            };
            create_render_pipeline(
                &device,
                &render_pipeline_layout,
                config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[model::ModelVertex::desc(), InstanceRaw::desc()],
                shader,
            )
        };

        //load our model from its .obj file
        let obj_model: model::Model =
            resources::load_obj_model("cube.obj", &device, &queue, &texture_bind_group_layout)
                .await
                .unwrap();

        //return all of our created data in a State struct
        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            obj_model,
            depth_texture,
            camera,
            projection,
            mouse_pressed: false,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            instances,
            instance_buffer,
            light_uniform,
            light_buffer,
            light_bind_group,
            light_render_pipeline,
        }
    }

    //resizing the window requires reconfiguring the surface
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
        self.depth_texture =
            texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        self.projection.resize(new_size.width, new_size.height);
    }

    //an inputs should return true if something changed, and false if nothing changed
    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => self.camera_controller.process_keyboard(*key, *state),
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            _ => false,
        }
    }

    fn update(&mut self, dt: instant::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        //write to the buffer with our updated data
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        //update light positon
        let old_position: cgmath::Vector3<_> = self.light_uniform.position.into();
        self.light_uniform.position = (cgmath::Quaternion::from_axis_angle(
            (0.0, 1.0, 0.0).into(),
            cgmath::Deg(60.0 * dt.as_secs_f32()),
        ) * old_position)
            .into();

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        //wait for the surface to produce a new texture that we will render to
        let output: wgpu::SurfaceTexture = self.surface.get_current_texture()?;

        //creates a TextureView with default settings to control how the render code interacts with the textures
        let view: wgpu::TextureView = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        //creates a command buffer (which most modern gpu's expect to recieve) that we can then send to the gpu
        let mut encoder: wgpu::CommandEncoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        //this block is needed to tell rust to drop all references and variables within it so we can finish() it (as encoder is  borrowed mutably)
        {
            //contains all the methods to actually draw to the window
            let mut render_pass: wgpu::RenderPass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    //can be anything
                    label: Some("Render Pass"),
                    //black box config for setting up colours properly
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        //tells wgpu what texture to save the colours to
                        view: &view,
                        //only used if multi-sampling is enabled (its not)
                        resolve_target: None,
                        //tells wgpu what to do with the colours on the screen
                        ops: wgpu::Operations {
                            //tells wgpu how to handle colours stored from the previous frame (currently just clearing the screen with a blueish colour) - this is compairable to a default background?
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            }),
                            //whether we should store our rendered results to the Texture from the TextureView
                            store: true,
                        },
                    })],
                    //actually uses the depth texture
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        //only using depth, no stensil yet
                        stencil_ops: None,
                    }),
                });

            //tells wgpu what instances we have and how to draw them
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            {
                use crate::model::DrawLight;
                render_pass.set_pipeline(&self.light_render_pipeline);
                render_pass.draw_light_model(
                    &self.obj_model,
                    &self.camera_bind_group,
                    &self.light_bind_group,
                );
            }

            render_pass.set_pipeline(&self.render_pipeline);

            {
                use model::DrawModel;
                render_pass.draw_model_instanced(
                    &self.obj_model,
                    0..self.instances.len() as u32,
                    &self.camera_bind_group,
                    &self.light_bind_group,
                );
            }
        }

        //tells wgpu to finish the command buffer and submit it to the render queue
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        //if all of this completes, return an Ok enum
        Ok(())
    }
}

//tells wasm to run the run() function when wasm is initialised
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
//run the rasterizer
//needs to be async as State::new() is now async aswell
pub async fn run() {
    //checks if there is platform specific code being ran
    cfg_if::cfg_if! {
        //if its on wasm, use the web logger instead of normal env_logger
        if #[cfg(target_arch = "wasm32")] {
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        } else {
            //wgpu doesn't use normal error logging, requires env_logger for its custom error messages
            env_logger::init();
        }
    }

    //a way to retrive events sent by the system, and windows registed into the event loop
    let event_loop: EventLoop<()> = EventLoop::new();

    //[TODO?] replace with more permanent solution that doesn't require unsafe?
    //work-around for https://github.com/rust-windowing/winit/issues/2051 - tldr; macos windows don't generate as they should with winit, so this allows them to work instantly
    #[cfg(target_os = "macos")]
    unsafe {
        use cocoa::appkit::NSApplication as _;
        cocoa::appkit::NSApp().setActivationPolicy_(
            cocoa::appkit::NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        );
    }

    //a window that can be manipulated to draw on the screen - in init it gets added to the event loop by the window builder
    let window: Window = WindowBuilder::new().build(&event_loop).unwrap();
    //setup QOL config for the window
    window.set_title("unknown-engine");
    //doens't seem to work?
    // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));

    //the state of the everything related to the program - the window, device, buffers, textures, models, ect
    let mut state: State = State::new(&window).await;
    //when the program last rendered
    let mut last_render_time: instant::Instant = instant::Instant::now();

    //code specific to wasm as it requires extra setup to get working
    #[cfg(target_arch = "wasm32")]
    {
        //winit prevents sizing with CSS, so we have to set the size manually when on web
        use winit::dpi::PhysicalSize;

        //[TODO] decide what resolution to use by default
        window.set_inner_size(PhysicalSize::new(450, 400));

        //black box code to init a wasm window
        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                //the element id corresponds to the element id in the html code for running the program
                let dst = doc.get_element_by_id("wasm")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    //starts the event loop to handle device, program and user events
    event_loop.run(move |event, _, control_flow| {
        //constantly re-renders and continues the scene even when not on the scene (useful for games)
        *control_flow = ControlFlow::Poll;
        match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion{ delta },
                .. // We're not using device_id currently
            } => if state.mouse_pressed {
                state.camera_controller.process_mouse(delta.0, delta.1)
            },
            //if something changes related to the window
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    //see what we will do with each different type of window related event
                    match event {
                        //if the system has requested the window to close, or there is a keyboard input
                        //doesn't work with wasm
                        #[cfg(not(target_arch="wasm32"))]
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            //if escape is pressed, the window will close
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        //if the window has been resized, resize the surface
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        //if the scale factor has been changed, resize the surface
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        //everything else does nothing for now
                        _ => {}
                    }
                }
            }
            //if a redraw of the screen is requested
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                //update internal state
                let now: instant::Instant = instant::Instant::now();
                let dt: instant::Duration = now - last_render_time;
                last_render_time = now;

                state.update(dt);

                //render these changes to the screen
                match state.render() {
                    Ok(_) => {}
                    //reconfigure the surface if lost (if our swap chain (kinda the frame buffer) has been lost)
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    //the system is out of memory, so we should probably quit the program
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    //all other errors (Outdated, Timeout) should be resolved by the next frame and should just be printed to the error log
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            //when the redraw is about to begin (we have no more events to proccess on this frame)
            Event::MainEventsCleared => {
                //redrawRequested will only trigger once, unless we manually request it
                window.request_redraw();
            }
            //all other events do nothing for now
            _ => {}
        }
    });
}

//[TODO] create real tests for the program
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
