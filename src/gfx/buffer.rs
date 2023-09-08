use bytemuck::NoUninit;
use wgpu::util::DeviceExt;

use super::Context;

#[derive(Debug)]
pub struct BulkBufferBuilder<'a> {
    descriptors: Vec<wgpu::util::BufferInitDescriptor<'a>>,
    current_usage: wgpu::BufferUsages,
}

impl<'a> BulkBufferBuilder<'a> {
    pub fn new() -> Self {
        Self {
            descriptors: vec![],
            current_usage: wgpu::BufferUsages::UNIFORM,
        }
    }

    pub fn set_usage(mut self, usage: wgpu::BufferUsages) -> Self {
        self.current_usage = usage;
        self
    }

    pub fn with_buffer(mut self, label: &'a str, contents: &'a [u8]) -> Self {
        let descriptor = wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: self.current_usage,
        };

        self.descriptors.push(descriptor);
        self
    }

    pub fn with_bytemuck_buffer<A: NoUninit>(self, label: &'a str, contents: &'a [A]) -> Self {
        self.with_buffer(label, bytemuck::cast_slice(contents))
    }

    pub fn build(self, context: &Context) -> Vec<wgpu::Buffer> {
        let mut buffers = vec![];
        for descriptor in self.descriptors {
            buffers.push(context.device.create_buffer_init(&descriptor));
        }
        buffers
    }
}
