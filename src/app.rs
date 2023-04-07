use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::renderer;

pub(crate) struct App {
    window: winit::window::Window,
    event_loop: EventLoop<()>,
    render_ctx: renderer::RenderContext,
    renderer: renderer::Renderer,
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

        let render_ctx = renderer::RenderContext::new(&window).await;
        let renderer = renderer::Renderer::new(&render_ctx);

        Self {
            window,
            event_loop,
            render_ctx,
            renderer,
        }
    }

    pub fn run(mut self) {
        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == self.window.id() => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {}
                },
                Event::MainEventsCleared => {
                    self.window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    self.renderer.render(&self.render_ctx);
                }
                _ => {}
            });
    }
}
