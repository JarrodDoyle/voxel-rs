mod chunk;
mod manager;

pub use {chunk::Chunk, manager::*};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Voxel {
    Empty,
    Color(u8, u8, u8),
}

#[derive(Debug, Clone, Copy)]
pub struct GenerationSettings {
    pub seed: i32,
    pub frequency: f32,
    pub octaves: u8,
    pub gain: f32,
    pub lacunarity: f32,
}
