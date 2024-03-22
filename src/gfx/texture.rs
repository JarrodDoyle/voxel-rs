// TODO: Support mip-mapping and multi-sampling

use super::{BindGroupBuilder, BindGroupLayoutBuilder, Context};

#[derive(Debug, Clone)]
pub struct TextureAttributes {
    pub size: wgpu::Extent3d,
    pub dimension: wgpu::TextureDimension,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub address_mode_w: wgpu::AddressMode,
    pub mag_filter: wgpu::FilterMode,
    pub min_filter: wgpu::FilterMode,
    pub mipmap_filter: wgpu::FilterMode,
    pub shader_visibility: wgpu::ShaderStages,
}

impl Default for TextureAttributes {
    fn default() -> Self {
        Self {
            size: Default::default(),
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            address_mode_u: wgpu::AddressMode::default(),
            address_mode_v: wgpu::AddressMode::default(),
            address_mode_w: wgpu::AddressMode::default(),
            mag_filter: wgpu::FilterMode::default(),
            min_filter: wgpu::FilterMode::default(),
            mipmap_filter: wgpu::FilterMode::default(),
            shader_visibility: wgpu::ShaderStages::FRAGMENT,
        }
    }
}

#[derive(Debug)]
pub struct TextureBuilder {
    pub attributes: TextureAttributes,
}

impl TextureBuilder {
    pub fn new() -> Self {
        Self {
            attributes: Default::default(),
        }
    }

    #[inline]
    pub fn with_size(mut self, width: u32, height: u32, depth: u32) -> Self {
        self.attributes.size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        };
        self
    }

    #[inline]
    pub fn with_dimension(mut self, dimension: wgpu::TextureDimension) -> Self {
        self.attributes.dimension = dimension;
        self
    }

    #[inline]
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.attributes.format = format;
        self
    }

    #[inline]
    pub fn with_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.attributes.usage = usage;
        self
    }

    #[inline]
    pub fn with_address_mode(mut self, address_mode: wgpu::AddressMode) -> Self {
        self.attributes.address_mode_u = address_mode;
        self.attributes.address_mode_v = address_mode;
        self.attributes.address_mode_w = address_mode;
        self
    }

    #[inline]
    pub fn with_filter_mode(mut self, filter_mode: wgpu::FilterMode) -> Self {
        self.attributes.mag_filter = filter_mode;
        self.attributes.min_filter = filter_mode;
        self.attributes.mipmap_filter = filter_mode;
        self
    }

    #[inline]
    pub fn with_shader_visibility(mut self, visibility: wgpu::ShaderStages) -> Self {
        self.attributes.shader_visibility = visibility;
        self
    }

    #[inline]
    pub fn build(self, context: &Context) -> Texture {
        Texture::new(context, self.attributes)
    }
}

#[derive(Debug)]
pub struct Texture {
    pub attributes: TextureAttributes,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl Texture {
    pub fn new(context: &Context, attributes: TextureAttributes) -> Self {
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: attributes.size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: attributes.dimension,
            format: attributes.format,
            usage: attributes.usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: attributes.address_mode_u,
            address_mode_v: attributes.address_mode_v,
            address_mode_w: attributes.address_mode_w,
            mag_filter: attributes.mag_filter,
            min_filter: attributes.min_filter,
            mipmap_filter: attributes.mipmap_filter,
            ..Default::default()
        });

        let view_dimension = match attributes.dimension {
            wgpu::TextureDimension::D1 => wgpu::TextureViewDimension::D1,
            wgpu::TextureDimension::D2 => wgpu::TextureViewDimension::D2,
            wgpu::TextureDimension::D3 => wgpu::TextureViewDimension::D3,
        };

        let bind_group_layout = BindGroupLayoutBuilder::new()
            .with_entry(
                attributes.shader_visibility,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension,
                    multisampled: false,
                },
                None,
            )
            .with_entry(
                attributes.shader_visibility,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                None,
            )
            .build(context);
        let bind_group = BindGroupBuilder::new()
            .with_layout(&bind_group_layout)
            .with_entry(wgpu::BindingResource::TextureView(&view))
            .with_entry(wgpu::BindingResource::Sampler(&sampler))
            .build(context);

        Self {
            attributes,
            texture,
            view,
            sampler,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn update(&self, context: &Context, data: &[u8]) {
        log::info!("Updating texture contents...");
        let copy_texture = wgpu::ImageCopyTexture {
            texture: &self.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };

        let size = self.attributes.size;
        let image_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * size.width),
            rows_per_image: Some(size.height),
        };

        context
            .queue
            .write_texture(copy_texture, data, image_layout, size);
    }
}
