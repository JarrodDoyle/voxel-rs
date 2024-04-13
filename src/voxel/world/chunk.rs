use ndarray::{s, Array3};
use wgpu::naga::FastHashSet;

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
    genned_blocks: FastHashSet<(usize, usize, usize)>,
    noise: Vec<f32>,
    blocks: Array3<Voxel>,
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

        let genned_blocks = FastHashSet::default();

        let shape = chunk_settings.dimensions * chunk_settings.block_dimensions;
        let num_voxels = shape.x * shape.y * shape.z;
        let blocks = Array3::from_shape_vec(
            (shape.x as usize, shape.y as usize, shape.z as usize),
            vec![Voxel::Empty; num_voxels as usize],
        )
        .unwrap();

        Self {
            settings: chunk_settings,
            noise,
            blocks,
            genned_blocks,
        }
    }

    pub fn get_region(
        &mut self,
        region_start: glam::UVec3,
        region_dims: glam::UVec3,
    ) -> Vec<Voxel> {
        let start = region_start;
        let end = region_start + region_dims;
        let dims = self.settings.dimensions * self.settings.block_dimensions;
        assert!(end.x <= dims.x && end.y <= dims.y && end.z <= dims.z);

        // Check that all the blocks needed are generated and generated them if needed
        // TODO: Don't hardcode this division!!
        let start_block = start / 8;
        let end_block = end / 8;
        for z in start_block.z..(end_block.z) {
            for y in (start_block.y)..(end_block.y) {
                for x in (start_block.x)..(end_block.x) {
                    if !self
                        .genned_blocks
                        .contains(&(x as usize, y as usize, z as usize))
                    {
                        self.gen_block(glam::uvec3(x, y, z));
                    }
                }
            }
        }

        //
        let region = self
            .blocks
            .slice(s![
                (start.x as usize)..(end.x as usize),
                (start.y as usize)..(end.y as usize),
                (start.z as usize)..(end.z as usize)
            ])
            .to_owned()
            .into_raw_vec();
        // dbg!(&region);
        region
    }

    // pub fn get_voxel(&mut self, pos: glam::UVec3) -> Voxel {
    //     let dims = self.settings.dimensions * self.settings.block_dimensions;
    //     debug_assert!(pos.x < dims.x && pos.y < dims.y && pos.z < dims.z);

    //     let block_pos = pos / self.settings.block_dimensions;
    //     let block_idx = math::to_1d_index(block_pos, self.settings.dimensions);
    //     let mut block = &self.blocks[block_idx];
    //     if block.is_empty() {
    //         self.gen_block(block_pos, block_idx);
    //         block = &self.blocks[block_idx]
    //     }

    //     let local_pos = pos % self.settings.block_dimensions;
    //     let local_idx = math::to_1d_index(local_pos, self.settings.block_dimensions);
    //     block[local_idx]
    // }

    pub fn get_block(&mut self, pos: glam::UVec3) -> Vec<Voxel> {
        let dims = self.settings.dimensions;
        assert!(pos.x < dims.x && pos.y < dims.y && pos.z < dims.z);

        let gen_key = &(pos.x as usize, pos.y as usize, pos.z as usize);
        if !self.genned_blocks.contains(gen_key) {
            self.gen_block(pos);
        }

        let block_dims = self.settings.block_dimensions;
        let start = pos * block_dims;
        let end = start + block_dims;
        let region = self
            .blocks
            .slice(s![
                (start.x as usize)..(end.x as usize),
                (start.y as usize)..(end.y as usize),
                (start.z as usize)..(end.z as usize)
            ])
            .to_owned()
            .into_raw_vec();
        region
    }

    pub fn gen_block(&mut self, block_pos: glam::UVec3) {
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
        // will be negative too. The chunk voxels are initialised as empty already
        // so we only need to modify them if we have at least one positive corner
        if block_sign != -8.0 {
            let mut vals = [0.0f32; 512];
            math::tri_lerp_block(&noise_vals, &[8, 8, 8], &mut vals);

            let block_dims = self.settings.block_dimensions;
            let start = block_pos * block_dims;
            let end = start + block_dims;
            let mut block = self.blocks.slice_mut(s![
                (start.x as usize)..(end.x as usize),
                (start.y as usize)..(end.y as usize),
                (start.z as usize)..(end.z as usize)
            ]);

            // TODO: Better voxel colours
            let mut val_idx = 0;
            for z in 0..block_dims.z {
                for y in 0..block_dims.y {
                    for x in 0..block_dims.x {
                        let val = vals[val_idx];
                        val_idx += 1;

                        if val > 0.0 {
                            let r = ((x + 1) * 32 - 1) as u8;
                            let g = ((y + 1) * 32 - 1) as u8;
                            let b = ((z + 1) * 32 - 1) as u8;
                            let block_idx = [z as usize, y as usize, x as usize];
                            block[block_idx] = Voxel::Color(r, g, b);
                        }
                    }
                }
            }
        }

        let key = (
            block_pos.x as usize,
            block_pos.y as usize,
            block_pos.z as usize,
        );
        self.genned_blocks.insert(key);
    }
}
