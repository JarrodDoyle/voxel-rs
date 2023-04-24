use super::{BindGroupBuilder, BindGroupLayoutBuilder, Context, Texture, TextureBuilder};

pub struct Renderer {
    clear_color: wgpu::Color,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    render_texture: Texture,
    voxel_texture: Texture,
}

impl Renderer {
    pub fn new(context: &Context, camera_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        log::info!("Creating render shader...");
        let shader_descriptor = wgpu::include_wgsl!("../../assets/shaders/shader.wgsl");
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

        log::info!("Creating voxel volume texture...");
        let voxel_texture = TextureBuilder::new()
            .with_size(8, 8, 8)
            .with_dimension(wgpu::TextureDimension::D3)
            .with_format(wgpu::TextureFormat::Rgba8Unorm)
            .with_usage(
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
            )
            .with_shader_visibility(wgpu::ShaderStages::COMPUTE)
            .build(&context);

        let data_len = 8 * 8 * 8 * 4;
        let mut data: Vec<u8> = Vec::with_capacity(data_len.try_into().unwrap());
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    let pos = glam::vec3(x as f32, y as f32, z as f32) - glam::vec3(3.5, 3.5, 3.5);
                    if pos.length_squared() <= (u32::pow(4, 2) as f32) {
                        data.push(((x + 1) * 32 - 1) as u8);
                        data.push(((y + 1) * 32 - 1) as u8);
                        data.push(((z + 1) * 32 - 1) as u8);
                        data.push(255u8);
                    } else {
                        data.push(0u8);
                        data.push(0u8);
                        data.push(0u8);
                        data.push(0u8);
                    }
                }
            }
        }
        voxel_texture.update(&context, &data);

        log::info!("Creating camera...");

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
        let cs_descriptor = wgpu::include_wgsl!("../../assets/shaders/voxel_volume.wgsl");
        let cs = context.device.create_shader_module(cs_descriptor);
        let compute_layout = BindGroupLayoutBuilder::new()
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: render_texture.attributes.format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                None,
            )
            .build(context);
        let compute_bind_group = BindGroupBuilder::new()
            .with_layout(&compute_layout)
            .with_entry(wgpu::BindingResource::TextureView(&render_texture.view))
            .build(context);
        let compute_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: Some(&context.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("compute"),
                            bind_group_layouts: &[
                                &compute_layout,
                                &voxel_texture.bind_group_layout,
                                &camera_bind_group_layout,
                            ],
                            push_constant_ranges: &[],
                        },
                    )),
                    module: &cs,
                    entry_point: "compute",
                });

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
            voxel_texture,
        }
    }

    pub fn render(&self, context: &Context, camera_bind_group: &wgpu::BindGroup) {
        let frame = context.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let size = self.render_texture.attributes.size;
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
        compute_pass.set_bind_group(1, &self.voxel_texture.bind_group, &[]);
        compute_pass.set_bind_group(2, camera_bind_group, &[]);
        compute_pass.dispatch_workgroups(size.width / 8, size.height / 8, 1);
        drop(compute_pass);

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

        // let mut command_buffers = Vec::with_capacity(encoders.len());
        // for encoder in encoders {
        //     command_buffers.push(encoder.finish());
        // }

        // context.queue.submit(command_buffers);
        // frame.present();
    }
}
