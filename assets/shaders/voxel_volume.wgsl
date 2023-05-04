@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> world_state: WorldState;
@group(0) @binding(2) var<storage, read_write> brickgrid: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read> brickmap_cache: array<Brickmap>;
@group(0) @binding(4) var<storage, read> shading_table: array<ShadingElement>;
@group(0) @binding(5) var<storage, read_write> cpu_feedback: Feedback;
@group(0) @binding(6) var<uniform> camera: Camera;

struct ShadingElement {
    albedo: u32,
}

struct Brickmap {
    bitmask: array<u32, 16>,
    shading_table_offset: u32,
    lod_color: u32,
}

struct Camera {
    projection: mat4x4<f32>,
    view: mat4x4<f32>,
    pos: vec3<f32>,
    _pad: f32,
};

struct WorldState {
    brickmap_cache_dims: vec3<u32>,
    _pad: u32,
};

struct HitInfo {
    hit: bool,
    hit_pos: vec3<i32>,
    brickmap_idx: u32,
    mask: vec3<bool>,
};

struct AabbHitInfo {
    hit: bool,
    distance: f32,
};

struct Feedback {
    max_count: u32,
    count: atomic<u32>,
    _pad1: u32,
    _pad2: u32,
    positions: array<vec4<i32>>,
}

// struct UnpackElement {
//     pos: vec3<i32>,
//     cache_idx: u32,
//     map: Brickmap,
// }

// struct Unpack {
//     max_count: u32,
//     count: u32,
//     maps: array<UnpackElement>
// }

// Utility function. Converts a position in 3d to a 1d index.
fn to_1d_index(p: vec3<i32>, dims: vec3<i32>) -> u32 {
    return u32(p.x + p.y * dims.x + p.z * dims.x * dims.y);
}

fn get_shading_offset(hit: HitInfo) -> u32 {
    let brickmap = &brickmap_cache[hit.brickmap_idx];
    let local_index = to_1d_index(hit.hit_pos % 8, vec3<i32>(8));
    let bitmask_index = local_index / 32u;
    var map_voxel_idx = 0u;
    for (var i: i32 = 0; i < i32(bitmask_index); i++) {
        map_voxel_idx += countOneBits((*brickmap).bitmask[i]);
    }
    let extracted_bits = extractBits((*brickmap).bitmask[bitmask_index], 0u, (local_index % 32u));
    map_voxel_idx += countOneBits(extracted_bits);
    return (*brickmap).shading_table_offset + map_voxel_idx;
}

fn ray_intersect_aabb(
    ray_pos: vec3<f32>,
    ray_dir: vec3<f32>,
    min: vec3<f32>,
    max: vec3<f32>
) -> AabbHitInfo {
    let ray_dir_inv = 1.0 / ray_dir;
    let t1 = (min - ray_pos) * ray_dir_inv;
    let t2 = (max - ray_pos) * ray_dir_inv;
    let t_min = min(t1, t2);
    let t_max = max(t1, t2);
    let tmin = max(max(t_min.x, 0.0), max(t_min.y, t_min.z));
    let tmax = min(t_max.x, min(t_max.y, t_max.z));
    return AabbHitInfo(tmax > tmin, tmin);
}

fn point_inside_aabb(p: vec3<i32>, min: vec3<i32>, max: vec3<i32>) -> bool {
    let clamped = clamp(p, min, max - vec3<i32>(1));
    return clamped.x == p.x && clamped.y == p.y && clamped.z == p.z;
}

fn voxel_hit(brickmap_idx: u32, p: vec3<i32>) -> bool {
    let local_index = to_1d_index(p % 8, vec3<i32>(8));
    let bitmask_segment = brickmap_cache[brickmap_idx].bitmask[local_index / 32u];
    return (bitmask_segment >> (local_index % 32u) & 1u) != 0u;
}

fn brick_ray_cast(
    chunk_pos: vec3<i32>,
    brickmap_idx: u32,
    orig_ray_pos: vec3<f32>,
    ray_dir: vec3<f32>
) -> HitInfo {
    var hit_info = HitInfo(false, vec3<i32>(0), 0u, vec3<bool>(false));

    var ray_pos = orig_ray_pos * 8.0;

    let min = vec3<f32>(chunk_pos * 8);
    let max = min + vec3<f32>(8.0);
    let aabbHit = ray_intersect_aabb(ray_pos, ray_dir, min, max);
    var tmin = aabbHit.distance;

    if (aabbHit.hit) {
        // Accelerate ray
        if (tmin > 0.0) {
            ray_pos += ray_dir * (tmin + 0.0001);
        }
        tmin = max(0.0, tmin);

        // DDA setup
        let delta_dist = abs(length(ray_dir) / ray_dir);
        let ray_step = vec3<i32>(sign(ray_dir));
        var map_pos = vec3<i32>(floor(ray_pos));
        var side_dist = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;
        map_pos = map_pos % 8;

        let max_brick_depth = 8 + 8 + 8;
        for (var i: i32 = 0; i < max_brick_depth; i++) {
            if (!point_inside_aabb(map_pos, vec3<i32>(0), vec3<i32>(8))) {
                break;
            }

            if (voxel_hit(brickmap_idx, map_pos)){
                hit_info.hit = true;
                hit_info.hit_pos = map_pos;
                hit_info.brickmap_idx = brickmap_idx;
                break;
            }

            let smallest = min(side_dist.x, min(side_dist.y, side_dist.z));
            if (smallest == side_dist.x) {
                hit_info.mask = vec3<bool>(true, false, false);
            }
            else if (smallest == side_dist.y) {
                hit_info.mask = vec3<bool>(false, true, false);
            }
            else {
                hit_info.mask = vec3<bool>(false, false, true);
            }

            side_dist += vec3<f32>(hit_info.mask) * delta_dist;
            map_pos += vec3<i32>(hit_info.mask) * ray_step;
        }
    }

    return hit_info;
}

fn grid_cast_ray(orig_ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> HitInfo {
    var hit_info = HitInfo(false, vec3<i32>(0), 0u, vec3<bool>(false));

    let min = vec3<f32>(0.0);
    let max = min + vec3<f32>(world_state.brickmap_cache_dims);
    let aabbHit = ray_intersect_aabb(orig_ray_pos, ray_dir, min, max);
    var ray_pos = orig_ray_pos;
    var tmin = aabbHit.distance;
    if (aabbHit.hit) {
        // Accelerate ray
        if (tmin > 0.0) {
            ray_pos += ray_dir * (tmin + 0.0001);
        }
        tmin = max(0.0, tmin);

        // Convert ray_pos into chunk scale
        // ray_pos /= 8.0;

        // DDA setup
        let delta_dist = abs(length(ray_dir) / ray_dir);
        let ray_step = vec3<i32>(sign(ray_dir));
        var map_pos = vec3<i32>(floor(ray_pos));
        var side_dist = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;

        let dims = world_state.brickmap_cache_dims;
        let max_grid_depth = i32(dims.x + dims.y + dims.z);
        for (var i: i32 = 0; i < max_grid_depth; i++) {
            if (!point_inside_aabb(map_pos, vec3<i32>(0), vec3<i32>(world_state.brickmap_cache_dims))) {
                break;
            }

            let grid_idx = to_1d_index(map_pos, vec3<i32>(world_state.brickmap_cache_dims));
            let brick_ptr = brickgrid[grid_idx];
            
            // Ptr = 28 bits LOD colour / brickmap index + 4 bits load flags
            // Flags:
            // 0 = empty
            // 1 = unloaded
            // 2 = loading
            // 4 = loaded
            let flags = brick_ptr & 0xFu;
            if flags == 1u {
                // Add to the load queue
                if (atomicLoad(&cpu_feedback.count) < cpu_feedback.max_count) {
                    if ((atomicOr(&brickgrid[grid_idx], 2u) & 0x2u) == 0u) {
                        let index = atomicAdd(&cpu_feedback.count, 1u);
                        if (index < cpu_feedback.max_count) {
                            cpu_feedback.positions[index] = vec4<i32>(map_pos, 0);
                        }
                        else {
                            atomicSub(&cpu_feedback.count, 1u);
                            atomicXor(&brickgrid[grid_idx], 2u);
                        }
                    }
                }

                // Set hit info stuff?
                break;
            }
            else if flags == 4u {
                let brickmap_idx = brick_ptr >> 8u;
                let tmp_voxel_hit = brick_ray_cast(map_pos, brickmap_idx, orig_ray_pos, ray_dir);

                if (tmp_voxel_hit.hit == true){
                    hit_info.hit = tmp_voxel_hit.hit;
                    hit_info.hit_pos = tmp_voxel_hit.hit_pos + (map_pos * 8);
                    hit_info.mask = tmp_voxel_hit.mask;
                    break;
                }
            }

            let smallest = min(side_dist.x, min(side_dist.y, side_dist.z));
            if (smallest == side_dist.x) {
                hit_info.mask = vec3<bool>(true, false, false);
            }
            else if (smallest == side_dist.y) {
                hit_info.mask = vec3<bool>(false, true, false);
            }
            else {
                hit_info.mask = vec3<bool>(false, false, true);
            }

            side_dist += vec3<f32>(hit_info.mask) * delta_dist;
            map_pos += vec3<i32>(hit_info.mask) * ray_step;
        }
    }

    return hit_info;
}

@compute @workgroup_size(8, 8, 1)
fn compute(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let img_coord = vec2<i32>(global_id.xy);
    let img_dims = textureDimensions(output);

    // This discards the extra pixels in cases where the image size isn't perfectly divisible by the kernel.xy
    if (img_coord.x >= img_dims.x || img_coord.y >= img_dims.y) {
        return;
    }

    // Construct ray
    let img_coord_frac = vec2<f32>(img_coord) / vec2<f32>(img_dims);
    let screen_pos = img_coord_frac * 2.0 - vec2<f32>(1.0);
    var ray_eye = camera.projection * vec4<f32>(screen_pos, -1.0, 0.0);
    ray_eye = vec4<f32>(ray_eye.xy, -1.0, 0.0);
    let ray_dir = normalize((camera.view * ray_eye).xyz);
    let ray_pos = camera.pos;

    // Cast the ray
    var hit_info = grid_cast_ray(ray_pos, ray_dir);
    var color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (hit_info.hit){
        // if (hit_info.mask.x) {
        //     color.x = 1.0;
        // }
        // else if (hit_info.mask.y) {
        //     color.y = 1.0;
        // }
        // else if (hit_info.mask.z) {
        //     color.z = 1.0;
        // }
        // else {
        //     color = vec4<f32>(1.0);
        // }
        let offset = get_shading_offset(hit_info);
        let raw_color = shading_table[offset].albedo;
        color.x = f32((raw_color >> 24u) & 255u) / 255.0;
        color.y = f32((raw_color >> 16u) & 255u) / 255.0;
        color.z = f32((raw_color >> 8u) & 255u) / 255.0;
        color.w = f32(raw_color & 255u) / 255.0;
    }

    textureStore(output, img_coord, color);
}