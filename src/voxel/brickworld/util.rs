use crate::voxel::world::{Voxel, WorldManager};

use super::brickmap::BrickgridFlag;

pub fn cull_interior_voxels(
    world: &mut WorldManager,
    grid_pos: glam::IVec3,
) -> ([u32; 16], Vec<u32>) {
    // This is the data we want to return
    let mut bitmask_data = [0xFFFFFFFF_u32; 16];
    let mut albedo_data = Vec::<u32>::new();

    // Calculate world chunk and block positions for each that may be accessed
    let center_pos = grid_pos_to_world_pos(world, grid_pos);
    let forward_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(1, 0, 0));
    let backward_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(-1, 0, 0));
    let left_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 0, -1));
    let right_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 0, 1));
    let up_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 1, 0));
    let down_pos = grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, -1, 0));

    // Fetch those blocks
    let center_block = world.get_block(center_pos.0, center_pos.1);
    let forward_block = world.get_block(forward_pos.0, forward_pos.1);
    let backward_block = world.get_block(backward_pos.0, backward_pos.1);
    let left_block = world.get_block(left_pos.0, left_pos.1);
    let right_block = world.get_block(right_pos.0, right_pos.1);
    let up_block = world.get_block(up_pos.0, up_pos.1);
    let down_block = world.get_block(down_pos.0, down_pos.1);

    //  Reusable array of whether cardinal neighbours are empty
    let mut neighbours = [false; 6];
    for z in 0..8 {
        // Each z level contains two bitmask segments of voxels
        let mut entry = 0u64;
        for y in 0..8 {
            for x in 0..8 {
                // Ignore non-solids
                let idx = x + y * 8 + z * 8 * 8;
                let empty_voxel = Voxel::Empty;

                match center_block[idx] {
                    Voxel::Empty => continue,
                    Voxel::Color(r, g, b) => {
                        // A voxel is on the surface if at least one of it's
                        // cardinal neighbours is non-solid.
                        neighbours[0] = if x == 7 {
                            forward_block[idx - 7] == empty_voxel
                        } else {
                            center_block[idx + 1] == empty_voxel
                        };

                        neighbours[1] = if x == 0 {
                            backward_block[idx + 7] == empty_voxel
                        } else {
                            center_block[idx - 1] == empty_voxel
                        };

                        neighbours[2] = if z == 7 {
                            right_block[idx - 448] == empty_voxel
                        } else {
                            center_block[idx + 64] == empty_voxel
                        };

                        neighbours[3] = if z == 0 {
                            left_block[idx + 448] == empty_voxel
                        } else {
                            center_block[idx - 64] == empty_voxel
                        };

                        neighbours[4] = if y == 7 {
                            up_block[idx - 56] == empty_voxel
                        } else {
                            center_block[idx + 8] == empty_voxel
                        };

                        neighbours[5] = if y == 0 {
                            down_block[idx + 56] == empty_voxel
                        } else {
                            center_block[idx - 8] == empty_voxel
                        };

                        // Set the appropriate bit in the z entry and add the
                        // shading data
                        let surface_voxel = neighbours.iter().any(|v| *v);
                        if surface_voxel {
                            entry += 1 << (x + y * 8);
                            let albedo = ((r as u32) << 24)
                                + ((g as u32) << 16)
                                + ((b as u32) << 8)
                                + 255u32;
                            albedo_data.push(albedo);
                        }
                    }
                }
            }
        }
        let offset = 2 * z;
        bitmask_data[offset] = (entry & 0xFFFFFFFF).try_into().unwrap();
        bitmask_data[offset + 1] = ((entry >> 32) & 0xFFFFFFFF).try_into().unwrap();
    }

    (bitmask_data, albedo_data)
}

pub fn to_brickgrid_element(brickmap_cache_idx: u32, flags: BrickgridFlag) -> u32 {
    (brickmap_cache_idx << 8) + flags as u32
}

pub fn grid_pos_to_world_pos(
    world: &mut WorldManager,
    grid_pos: glam::IVec3,
) -> (glam::IVec3, glam::UVec3) {
    // We deal with dvecs here because we want a negative grid_pos to have floored
    // chunk_pos
    let chunk_dims = world.get_chunk_dims().as_dvec3();
    let chunk_pos = (grid_pos.as_dvec3() / chunk_dims).floor();
    let block_pos = grid_pos - (chunk_pos * chunk_dims).as_ivec3();
    (chunk_pos.as_ivec3(), block_pos.as_uvec3())
}
