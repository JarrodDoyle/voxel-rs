use winit::{dpi::PhysicalSize, window::Window};

use crate::texture::{Texture, TextureBuilder};

pub(crate) struct RenderContext {
    pub instance: wgpu::Instance,
    pub size: PhysicalSize<u32>,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl RenderContext {
    pub async fn new(window: &Window) -> Self {
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
                    limits: wgpu::Limits::default(),
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
            instance,
            size,
            surface,
            surface_config,
            adapter,
            device,
            queue,
        }
    }
}

pub(crate) struct Renderer {
    clear_color: wgpu::Color,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    render_texture: Texture,
}

impl Renderer {
    pub fn new(context: &RenderContext) -> Self {
        log::info!("Creating render shader...");
        let shader_descriptor = wgpu::include_wgsl!("../assets/shaders/shader.wgsl");
        let shader = context.device.create_shader_module(shader_descriptor);

        log::info!("Creating render texture...");
        let render_texture = TextureBuilder::new()
            .with_size(context.size.width, context.size.height, 1)
            .with_format(wgpu::TextureFormat::Rgba8Unorm)
            .with_usage(
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
            )
            .with_shader_visibility(wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE)
            .build(&context);

        let data_len = context.size.width * context.size.height * 4;
        let mut data: Vec<u8> = Vec::with_capacity(data_len.try_into().unwrap());
        for _ in 0..context.size.height {
            for _ in 0..context.size.width {
                data.push(255u8);
                data.push(0u8);
                data.push(0u8);
                data.push(255u8);
            }
        }
        render_texture.update(&context, &data);

        log::info!("Creating render pipeline...");
        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&context.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("draw"),
                            bind_group_layouts: &[&render_texture.bind_group_layout],
                            push_constant_ranges: &[],
                        },
                    )),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vertex",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fragment",
                        targets: &[Some(context.surface_config.format.into())],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

        log::info!("Creating compute pipeline...");
        let cs_descriptor = wgpu::include_wgsl!("../assets/shaders/image_recolor.wgsl");
        let cs = context.device.create_shader_module(cs_descriptor);
        let compute_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: render_texture.attributes.format,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    }],
                });
        let compute_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &compute_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&render_texture.view),
                }],
            });
        let compute_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: Some(&context.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("compute"),
                            bind_group_layouts: &[&compute_layout],
                            push_constant_ranges: &[],
                        },
                    )),
                    module: &cs,
                    entry_point: "compute",
                });

        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let size = render_texture.attributes.size;
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &compute_bind_group, &[]);
        compute_pass.dispatch_workgroups(size.width / 8, size.height / 8, 1);
        drop(compute_pass);

        context.queue.submit(Some(encoder.finish()));

        let clear_color = wgpu::Color {
            r: 255.0 / 255.0,
            g: 216.0 / 255.0,
            b: 102.0 / 255.0,
            a: 1.0,
        };

        Self {
            clear_color,
            compute_pipeline,
            compute_bind_group,
            render_pipeline,
            render_texture,
        }
    }

    pub fn render(&mut self, context: &RenderContext) {
        let frame = context.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_texture.bind_group, &[]);
        render_pass.draw(0..6, 0..1);

        drop(render_pass);

        context.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
