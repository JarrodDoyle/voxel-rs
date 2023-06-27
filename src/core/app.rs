use std::time::Instant;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use super::camera;
use crate::{
    render::{self, Renderer},
    voxel,
};

pub struct App {
    window: winit::window::Window,
    event_loop: EventLoop<()>,
    render_ctx: render::Context,
}

impl App {
    pub async fn new(width: u32, height: u32, title: &str) -> Self {
        log::info!("Initialising window...");
        let size = PhysicalSize::new(width, height);
        let event_loop = EventLoop::new();
        let window = winit::window::WindowBuilder::new()
            .with_title(title)
            .with_inner_size(size)
            .build(&event_loop)
            .unwrap();

        let render_ctx = render::Context::new(
            &window,
            wgpu::Limits {
                max_storage_buffer_binding_size: 1 << 29,
                max_buffer_size: 1 << 29,
                ..Default::default()
            },
        )
        .await;

        Self {
            window,
            event_loop,
            render_ctx,
        }
    }

    pub fn run(self) {
        let mut camera_controller = camera::CameraController::new(
            &self.render_ctx,
            camera::Camera::new(
                glam::Vec3 {
                    x: 4.01,
                    y: 4.01,
                    z: 20.0,
                },
                -90.0_f32.to_radians(),
                0.0_f32.to_radians(),
            ),
            camera::Projection::new(
                self.render_ctx.size.width,
                self.render_ctx.size.height,
                90.0_f32.to_radians(),
                0.01,
                100.0,
            ),
            10.0,
            0.25,
        );

        let mut world = voxel::world::WorldManager::new(
            voxel::world::GenerationSettings {
                seed: 0,
                frequency: 0.04,
                octaves: 3,
                gain: 0.5,
                lacunarity: 2.0,
            },
            glam::uvec3(32, 32, 32),
        );

        let mut renderer = voxel::VoxelRenderer::new(&self.render_ctx, &camera_controller);

        let mut cumulative_dt = 0.0;
        let mut frames_accumulated = 0.0;
        let mut last_render_time = Instant::now();
        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == self.window.id() => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {
                        camera_controller.process_events(event);
                    }
                },
                Event::MainEventsCleared => {
                    self.window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    let now = Instant::now();
                    let dt = now - last_render_time;
                    last_render_time = now;
                    camera_controller.update(dt);
                    camera_controller.update_buffer(&self.render_ctx);
                    renderer.render(&self.render_ctx);
                    renderer.update(&dt, &self.render_ctx);
                    renderer.update_brickmap(&self.render_ctx, &mut world);

                    // Simple framerate tracking
                    cumulative_dt += dt.as_secs_f32();
                    frames_accumulated += 1.0;
                    if cumulative_dt >= 1.0 {
                        let fps = frames_accumulated * 1.0 / cumulative_dt;
                        let frame_time = cumulative_dt * 1000.0 / frames_accumulated;
                        log::info!("FPS: {}, Frame Time: {}", fps.floor(), frame_time);
                        cumulative_dt = 0.0;
                        frames_accumulated = 0.0;
                    }
                }
                _ => {}
            });
    }
}
