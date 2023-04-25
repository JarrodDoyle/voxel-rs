@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> world_state: WorldState;
@group(0) @binding(2) var<storage, read> brickmap_cache: array<Brickmap>;
@group(0) @binding(3) var<storage, read> shading_table: array<ShadingElement>;
@group(0) @binding(4) var<uniform> camera: Camera;

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
    mask: vec3<bool>,
};

struct AabbHitInfo {
    hit: bool,
    distance: f32,
};

fn get_shading_offset(p: vec3<i32>) -> u32 {
    // return 0u;

    // What brickmap are we in?
    let brickmap_index = (p.x / 8) + (p.y / 8) * 32 + (p.z / 8) * 1024;
    let local_index = u32((p.x % 8) + (p.y % 8) * 8 + (p.z % 8) * 64);
    let brickmap = &brickmap_cache[brickmap_index];
    let bitmask_index = local_index / 32u;
    var map_voxel_idx = 0u;
    for (var i: i32 = 0; i < i32(bitmask_index); i++) {
        map_voxel_idx += countOneBits((*brickmap).bitmask[i]);
    }
    let extracted_bits = extractBits((*brickmap).bitmask[bitmask_index], 0u, (local_index % 32u));
    map_voxel_idx += countOneBits(extracted_bits);
    return (*brickmap).shading_table_offset + map_voxel_idx;


    // let local_index = u32(p.x + p.y * 8 + p.z * 64);
    // let bitmask_index = local_index / 32u;
    // var map_voxel_idx = 0u;
    // for (var i: i32 = 0; i < i32(bitmask_index); i++) {
    //     map_voxel_idx += countOneBits(brickmap.bitmask[i]);
    // }
    // let extracted_bits = extractBits(brickmap.bitmask[bitmask_index], 0u, (local_index % 32u));
    // map_voxel_idx += countOneBits(extracted_bits);
    // return brickmap.shading_table_offset + map_voxel_idx;
}

fn ray_intersect_aabb(ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> AabbHitInfo {
    let ray_dir_inv = 1.0 / ray_dir;
    let t1 = (vec3<f32>(0.0) - ray_pos) * ray_dir_inv;
    let t2 = ((vec3<f32>(8.0) * vec3<f32>(world_state.brickmap_cache_dims)) - ray_pos) * ray_dir_inv;
    let t_min = min(t1, t2);
    let t_max = max(t1, t2);
    let tmin = max(max(t_min.x, 0.0), max(t_min.y, t_min.z));
    let tmax = min(t_max.x, min(t_max.y, t_max.z));
    return AabbHitInfo(tmax > tmin, tmin);
}

fn point_inside_aabb(p: vec3<i32>) -> bool {
    let max = vec3<i32>(world_state.brickmap_cache_dims) * 8;
    let clamped = clamp(p, vec3<i32>(0), max - vec3<i32>(1));
    return clamped.x == p.x && clamped.y == p.y && clamped.z == p.z;
}

fn voxel_hit(p: vec3<i32>) -> bool {
    let brickmap_index = (p.x / 8) + (p.y / 8) * 32 + (p.z / 8) * 1024;
    let local_index = u32((p.x % 8) + (p.y % 8) * 8 + (p.z % 8) * 64);
    return (brickmap_cache[brickmap_index].bitmask[local_index / 32u] >> (local_index % 32u) & 1u) != 0u;
}

fn cast_ray(orig_ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> HitInfo {
    var hit_info = HitInfo(false, vec3<i32>(0), vec3<bool>(false));

    let aabbHit = ray_intersect_aabb(orig_ray_pos, ray_dir);
    var ray_pos = orig_ray_pos;
    var tmin = aabbHit.distance;
    if (aabbHit.hit) {
        // Accelerate ray
        if (tmin > 0.0) {
            ray_pos += ray_dir * (tmin - 0.0001);
        }
        tmin = max(0.0, tmin);

        // DDA setup
        let delta_dist = abs(length(ray_dir) / ray_dir);
        let ray_step = vec3<i32>(sign(ray_dir));
        var map_pos = vec3<i32>(floor(ray_pos));
        var side_dist = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;

        // TODO: don't hardcode max ray depth
        for (var i: i32 = 0; i < 2048; i++) {
            if (side_dist.x < side_dist.y) {
                if (side_dist.x < side_dist.z) {
                    side_dist.x += delta_dist.x;
                    map_pos.x += ray_step.x;
                    hit_info.mask = vec3<bool>(true, false, false);
                }
                else {
                    side_dist.z += delta_dist.z;
                    map_pos.z += ray_step.z;
                    hit_info.mask = vec3<bool>(false, false, true);
                }
            }
            else {
                if (side_dist.y < side_dist.z) {
                    side_dist.y += delta_dist.y;
                    map_pos.y += ray_step.y;
                    hit_info.mask = vec3<bool>(false, true, false);
                }
                else {
                    side_dist.z += delta_dist.z;
                    map_pos.z += ray_step.z;
                    hit_info.mask = vec3<bool>(false, false, true);
                }
            }

            if (!point_inside_aabb(map_pos)) {
                break;
            }

            if (voxel_hit(map_pos)) {
                hit_info.hit = true;
                hit_info.hit_pos = map_pos;
                break;
            }
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
    var hit_info = cast_ray(ray_pos, ray_dir);
    var color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (hit_info.hit){
        if (hit_info.mask.x) {
            color.x = 1.0;
        }
        else if (hit_info.mask.y) {
            color.y = 1.0;
        }
        else if (hit_info.mask.z) {
            color.z = 1.0;
        }
        else {
            color = vec4<f32>(1.0);
        }
        let offset = get_shading_offset(hit_info.hit_pos);
        let raw_color = shading_table[offset].albedo;
        color.x = f32((raw_color >> 24u) & 255u) / 255.0;
        color.y = f32((raw_color >> 16u) & 255u) / 255.0;
        color.z = f32((raw_color >> 8u) & 255u) / 255.0;
        color.w = f32(raw_color & 255u) / 255.0;
        // color = textureLoad(voxels_t, hit_info.hit_pos, 0);
    }

    textureStore(output, img_coord, color);
}