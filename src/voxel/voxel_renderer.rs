use std::time::Duration;

use crate::{core, render};

#[derive(Debug)]
pub struct VoxelRenderer {
    clear_color: wgpu::Color,
    render_texture: render::Texture,
    render_pipeline: wgpu::RenderPipeline,
    brickmap_manager: super::brickmap::BrickmapManager,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,
}

impl VoxelRenderer {
    pub fn new(context: &render::Context, camera_controller: &core::CameraController) -> Self {
        log::info!("Creating render shader...");
        let shader_descriptor = wgpu::include_wgsl!("../../assets/shaders/shader.wgsl");
        let shader = context.device.create_shader_module(shader_descriptor);

        log::info!("Creating render texture...");
        let render_texture = render::TextureBuilder::new()
            .with_size(context.size.width, context.size.height, 1)
            .with_format(wgpu::TextureFormat::Rgba8Unorm)
            .with_usage(
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
            )
            .with_shader_visibility(wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE)
            .build(&context);

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

        log::info!("Creating brickmap manager...");
        let mut brickmap_manager = super::brickmap::BrickmapManager::new(context);
        let sphere_center = glam::vec3(3.5, 3.5, 3.5);
        let sphere_r2 = u32::pow(4, 2) as f32;
        for chunk_idx in 0..32768 {
            if chunk_idx % 3 == 0 || chunk_idx % 5 == 0 || chunk_idx % 7 == 0 {
                continue;
            }

            let chunk_pos = glam::uvec3(chunk_idx % 32, (chunk_idx / 32) % 32, chunk_idx / 1024);
            let mut bitmask_data = [0xFFFFFFFF as u32; 16];
            let mut albedo_data = Vec::<u32>::new();
            for z in 0..8 {
                let mut entry = 0u64;
                for y in 0..8 {
                    for x in 0..8 {
                        let idx = x + y * 8;
                        let pos = glam::vec3(x as f32, y as f32, z as f32);
                        if (pos - sphere_center).length_squared() <= sphere_r2 {
                            entry += 1 << idx;
                            let mut albedo = 0u32;
                            albedo += ((chunk_pos.x + 1) * 8 - 1) << 24;
                            albedo += ((chunk_pos.y + 1) * 8 - 1) << 16;
                            albedo += ((chunk_pos.z + 1) * 8 - 1) << 8;
                            albedo += 255;
                            albedo_data.push(albedo);
                        }
                    }
                }
                bitmask_data[2 * z as usize] = (entry & 0xFFFFFFFF).try_into().unwrap();
                bitmask_data[2 * z as usize + 1] = ((entry >> 32) & 0xFFFFFFFF).try_into().unwrap();
            }
            brickmap_manager.set_data(chunk_pos, &bitmask_data, &albedo_data);
        }
        brickmap_manager.update_buffer(context);

        log::info!("Creating compute pipeline...");
        let cs_descriptor = wgpu::include_wgsl!("../../assets/shaders/voxel_volume.wgsl");
        let cs = context.device.create_shader_module(cs_descriptor);
        let compute_layout = render::BindGroupLayoutBuilder::new()
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: render_texture.attributes.format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                None,
            )
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                None,
            )
            .build(context);
        let compute_bind_group = render::BindGroupBuilder::new()
            .with_layout(&compute_layout)
            .with_entry(wgpu::BindingResource::TextureView(&render_texture.view))
            .with_entry(brickmap_manager.get_worldstate_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickgrid_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickmap_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_shading_buffer().as_entire_binding())
            .with_entry(camera_controller.get_buffer().as_entire_binding())
            .build(context);
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

        Self {
            clear_color: wgpu::Color::BLACK,
            render_texture,
            render_pipeline,
            brickmap_manager,
            compute_pipeline,
            compute_bind_group,
        }
    }
}

impl render::Renderer for VoxelRenderer {
    fn render(&self, context: &render::Context) {
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
    }

    fn update(&self, dt: &Duration, context: &render::Context) {}
}
