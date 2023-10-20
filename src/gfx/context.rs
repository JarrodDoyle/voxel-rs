use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::Window,
};

pub struct Context {
    pub window: Window,
    pub instance: wgpu::Instance,
    pub size: PhysicalSize<u32>,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Context {
    pub async fn new(window: Window, limits: wgpu::Limits) -> Self {
        log::info!("Initialising WGPU context...");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            dx12_shader_compiler: Default::default(),
        });

        // To be able to start drawing we need a few things:
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
                    limits,
                },
                None,
            )
            .await
            .unwrap();

        log::info!("Configuring window surface...");
        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &surface_config);

        Self {
            window,
            instance,
            size,
            surface,
            surface_config,
            adapter,
            device,
            queue,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn handle_window_event(
        &mut self,
        event: &Event<()>,
        control_flow: &mut ControlFlow,
    ) -> bool {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if *window_id == self.window.id() => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                    true
                }
                WindowEvent::Resized(physical_size) => {
                    self.resize(*physical_size);
                    false
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    self.resize(**new_inner_size);
                    true
                }
                _ => false,
            },
            Event::MainEventsCleared => {
                self.window.request_redraw();
                true
            }
            _ => false,
        }
    }
}
