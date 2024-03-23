use std::ops::RangeBounds;

use bytemuck::NoUninit;
use wgpu::util::DeviceExt;

use super::Context;

#[derive(Debug)]
pub struct BulkBufferBuilder<'a> {
    order: Vec<(bool, usize)>,
    init_descriptors: Vec<wgpu::util::BufferInitDescriptor<'a>>,
    descriptors: Vec<wgpu::BufferDescriptor<'a>>,
    current_usage: wgpu::BufferUsages,
}

impl<'a> BulkBufferBuilder<'a> {
    pub fn new() -> Self {
        Self {
            order: vec![],
            init_descriptors: vec![],
            descriptors: vec![],
            current_usage: wgpu::BufferUsages::UNIFORM,
        }
    }

    pub fn set_usage(mut self, usage: wgpu::BufferUsages) -> Self {
        self.current_usage = usage;
        self
    }

    pub fn with_buffer(mut self, label: &'a str, size: u64, mapped: bool) -> Self {
        let descriptor = wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: self.current_usage,
            mapped_at_creation: mapped,
        };

        self.order.push((false, self.descriptors.len()));
        self.descriptors.push(descriptor);
        self
    }

    pub fn with_init_buffer(mut self, label: &'a str, contents: &'a [u8]) -> Self {
        let descriptor = wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: self.current_usage,
        };

        self.order.push((true, self.init_descriptors.len()));
        self.init_descriptors.push(descriptor);
        self
    }

    pub fn with_init_buffer_bm<A: NoUninit>(self, label: &'a str, contents: &'a [A]) -> Self {
        self.with_init_buffer(label, bytemuck::cast_slice(contents))
    }

    pub fn build(self, context: &Context) -> Vec<wgpu::Buffer> {
        let device = &context.device;
        let mut buffers = vec![];
        for (init, index) in self.order {
            let buffer = if init {
                device.create_buffer_init(&(self.init_descriptors[index]))
            } else {
                device.create_buffer(&(self.descriptors[index]))
            };

            buffers.push(buffer);
        }

        buffers
    }
}

pub trait BufferExt {
    fn get_mapped_range<S: RangeBounds<wgpu::BufferAddress>, T: bytemuck::Pod>(
        &self,
        context: &Context,
        bounds: S,
    ) -> Vec<T>;
}

impl BufferExt for wgpu::Buffer {
    fn get_mapped_range<S: RangeBounds<wgpu::BufferAddress>, T: bytemuck::Pod>(
        &self,
        context: &Context,
        bounds: S,
    ) -> Vec<T> {
        let slice = self.slice(bounds);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        context.device.poll(wgpu::Maintain::Wait);
        let data: Vec<T> = bytemuck::cast_slice(slice.get_mapped_range().as_ref()).to_vec();
        self.unmap();

        data
    }
}
