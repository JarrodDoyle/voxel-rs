use crate::math;

use super::Voxel;

#[derive(Debug)]
pub struct Chunk {
    pos: glam::IVec3,
    noise: Vec<f32>,
    blocks: Vec<Vec<Voxel>>,
}

impl Chunk {
    pub fn new(pos: glam::IVec3, noise: Vec<f32>, blocks: Vec<Vec<Voxel>>) -> Self {
        Self { pos, noise, blocks }
    }

    pub fn get_block(&mut self, block_pos: glam::UVec3, chunk_dims: glam::UVec3) -> Vec<Voxel> {
        assert_eq!(
            self.blocks.len(),
            (chunk_dims.x * chunk_dims.y * chunk_dims.z) as usize
        );

        let block_idx = math::to_1d_index(block_pos, chunk_dims);
        let mut block = &self.blocks[block_idx];
        if block.is_empty() {
            self.gen_block(block_pos, block_idx, chunk_dims);
            block = &self.blocks[block_idx]
        }

        block.to_owned()
    }

    pub fn gen_block(&mut self, block_pos: glam::UVec3, block_idx: usize, chunk_dims: glam::UVec3) {
        let block = &mut self.blocks[block_idx];
        let noise_dims = chunk_dims + glam::uvec3(1, 1, 1);

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
