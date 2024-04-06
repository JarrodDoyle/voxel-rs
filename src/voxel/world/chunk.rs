use crate::math;

use super::{GenerationSettings, Voxel};

#[derive(Debug, Clone, Copy)]
pub struct ChunkSettings {
    pub dimensions: glam::UVec3,
    pub block_dimensions: glam::UVec3,
}

#[derive(Debug)]
pub struct Chunk {
    settings: ChunkSettings,
    noise: Vec<f32>,
    blocks: Vec<Vec<Voxel>>,
}

impl Chunk {
    pub fn new(
        generation_settings: &GenerationSettings,
        chunk_settings: ChunkSettings,
        pos: glam::IVec3,
    ) -> Self {
        let dims = chunk_settings.dimensions;

        // We use dimensions of `chunk_dims + 1` because the corners on the last chunk
        // block of each axis step outside of our 0..N bounds, sharing a value with the
        // neighbouring chunk
        let noise = simdnoise::NoiseBuilder::fbm_3d_offset(
            pos.x as f32 * dims.x as f32,
            dims.x as usize + 1,
            pos.y as f32 * dims.y as f32,
            dims.y as usize + 1,
            pos.z as f32 * dims.z as f32,
            dims.z as usize + 1,
        )
        .with_seed(generation_settings.seed)
        .with_freq(generation_settings.frequency)
        .with_octaves(generation_settings.octaves)
        .with_gain(generation_settings.gain)
        .with_lacunarity(generation_settings.lacunarity)
        .generate()
        .0;

        let num_blocks = dims.x * dims.y * dims.z;
        let blocks = vec![vec![]; num_blocks as usize];

        Self {
            settings: chunk_settings,
            noise,
            blocks,
        }
    }

    pub fn get_block(&mut self, pos: glam::UVec3) -> Vec<Voxel> {
        let dims = self.settings.dimensions;
        assert!(pos.x < dims.x && pos.y < dims.y && pos.z < dims.z);

        let block_idx = math::to_1d_index(pos, dims);
        let mut block = &self.blocks[block_idx];
        if block.is_empty() {
            self.gen_block(pos, block_idx);
            block = &self.blocks[block_idx]
        }

        block.to_owned()
    }

    pub fn gen_block(&mut self, block_pos: glam::UVec3, block_idx: usize) {
        let block = &mut self.blocks[block_idx];
        let noise_dims = self.settings.dimensions + glam::uvec3(1, 1, 1);

        // Extract relevant noise values from the chunk
        let mut noise_vals = Vec::new();
        let mut block_sign = 0.0;
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    let noise_pos = glam::uvec3(x, y, z) + block_pos;
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
}
