use std::time::Duration;
use wgpu::util::DeviceExt;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::gfx::Context;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    projection: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    pos: [f32; 3],
    _pad: f32,
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            projection: glam::Mat4::IDENTITY.to_cols_array_2d(),
            view: glam::Mat4::IDENTITY.to_cols_array_2d(),
            pos: glam::Vec3::ZERO.to_array(),
            _pad: 0.0,
        }
    }

    pub fn update(&mut self, view: glam::Mat4, projection: glam::Mat4, pos: glam::Vec3) {
        self.view = view.to_cols_array_2d();
        self.projection = projection.to_cols_array_2d();
        self.pos = pos.to_array();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Camera {
    pub fn new(position: glam::Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
        }
    }

    pub fn get_view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_to_rh(
            self.position,
            glam::vec3(
                self.pitch.cos() * self.yaw.cos(),
                self.pitch.sin(),
                self.pitch.cos() * self.yaw.sin(),
            )
            .normalize(),
            glam::Vec3::Y,
        )
        .transpose()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Projection {
    aspect: f32,
    fov_y: f32,
    z_near: f32,
    z_far: f32,
}

impl Projection {
    pub fn new(width: u32, height: u32, fov_y: f32, z_near: f32, z_far: f32) -> Self {
        let aspect = height as f32 / width as f32;
        Self {
            aspect,
            fov_y,
            z_near,
            z_far,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = height as f32 / width as f32;
    }

    pub fn get_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far).transpose()
    }
}

#[derive(Debug)]
pub struct CameraController {
    camera: Camera,
    projection: Projection,
    uniform: CameraUniform,
    buffer: wgpu::Buffer,
    move_speed: f32,
    mouse_sensitivity: f32,
    move_dirs_pressed: glam::IVec3,
    rot_dirs_pressed: glam::IVec2,
}

impl CameraController {
    pub fn new(
        context: &Context,
        camera: Camera,
        projection: Projection,
        move_speed: f32,
        mouse_sensitivity: f32,
    ) -> Self {
        let mut uniform = CameraUniform::new();
        uniform.update(
            camera.get_view_matrix(),
            projection.get_matrix(),
            camera.position,
        );

        let buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            camera,
            projection,
            uniform,
            buffer,
            move_speed,
            mouse_sensitivity,
            move_dirs_pressed: glam::ivec3(0, 0, 0),
            rot_dirs_pressed: glam::ivec2(0, 0),
        }
    }

    pub fn process_events(&mut self, event: &WindowEvent) -> bool {
        let mut handled = true;
        match event {
            WindowEvent::Resized(physical_size) => {
                self.projection
                    .resize(physical_size.width, physical_size.height);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => {
                let val = match state {
                    ElementState::Pressed => 1,
                    ElementState::Released => 0,
                };

                match keycode {
                    KeyCode::KeyW => {
                        self.move_dirs_pressed.z = val;
                    }
                    KeyCode::KeyS => {
                        self.move_dirs_pressed.z = -val;
                    }
                    KeyCode::KeyA => {
                        self.move_dirs_pressed.x = -val;
                    }
                    KeyCode::KeyD => {
                        self.move_dirs_pressed.x = val;
                    }
                    KeyCode::KeyQ => {
                        self.move_dirs_pressed.y = val;
                    }
                    KeyCode::KeyE => {
                        self.move_dirs_pressed.y = -val;
                    }
                    KeyCode::ArrowUp => {
                        self.rot_dirs_pressed.y = val;
                    }
                    KeyCode::ArrowDown => {
                        self.rot_dirs_pressed.y = -val;
                    }
                    KeyCode::ArrowLeft => {
                        self.rot_dirs_pressed.x = -val;
                    }
                    KeyCode::ArrowRight => {
                        self.rot_dirs_pressed.x = val;
                    }
                    _ => handled = false,
                }
            }
            _ => handled = false,
        }

        handled
    }

    pub fn update(&mut self, dt: Duration) {
        let dt = dt.as_secs_f32();

        // Calculate look vectors
        let pitch = self.camera.pitch;
        let yaw = self.camera.yaw;
        let front = glam::vec3(
            pitch.cos() * yaw.cos(),
            pitch.sin(),
            pitch.cos() * yaw.sin(),
        )
        .normalize();
        let right = front.cross(glam::Vec3::Y).normalize();
        let up = right.cross(front).normalize();

        // Apply movement
        let ms = self.move_speed * dt;
        self.camera.position += front * ms * self.move_dirs_pressed.z as f32;
        self.camera.position += right * ms * self.move_dirs_pressed.x as f32;
        self.camera.position += up * ms * self.move_dirs_pressed.y as f32;

        // Apply rotation
        let cam_ms = (self.move_speed * self.move_speed).to_radians() * dt;
        let max_pitch = 85_f32.to_radians();
        self.camera.yaw += cam_ms * self.rot_dirs_pressed.x as f32;
        self.camera.pitch += cam_ms * self.rot_dirs_pressed.y as f32;
        self.camera.pitch = self.camera.pitch.clamp(-max_pitch, max_pitch);

        // Debug log
        // log::info!("Camera Front: {:?}", front);
        // log::info!("Move Speed: {:?} {:?} {:?}", self.move_speed, ms, dt);
        // log::info!("Camera Position: {:?}", self.camera.position);
        // log::info!("Camera Yaw: {:?}", self.camera.yaw);
        // log::info!("Camera Pitch: {:?}", self.camera.pitch);
    }

    pub fn update_buffer(&mut self, context: &Context) {
        self.uniform.update(
            self.camera.get_view_matrix(),
            self.projection.get_matrix(),
            self.camera.position,
        );
        context
            .queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}
