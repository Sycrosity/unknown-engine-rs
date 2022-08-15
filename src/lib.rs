use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

//run the rasterizer
pub fn run() {
    //wgpu doesn't use normal error logging, requires env_logger for its custom error messages
    env_logger::init();

    //a way to retrive events sent by the system, and windows registed into the event loop
    let event_loop: EventLoop<()> = EventLoop::new();
    //a window that can be manipulated to draw on the screen - in init it gets added to the event loop by the window builder
    let window: Window = WindowBuilder::new().build(&event_loop).unwrap();

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
