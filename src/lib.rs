use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

//wasm specific dependencies
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

//tells wasm to run the run() function when wasm is initialised
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]

//for when using multithreading
// pub async fn run() {

//run the rasterizer
pub fn run() {
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
    //a window that can be manipulated to draw on the screen - in init it gets added to the event loop by the window builder
    let window: Window = WindowBuilder::new().build(&event_loop).unwrap();

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

    //starts the event loop
    event_loop.run(
        move |event: Event<()>, _, control_flow: &mut ControlFlow| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                //system requested window closing
                WindowEvent::CloseRequested
                //any keyboard input
                | WindowEvent::KeyboardInput {
                    //if escape is pressed, the window will close.
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            _ => {}
        },
    );
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
