use std::sync::Arc;

use anyhow::{Context as _, Result};
use winit::{
    dpi::PhysicalSize, event::WindowEvent, event_loop::EventLoopWindowTarget, window::Window,
};

pub struct Context<'window> {
    pub window: Arc<Window>,
    pub instance: wgpu::Instance,
    pub size: PhysicalSize<u32>,
    pub surface: wgpu::Surface<'window>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl<'window> Context<'window> {
    pub async fn new(window: Arc<Window>, limits: wgpu::Limits) -> Result<Self> {
        log::info!("Initialising WGPU context...");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            dx12_shader_compiler: Default::default(),
            ..Default::default()
        });

        // To be able to start drawing we need a few things:
        // - A surface
        // - A GPU device to draw to the surface
        // - A draw command queue
        log::info!("Initialising window surface...");
        let surface = instance.create_surface(window.clone())?;

        log::info!("Requesting GPU adapter...");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        log::info!("Checking GPU adapter meets requirements");
        log::info!("Requesting GPU device...");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: limits,
                },
                None,
            )
            .await?;

        log::info!("Configuring window surface...");
        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .context("Surface configuration unsupported by adapter")?;
        surface.configure(&device, &surface_config);

        Ok(Self {
            window,
            instance,
            size,
            surface,
            surface_config,
            adapter,
            device,
            queue,
        })
    }

    pub fn resize_surface(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn handle_window_event(
        &mut self,
        event: &WindowEvent,
        elwt: &EventLoopWindowTarget<()>,
    ) -> bool {
        let mut handled = true;
        match event {
            WindowEvent::CloseRequested => {
                elwt.exit();
            }
            WindowEvent::Resized(physical_size) => {
                self.resize_surface(*physical_size);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.resize_surface(self.window.inner_size());
            }

            _ => handled = false,
        }

        handled
    }
}
