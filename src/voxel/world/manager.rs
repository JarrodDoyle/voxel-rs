use std::collections::HashMap;

use super::{chunk::ChunkSettings, Chunk, GenerationSettings, Voxel};

pub struct WorldManager {
    generation_settings: GenerationSettings,
    chunk_settings: ChunkSettings,
    chunks: HashMap<glam::IVec3, Chunk>,
}

impl WorldManager {
    pub fn new(generation_settings: GenerationSettings, chunk_settings: ChunkSettings) -> Self {
        let chunks = HashMap::new();
        Self {
            generation_settings,
            chunk_settings,
            chunks,
        }
    }

    pub fn get_chunk_dims(&self) -> glam::UVec3 {
        self.chunk_settings.dimensions
    }

    pub fn get_block(&mut self, chunk_pos: glam::IVec3, local_pos: glam::UVec3) -> Vec<Voxel> {
        // There's no world saving yet, so if a chunk isn't currently loaded we need to
        // generate it's base noise values
        if !self.chunks.contains_key(&chunk_pos) {
            let new_chunk = self.gen_chunk(chunk_pos);
            self.chunks.insert(chunk_pos, new_chunk);
        }

        let chunk = self.chunks.get_mut(&chunk_pos).unwrap();
        chunk.get_block(local_pos)
    }

    fn gen_chunk(&mut self, pos: glam::IVec3) -> Chunk {
        Chunk::new(&self.generation_settings, self.chunk_settings, pos)
    }
}
