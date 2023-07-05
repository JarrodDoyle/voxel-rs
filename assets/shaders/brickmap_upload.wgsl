@group(0) @binding(0) var<uniform> world_state: WorldState;
@group(0) @binding(1) var<storage, read_write> brickgrid: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> brickmap_cache: array<Brickmap>;
@group(0) @binding(3) var<storage, read_write> shading_table: array<ShadingElement>;
@group(0) @binding(4) var<storage, read> brickmap_unpack: BrickmapUnpack;
@group(0) @binding(5) var<storage, read> brickgrid_unpack: BrickgridUnpack;

struct ShadingElement {
    albedo: u32,
}

struct Brickmap {
    bitmask: array<u32, 16>,
    shading_table_offset: u32,
    lod_color: u32,
}

struct WorldState {
    brickgrid_dims: vec3<u32>,
    _pad: u32,
};

struct BrickmapUnpack {
    max_count: u32,
    count: u32,
    _pad1: u32,
    _pad2: u32,
    elements: array<BrickmapUnpackElement>,
}

struct BrickmapUnpackElement {
    cache_idx: u32,
    brickmap: Brickmap,
    shading_element_count: u32,
    shading_elements: array<ShadingElement, 512>, // Always have space for a full map.
}

struct BrickgridUnpack {
    max_count: u32,
    count: u32,
    _pad1: u32,
    _pad2: u32,
    elements: array<BrickgridUnpackElement>,
}

struct BrickgridUnpackElement {
    grid_idx: u32,
    grid_val: u32,
}

// Utility function. Converts a position in 3d to a 1d index.
fn to_1d_index(p: vec3<i32>, dims: vec3<i32>) -> u32 {
    return u32(p.x + p.y * dims.x + p.z * dims.x * dims.y);
}

@compute @workgroup_size(8,1,1)
fn compute(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let unpack_idx = global_id.x;

    // Brickgrid unpacking
    if (unpack_idx < brickgrid_unpack.count){
        let element = brickgrid_unpack.elements[unpack_idx];
        brickgrid[element.grid_idx] = element.grid_val;
    }

    // Brickmap unpacking
    if (unpack_idx < brickmap_unpack.count) {
        let element = &brickmap_unpack.elements[unpack_idx];
        brickmap_cache[(*element).cache_idx] = (*element).brickmap;
        let st_offset = (*element).brickmap.shading_table_offset;
        for (var i: u32 = 0u; i < (*element).shading_element_count; i++) {
            shading_table[st_offset + i] = (*element).shading_elements[i];
        }
    }
}