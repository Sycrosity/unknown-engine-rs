//for now, before everything is implimented, we will allow unused/dead code to exist without warnings
#![allow(dead_code)]

mod texture;

use wgpu::{include_wgsl, util::DeviceExt};

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

//wasm specific dependencies
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

//needs Pod and Zeroable to be able to cast it to a &[u8]
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
//stores relevant data for a singular vertex
struct Vertex {
    position: [f32; 3],
    //texture coordinates
    tex_coords: [f32; 2],
}

impl Vertex {
    //the default description of what a vertex is - how it stores position, colour, ect
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            //defines the width of a vertex - here most likely 24 bytes
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            //how often to move to the next vertex - can be wgpu::VertexStepMode::Instance if we want to only change vertices when we start drawing an instance
            step_mode: wgpu::VertexStepMode::Vertex,
            //describe the individual parts of a vertex - generally the same structure as the shader (could use the vertex_attr_array![] macro but it requires some jankyness so will keep with this for now
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
            ],
        }
    }
}

//the verticies of whatever shape we are trying to make (here a pentagon)
const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.00759614],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.43041354],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.949397],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.84732914],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.2652641],
    },
];

#[rustfmt::skip]
//the order of the vertices - removes the need to repeat vertices and waste memory
const INDICES: &[u16] = &[
    0, 1, 4,
    1, 2, 4,
    2, 3, 4,
];

//a view into our scene that can move around (using rasterization) and give the perception of depth
struct Camera {
    //where our camera is looking at our scene from
    eye: cgmath::Point3<f32>,
    //what we are looking at (most likely the origin, [0,0,0])
    target: cgmath::Point3<f32>,
    //where up is - used for orientation
    up: cgmath::Vector3<f32>,
    //the aspect ration
    aspect: f32,
    //field of view
    fovy: f32,
    //what counts as too close to render
    znear: f32,
    //what counts as too far away to render
    zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        //a matrix to move the world to where the camera is at
        let view: cgmath::Matrix4<f32> =
            cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        // a matrix that wraps the scene to give the illusion of depth
        let proj: cgmath::Matrix4<f32> =
            cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        // wgpu's coordinate system is based on DirectX and Metal's, whereas normalised device coordinates (present in OpenGL, cgmath and most game math crates) have x and y coords within the range of +1.0 and -1.0 - so we need a matrix to scale and translate cgmath's scene to wgpu's
        #[rustfmt::skip]
        pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.0,
            0.0, 0.0, 0.5, 1.0,
        );

        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
}

//we need this for Rust to store our data correctly for the shaders
#[repr(C)]
//this is so we can store this in a buffer (aka have it turned into a &[u8])
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
//the camera matrix data for use in the buffer
struct CameraUniform {
    //we can't use cgmath with bytemuck directly so we'll have to convert the Matrix4 into a 4x4 f32 array
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    //convert a Camera into a CameraUniform so it can be used in a uniform buffer
    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

//how the camera is controlled
struct CameraController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    fn new(speed: f32) -> Self {
        Self {
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }

    //how to handle camera movement
    fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            //when something is pressed on the keyboard
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state,
                    //save it in the temporary keycode variable
                    virtual_keycode: Some(keycode),
                    ..
                },
                ..
            } => {
                let is_pressed: bool = *state == ElementState::Pressed;
                match keycode {
                    //[TODO] make this part of a config file or user options
                    //keybinds for all directions of camera movement
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    //interpret the 
    fn update_camera(&self, camera: &mut Camera) {
        use cgmath::InnerSpace;
        let forward: cgmath::Vector3<f32> = camera.target - camera.eye;
        let forward_norm: cgmath::Vector3<f32> = forward.normalize();
        let forward_mag: f32 = forward.magnitude();

        //forward_mag > self.speed prevents glitching when camera gets too close to the center of the scene.
        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }

        let right: cgmath::Vector3<f32> = forward_norm.cross(camera.up);

        //redo radius calc in case the fowrard/backward is pressed.
        let forward: cgmath::Vector3<f32> = camera.target - camera.eye;
        let forward_mag: f32 = forward.magnitude();

        if self.is_right_pressed {
            //rescale the distance between the target and eye so that it doesn't change - the eye therefore still lies on the circle made by the target and eye.
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
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
    //buffers are used to store all the data we want to draw (so we don't have to expensively recomplie the shader on every update)
    //to store all the individual vertices in our elements
    vertex_buffer: wgpu::Buffer,
    //to store all the indices to elements in VERTICES to create triangles
    index_buffer: wgpu::Buffer,
    //how many indices are in the INDICES constant
    num_indices: u32,
    //describes how a set of textures can be accessed by the shader
    diffuse_bind_group: wgpu::BindGroup,
    //aa texture generated from texture.rs
    diffuse_texture: texture::Texture,
    //a view into our scene that can move around (using rasterization) and give the perception of depth
    camera: Camera,
    //the camera matrix data for use in the buffer
    camera_uniform: CameraUniform,
    //to store the matrix data associated with the camera
    camera_buffer: wgpu::Buffer,
    //describes how the camera can be accessed by the shader
    camera_bind_group: wgpu::BindGroup,
    //how the camera is controlled
    camera_controller: CameraController,
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
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        //collect the bytes from happy-tree.png
        let diffuse_bytes = include_bytes!("assets/happy-tree.png");
        //create a texture using our texture.rs file and our image bytes
        let diffuse_texture: texture::Texture =
            texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "happy-tree.png").unwrap();

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
                ],
            });

        //describes how a set of textures can be accessed by a shader
        let diffuse_bind_group: wgpu::BindGroup =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("diffuse_bind_group"),
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                ],
            });

        let camera: Camera = Camera {
            // position the camera one unit up and 2 units back - the +z coordinate is out of the screen (coord ranges are 1.0 to -1.0)
            eye: (0.0, 1.0, 2.0).into(),
            //have it look at the origin
            target: (0.0, 0.0, 0.0).into(),
            //which way is "up" - here (0.0, 1.0, 0.0)
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            //a basic, random value - allow user to change in settings
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        //convert our camera matrix into a CameraUniform
        let mut camera_uniform: CameraUniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

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
                    //the camera only needs to be visible to the vertex shader
                    visibility: wgpu::ShaderStages::VERTEX,
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

        //how the camera is controlled
        let camera_controller: CameraController = CameraController::new(0.2);

        //creates a shader from our shader file (in this case, shader.wgsl)
        //the include_wgsl!() macro makes it so we don't have to write really dumb boilerplate code to create the shader
        let shader: wgpu::ShaderModule = device.create_shader_module(include_wgsl!("shader.wgsl"));

        //setup for our rendering pipeline
        let render_pipeline_layout: wgpu::PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                //the list of bind groups being used
                bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        //describes the actions our gpu will perform when acting on a set of data
        let render_pipeline: wgpu::RenderPipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    //specifies which shader function should be our entrypoint
                    entry_point: "vs_main",
                    //the types of vertices we want to pass to the vertex shader
                    buffers: &[Vertex::desc()],
                },
                //technically optional, so has to be wrapped in a Some enum
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    //for now, only need one for surface
                    targets: &[Some(wgpu::ColorTargetState {
                        // 4.
                        format: config.format,
                        //for now, blending should just replace old pixel data with new pixel data
                        blend: Some(wgpu::BlendState::REPLACE),
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
                //we aren't using a depth or stensil buffer yet so this doesn't apply
                depth_stencil: None,
                //[TODO] learn what multisampling is and add comments for it
                multisample: wgpu::MultisampleState {
                    //determines how many samples should be active
                    count: 1,
                    //specifies which samples should be active - in this case all of them ( represented by !0 )
                    mask: !0,
                    //for anti-aliasing - doesn't apply for now
                    alpha_to_coverage_enabled: false,
                },
                //how many array layers render attachments can have - we aren't rendering to array layers, so for now this is 0
                multiview: None,
            });

        //a buffer to store the vertex data we want to draw (so we don't have to expensively recomplie the shader on every update)
        let vertex_buffer: wgpu::Buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                //cast to &[u8] as that is how gpu buffers typically expect buffer data
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer: wgpu::Buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let num_indices: u32 = INDICES.len() as u32;

        //return all of our created data in a State struct
        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            diffuse_texture,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller
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
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        //an inputs should return true if something changed, and false if nothing changed
        self.camera_controller.process_events(event)
    }

    fn update(&mut self) {

        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        //write to the buffer with our updated data
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
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
                    //will be used later - for now is just None.
                    depth_stencil_attachment: None,
                });

            //set the rendering pipeline to the only one we have so far
            render_pass.set_pipeline(&self.render_pipeline);
            //tells wgu how to access textures
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            //tells wgu how to use apply the camera matrix
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            //tells wgpu what slice of the vertex buffer to use - here it's .. which means all of it
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            //tells wgpu what slice of the index buffer to use (all of it), and what format the indices are in
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            //tells wgpu to draw something using our indices and vertices
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1); // 3.
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
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
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

    //[TODO] add description
    let mut state: State = State::new(&window).await;

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

    // println!("{:?}", state.camera.build_view_projection_matrix());

    //starts the event loop to handle device, program and user events
    event_loop.run(move |event, _, control_flow| match event {
        //if something changes related to the window
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            if !state.input(event) {
                //see what we will do with each different type of window related event
                match event {
                    //if the system has requested the window to close, or there is a keyboard input
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
            state.update();
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
