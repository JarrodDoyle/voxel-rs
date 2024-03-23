use std::num::NonZeroU32;

use anyhow::{Context as _, Result};

use super::Context;

#[derive(Debug, Default)]
pub struct BindGroupLayoutBuilder<'a> {
    next_binding: u32,
    entries: Vec<wgpu::BindGroupLayoutEntry>,
    label: Option<&'a str>,
}

impl<'a> BindGroupLayoutBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    #[inline]
    pub fn with_entry(
        mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::BindingType,
        count: Option<NonZeroU32>,
    ) -> Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.next_binding,
            visibility,
            ty,
            count,
        });
        self.next_binding += 1;
        self
    }

    #[inline]
    pub fn with_uniform_entry(self, visibility: wgpu::ShaderStages) -> Self {
        self.with_entry(
            visibility,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        )
    }

    #[inline]
    pub fn with_rw_storage_entry(self, visibility: wgpu::ShaderStages) -> Self {
        self.with_entry(
            visibility,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        )
    }

    #[inline]
    pub fn with_ro_storage_entry(self, visibility: wgpu::ShaderStages) -> Self {
        self.with_entry(
            visibility,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        )
    }

    #[inline]
    pub fn build(self, context: &Context) -> wgpu::BindGroupLayout {
        context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: self.label,
                entries: &self.entries,
            })
    }
}

#[derive(Debug, Default)]
pub struct BindGroupBuilder<'a> {
    next_binding: u32,
    label: Option<&'a str>,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
    layout: Option<&'a wgpu::BindGroupLayout>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    #[inline]
    pub fn with_entry(mut self, resource: wgpu::BindingResource<'a>) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.next_binding,
            resource,
        });
        self.next_binding += 1;
        self
    }

    #[inline]
    pub fn with_layout(mut self, layout: &'a wgpu::BindGroupLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    #[inline]
    pub fn build(self, context: &Context) -> Result<wgpu::BindGroup> {
        Ok(context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: self.label,
                layout: self.layout.context("BindGroupBuilder has no layout.")?,
                entries: self.entries.as_slice(),
            }))
    }
}
