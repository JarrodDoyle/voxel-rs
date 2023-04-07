use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

pub(crate) struct AppWindow {
    window: winit::window::Window,
    event_loop: EventLoop<()>,
    instance: wgpu::Instance,
    size: PhysicalSize<u32>,
    surface: wgpu::Surface,
    config: wgpu::SurfaceConfiguration,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl AppWindow {
    pub async fn new(width: u32, height: u32, title: &str) -> Self {
        log::info!("Initialising window...");
        let size = PhysicalSize::new(width, height);
        let event_loop = EventLoop::new();
        let window = winit::window::WindowBuilder::new()
            .with_title(title)
            .with_inner_size(size)
            .build(&event_loop)
            .unwrap();

        log::info!("Initialising WGPU context...");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            dx12_shader_compiler: Default::default(),
        });

        // To be able to start drawing to our window we need a few things:
        // - A surface
        // - A GPU device to draw to the surface
        // - A draw command queue
        log::info!("Initialising window surface...");
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        log::info!("Requesting GPU adapter...");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        log::info!("Checking GPU adapter meets requirements");
        log::info!("Requesting GPU device...");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        log::info!("Configuring window surface...");
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        Self {
            window,
            event_loop,
            instance,
            size,
            surface,
            config,
            adapter,
            device,
            queue,
        }
    }

    pub fn run(self) {
        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == self.window.id() => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {}
                },
                _ => {}
            });
    }
}
