use std::collections::HashMap;

use super::{Chunk, GenerationSettings, Voxel};

pub struct WorldManager {
    settings: GenerationSettings,
    chunk_dims: glam::UVec3,
    chunks: HashMap<glam::IVec3, Chunk>,
}

impl WorldManager {
    pub fn new(settings: GenerationSettings, chunk_dims: glam::UVec3) -> Self {
        let chunks = HashMap::new();
        Self {
            settings,
            chunk_dims,
            chunks,
        }
    }

    pub fn get_chunk_dims(&self) -> glam::UVec3 {
        self.chunk_dims
    }

    pub fn get_block(&mut self, chunk_pos: glam::IVec3, local_pos: glam::UVec3) -> Vec<Voxel> {
        // There's no world saving yet, so if a chunk isn't currently loaded we need to
        // generate it's base noise values
        if !self.chunks.contains_key(&chunk_pos) {
            let new_chunk = self.gen_chunk(chunk_pos);
            self.chunks.insert(chunk_pos, new_chunk);
        }

        let chunk = self.chunks.get_mut(&chunk_pos).unwrap();
        chunk.get_block(local_pos, self.chunk_dims)
    }

    fn gen_chunk(&mut self, pos: glam::IVec3) -> Chunk {
        // We use dimensions of `chunk_dims + 1` because the corners on the last chunk
        // block of each axis step outside of our 0..N bounds, sharing a value with the
        // neighbouring chunk
        let noise = simdnoise::NoiseBuilder::fbm_3d_offset(
            pos.x as f32 * self.chunk_dims.x as f32,
            self.chunk_dims.x as usize + 1,
            pos.y as f32 * self.chunk_dims.y as f32,
            self.chunk_dims.y as usize + 1,
            pos.z as f32 * self.chunk_dims.z as f32,
            self.chunk_dims.z as usize + 1,
        )
        .with_seed(self.settings.seed)
        .with_freq(self.settings.frequency)
        .with_octaves(self.settings.octaves)
        .with_gain(self.settings.gain)
        .with_lacunarity(self.settings.lacunarity)
        .generate()
        .0;

        let num_blocks = self.chunk_dims.x * self.chunk_dims.y * self.chunk_dims.z;
        let blocks = vec![vec![]; num_blocks as usize];
        Chunk::new(pos, noise, blocks)
    }
}
