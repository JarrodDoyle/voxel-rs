use crate::math;
use std::collections::HashMap;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Voxel {
    Empty,
    Color(u8, u8, u8),
}

#[derive(Debug)]
pub struct Chunk {
    pos: glam::IVec3,
    noise: Vec<f32>,
    blocks: Vec<Vec<Voxel>>,
}

impl Chunk {
    fn get_block(&mut self, pos: glam::UVec3, chunk_dims: glam::UVec3) -> Vec<Voxel> {
        let noise_dims = chunk_dims + glam::uvec3(1, 1, 1);
        let block_idx = math::to_1d_index(pos, chunk_dims);
        assert_eq!(
            self.blocks.len(),
            (chunk_dims.x * chunk_dims.y * chunk_dims.z) as usize
        );
        let block = &mut self.blocks[block_idx];

        if block.is_empty() {
            // Extract relevant noise values from the chunk
            let mut noise_vals = Vec::new();

            let mut block_sign = 0.0;
            for z in 0..2 {
                for y in 0..2 {
                    for x in 0..2 {
                        let noise_pos = glam::uvec3(x, y, z) + pos;
                        let noise_idx = math::to_1d_index(noise_pos, noise_dims);
                        let val = self.noise[noise_idx];
                        noise_vals.push(val);
                        block_sign += val.signum();
                    }
                }
            }

            // If all the corners are negative, then all the interpolated values
            // will be negative too. In that case we can just fill with empty.
            if block_sign == -8.0 {
                block.resize(512, Voxel::Empty);
            } else {
                let mut vals = [0.0f32; 512];
                math::tri_lerp_block(&noise_vals, &[8, 8, 8], &mut vals);

                // TODO: Better voxel colours
                let mut idx = 0;
                for z in 0..8 {
                    for y in 0..8 {
                        for x in 0..8 {
                            let val = vals[idx];
                            idx += 1;

                            if val > 0.0 {
                                let r = ((x + 1) * 32 - 1) as u8;
                                let g = ((y + 1) * 32 - 1) as u8;
                                let b = ((z + 1) * 32 - 1) as u8;
                                block.push(Voxel::Color(r, g, b));
                            } else {
                                block.push(Voxel::Empty);
                            }
                        }
                    }
                }
            }
        }

        block.to_owned()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GenerationSettings {
    pub seed: i32,
    pub frequency: f32,
    pub octaves: u8,
    pub gain: f32,
    pub lacunarity: f32,
}

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
        Chunk { pos, noise, blocks }
    }
}
