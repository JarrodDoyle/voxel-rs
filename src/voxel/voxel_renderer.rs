use std::time::Duration;

use crate::{core, render};

#[derive(Debug)]
pub struct VoxelRenderer {
    clear_color: wgpu::Color,
    render_texture: render::Texture,
    render_pipeline: wgpu::RenderPipeline,
    brickmap_manager: super::brickmap::BrickmapManager,
    raycast_pipeline: wgpu::ComputePipeline,
    raycast_bind_group: wgpu::BindGroup,
    unpack_pipeline: wgpu::ComputePipeline,
    unpack_bind_group: wgpu::BindGroup,
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
            .build(context);

        log::info!("Creating render pipeline...");
        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Raycast Quad"),
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
        let brickgrid_dims = glam::uvec3(64, 64, 64);
        let brickmap_manager = super::brickmap::BrickmapManager::new(
            context,
            brickgrid_dims,
            usize::pow(32, 3),
            u32::pow(2, 24),
            256,
            512,
        );

        log::info!("Creating compute pipelines...");
        let cs_descriptor = wgpu::include_wgsl!("../../assets/shaders/brickmap_upload.wgsl");
        let cs = context.device.create_shader_module(cs_descriptor);
        let unpack_layout = render::BindGroupLayoutBuilder::new()
            .with_label("GPU Unpack BGL")
            .with_uniform_entry(wgpu::ShaderStages::COMPUTE)
            .with_rw_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_rw_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_rw_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_ro_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_ro_storage_entry(wgpu::ShaderStages::COMPUTE)
            .build(context);
        let unpack_bind_group = render::BindGroupBuilder::new()
            .with_label("GPU Unpack BG")
            .with_layout(&unpack_layout)
            .with_entry(brickmap_manager.get_worldstate_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickgrid_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickmap_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_shading_buffer().as_entire_binding())
            .with_entry(
                brickmap_manager
                    .get_brickmap_unpack_buffer()
                    .as_entire_binding(),
            )
            .with_entry(
                brickmap_manager
                    .get_brickgrid_unpack_buffer()
                    .as_entire_binding(),
            )
            .build(context);
        let unpack_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("GPU Unpack Pipeline"),
                    layout: Some(&context.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("GPU Unpack PL"),
                            bind_group_layouts: &[&unpack_layout],
                            push_constant_ranges: &[],
                        },
                    )),
                    module: &cs,
                    entry_point: "compute",
                });

        let cs_descriptor = wgpu::include_wgsl!("../../assets/shaders/voxel_volume.wgsl");
        let cs = context.device.create_shader_module(cs_descriptor);
        let raycast_layout = render::BindGroupLayoutBuilder::new()
            .with_label("Voxel Raycast BGL")
            .with_entry(
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: render_texture.attributes.format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                None,
            )
            .with_uniform_entry(wgpu::ShaderStages::COMPUTE)
            .with_rw_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_ro_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_ro_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_rw_storage_entry(wgpu::ShaderStages::COMPUTE)
            .with_uniform_entry(wgpu::ShaderStages::COMPUTE)
            .build(context);
        let raycast_bind_group = render::BindGroupBuilder::new()
            .with_label("Voxel Raycast BG")
            .with_layout(&raycast_layout)
            .with_entry(wgpu::BindingResource::TextureView(&render_texture.view))
            .with_entry(brickmap_manager.get_worldstate_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickgrid_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_brickmap_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_shading_buffer().as_entire_binding())
            .with_entry(brickmap_manager.get_feedback_buffer().as_entire_binding())
            .with_entry(camera_controller.get_buffer().as_entire_binding())
            .build(context);
        let raycast_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Voxel Raycast Pipeline"),
                    layout: Some(&context.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("Voxel Raycast PL"),
                            bind_group_layouts: &[&raycast_layout],
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
            raycast_pipeline,
            raycast_bind_group,
            unpack_pipeline,
            unpack_bind_group,
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
        compute_pass.set_pipeline(&self.raycast_pipeline);
        compute_pass.set_bind_group(0, &self.raycast_bind_group, &[]);
        compute_pass.dispatch_workgroups(size.width / 8, size.height / 8, 1);
        drop(compute_pass);

        let unpack_max_count = self.brickmap_manager.get_unpack_max_count() as u32;
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&self.unpack_pipeline);
        compute_pass.set_bind_group(0, &self.unpack_bind_group, &[]);
        compute_pass.dispatch_workgroups(unpack_max_count / 8, 1, 1);
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

        encoder.copy_buffer_to_buffer(
            self.brickmap_manager.get_feedback_buffer(),
            0,
            self.brickmap_manager.get_feedback_result_buffer(),
            0,
            self.brickmap_manager.get_feedback_result_buffer().size(),
        );

        context.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn update(&mut self, _dt: &Duration, _context: &render::Context) {}
}

impl VoxelRenderer {
    pub fn update_brickmap(
        &mut self,
        context: &render::Context,
        world: &mut super::world::WorldManager,
    ) {
        self.brickmap_manager
            .process_feedback_buffer(context, world);
    }
}
