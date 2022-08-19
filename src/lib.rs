//for now, before everything is implimented, we will allow unused/dead code to exist without warnings
#![allow(dead_code)]

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
    color: [f32; 3],
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

//the verticies of whatever shape we are trying to make (here a pentagon)
const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [0.5, 0.0, 0.5],
    },
];

//the order of the vertices - removes the need to repeat vertices and waste memory
const INDICES: &[u16] = &[
    0, 1, 4, //
    1, 2, 4, //
    2, 3, 4, //
];

//[TODO] add description
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
    //buffers are to store all the data we want to draw (so we don't have to expensively recomplie the shader on every update)
    //to store all the individual vertices in our elements
    vertex_buffer: wgpu::Buffer,
    //to store all the indices to elements in VERTICES to create triangles
    index_buffer: wgpu::Buffer,
    //how many indices are in the INDICES constant
    num_indices: u32,
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

        //creates a shader from our shader file (in this case, shader.wgsl)
        //the include_wgsl!() macro makes it so we don't have to write really dumb boilerplate code to create the shader
        let shader: wgpu::ShaderModule = device.create_shader_module(include_wgsl!("shader.wgsl"));

        //black box setup for rendering pipeline
        let render_pipeline_layout: wgpu::PipelineLayout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
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

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = INDICES.len() as u32;

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

    #[allow(unused)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        //for now, we don't have any events to capture so we leave this false

        false
    }

    fn update(&mut self) {

        //nothing to update for now, so this remains empty
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
