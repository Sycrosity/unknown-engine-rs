use cgmath::*;
use instant::Duration;
use std::f32::consts::FRAC_PI_2;
use winit::dpi::PhysicalPosition;
use winit::event::*;

// wgpu's coordinate system is based on DirectX and Metal's, whereas normalised device coordinates (present in OpenGL, cgmath and most game math crates) have x and y coords within the range of +1.0 and -1.0 - so we need a matrix to scale and translate cgmath's scene to wgpu's
#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

//helps us stop the camera looking straight up or straight down (which causes issues)
const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

//a view into our scene that can move and look around
#[derive(Debug)]
pub struct Camera {
    //where our camera is looking at our scene from
    pub position: Point3<f32>,
    //horizontal rotation (left-right)
    yaw: Rad<f32>,
    //vertical rotation (up-down)
    pitch: Rad<f32>,
}

impl Camera {
    pub fn new<V: Into<Point3<f32>>, Y: Into<Rad<f32>>, P: Into<Rad<f32>>>(
        position: V,
        yaw: Y,
        pitch: P,
    ) -> Self {
        Self {
            position: position.into(),
            yaw: yaw.into(),
            pitch: pitch.into(),
        }
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        Matrix4::look_to_rh(
            self.position,
            Vector3::new(self.yaw.0.cos(), self.pitch.0.sin(), self.yaw.0.sin()).normalize(),
            Vector3::unit_y(),
        )
    }
}

//a set of settings relating to how the camera looks and percieves the scene
pub struct Projection {
    //the aspect ratio
    aspect: f32,
    //field of view
    fov: Rad<f32>,
    //what counts as too close to render
    znear: f32,
    //what counts as too far away to render
    zfar: f32,
}

impl Projection {
    pub fn new<F: Into<Rad<f32>>>(width: u32, height: u32, fov: F, znear: f32, zfar: f32) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fov: fov.into(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    //perspective makes a matrix from four values
    pub fn calc_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * perspective(self.fov, self.aspect, self.znear, self.zfar)
    }
}

#[derive(Debug)]
pub struct CameraController {
    //
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    //how fast the camera moves when we tell it to move
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        //[TODO?] replace amount values with true false bools?
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            //how fast the camera swings around
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        let amount: f32 = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::Space => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::LShift => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn process_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            //assuming a line is about 100 pixels
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll as f32,
        };
    }

    //dt = delta_time
    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration) {
        let dt: f32 = dt.as_secs_f32();

        //move forward/backward and left/right
        let (yaw_sin, yaw_cos) = camera.yaw.0.sin_cos();
        let forward: Vector3<f32> = Vector3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right: Vector3<f32> = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        camera.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        //move in/out (aka. "zoom")
        //note: this isn't an actual zoom - The camera's position changes when zooming - this is just to make it easier to get closer to an object you want to focus on
        //[TODO] create an actual zoom (for gun sights ect)
        let (pitch_sin, pitch_cos) = camera.pitch.0.sin_cos();
        let scrollward: Vector3<f32> =
            Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        camera.position += scrollward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        //move up/down - since we don't use roll, we can just modify the y coordinate directly
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        //rotate
        camera.yaw += Rad(self.rotate_horizontal) * self.sensitivity * dt;
        camera.pitch += Rad(-self.rotate_vertical) * self.sensitivity * dt;

        //if process_mouse isn't called every frame, these values will not get set to zero, and the camera will rotate when moving in a non cardinal direction
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        //keep the camera's angle from going too high/low (as this can cause issues)
        if camera.pitch < -Rad(SAFE_FRAC_PI_2) {
            camera.pitch = -Rad(SAFE_FRAC_PI_2);
        } else if camera.pitch > Rad(SAFE_FRAC_PI_2) {
            camera.pitch = Rad(SAFE_FRAC_PI_2);
        }
    }
}
