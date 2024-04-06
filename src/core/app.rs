use std::{sync::Arc, time::Instant};

use anyhow::Result;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
};

use super::camera;
use crate::{
    gfx::{self, Renderer},
    voxel::{self, brickworld::BrickmapRenderer},
};

pub struct App<'window> {
    title: String,
    event_loop: EventLoop<()>,
    render_ctx: gfx::Context<'window>,
}

impl<'window> App<'window> {
    pub async fn new(width: u32, height: u32, title: &str) -> Result<Self> {
        log::info!("Initialising window...");
        let size = PhysicalSize::new(width, height);
        let event_loop = EventLoop::new()?;
        let window = Arc::new(
            winit::window::WindowBuilder::new()
                .with_title(title)
                .with_inner_size(size)
                .build(&event_loop)?,
        );

        let render_ctx = gfx::Context::new(
            window,
            wgpu::Limits {
                max_storage_buffer_binding_size: 1 << 30,
                max_buffer_size: 1 << 30,
                ..Default::default()
            },
        )
        .await?;

        Ok(Self {
            title: title.to_owned(),
            event_loop,
            render_ctx,
        })
    }

    pub fn run(mut self) -> Result<()> {
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

        let mut renderer = BrickmapRenderer::new(&self.render_ctx, &camera_controller)?;

        let mut cumulative_dt = 0.0;
        let mut frames_accumulated = 0.0;
        let mut last_render_time = Instant::now();
        self.event_loop.run(|event, elwt| {
            match event {
                Event::WindowEvent { window_id, event }
                    if window_id == self.render_ctx.window.id() =>
                {
                    if self.render_ctx.handle_window_event(&event, elwt) {
                        return;
                    }

                    if camera_controller.process_events(&event) {
                        return;
                    }

                    if let WindowEvent::RedrawRequested = event {
                        let now = Instant::now();
                        let dt = now - last_render_time;
                        last_render_time = now;
                        camera_controller.update(dt);
                        camera_controller.update_buffer(&self.render_ctx);

                        // !Hack: As far as I know I can't propagate errors out of here. So for now just ignore them
                        let _ = renderer.render(&self.render_ctx);
                        let _ = renderer.update(&dt, &self.render_ctx);
                        renderer.update_brickmap(&self.render_ctx, &mut world);

                        // Simple framerate tracking
                        self.render_ctx.window.set_title(&format!(
                            "{}: {} fps",
                            self.title,
                            (1.0 / dt.as_secs_f32()).floor()
                        ));
                        cumulative_dt += dt.as_secs_f32();
                        frames_accumulated += 1.0;
                        if cumulative_dt >= 1.0 {
                            let fps = frames_accumulated * 1.0 / cumulative_dt;
                            let frame_time = cumulative_dt * 1000.0 / frames_accumulated;
                            log::info!("FPS: {}, Frame Time: {}", fps.floor(), frame_time);
                            cumulative_dt = 0.0;
                            frames_accumulated = 0.0;
                        }

                        self.render_ctx.window.request_redraw();
                    }
                }
                _ => (),
            }
        })?;

        Ok(())
    }
}
