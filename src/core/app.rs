use std::time::Instant;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use super::camera;
use crate::render;

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

        let render_ctx = render::Context::new(&window).await;

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

        let camera_bind_group_layout = render::BindGroupLayoutBuilder::new()
            .with_label("camera_bind_group_layout")
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .build(&self.render_ctx);
        let camera_bind_group = render::BindGroupBuilder::new()
            .with_label("camera_bind_group")
            .with_layout(&camera_bind_group_layout)
            .with_entry(camera_controller.get_buffer().as_entire_binding())
            .build(&self.render_ctx);

        let renderer = render::Renderer::new(&self.render_ctx, &camera_bind_group_layout);

        let mut last_render_time = Instant::now();
        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == self.window.id() => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {
                        camera_controller.process_events(&event);
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
                    renderer.render(&self.render_ctx, &camera_bind_group);
                }
                _ => {}
            });
    }
}
