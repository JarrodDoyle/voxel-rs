/// Linear interpolation.
#[inline]
pub fn lerp(a: f32, b: f32, w: f32) -> f32 {
    assert!(0.0 <= w && w <= 1.0);
    a + (b - a) * w
}

/// Bilinear interpolation.
/// Expected order of `p` is from a nested `for` loop with the outer loop being `y`.
/// `w` is expected to be `[wx, wy]`
#[inline]
pub fn bi_lerp(p: &[f32], w: &[f32]) -> f32 {
    assert_eq!(p.len(), 4);
    assert_eq!(w.len(), 2);

    lerp(lerp(p[0], p[1], w[0]), lerp(p[2], p[3], w[0]), w[1])
}

/// Trilinear interpolation.
/// Expected order of `p` is from a nested `for` loop with the outer loop being `z`.
/// `w` is expected to be `[wx, wy, wz]`.
#[inline]
pub fn tri_lerp(p: &[f32], w: &[f32]) -> f32 {
    assert_eq!(p.len(), 8);
    assert_eq!(w.len(), 3);

    let c00 = p[0] + (p[1] - p[0]) * w[0];
    let c10 = p[2] + (p[3] - p[2]) * w[0];
    let c01 = p[4] + (p[5] - p[4]) * w[0];
    let c11 = p[6] + (p[7] - p[6]) * w[0];
    let c0 = c00 + (c10 - c00) * w[1];
    let c1 = c01 + (c11 - c01) * w[1];
    c0 + (c1 - c0) * w[2]
}

/// Computes a unifom grid of trilinear interpolations
#[inline]
pub fn tri_lerp_block(p: &[f32], dims: &[u32], vals: &mut [f32]) {
    assert_eq!(p.len(), 8);
    assert_eq!(dims.len(), 3);
    // assert vals length matches dims

    // Precalculate coefficients
    let a0 = p[0];
    let a1 = -p[0] + p[1];
    let a2 = -p[0] + p[2];
    let a3 = -p[0] + p[4];
    let a4 = p[0] - p[2] - p[1] + p[3];
    let a5 = p[0] - p[4] - p[1] + p[5];
    let a6 = p[0] - p[4] - p[2] + p[6];
    let a7 = -p[0] + p[4] + p[2] - p[6] + p[1] - p[5] - p[3] + p[7];

    // Calculate each value
    let dx_max = (dims[0] - 1) as f32;
    let dy_max = (dims[1] - 1) as f32;
    let dz_max = (dims[2] - 1) as f32;
    let mut i = 0;
    for z in 0..dims[0] {
        for y in 0..dims[1] {
            for x in 0..dims[2] {
                let dx = x as f32 / dx_max;
                let dy = y as f32 / dy_max;
                let dz = z as f32 / dz_max;

                let val = a0
                    + a1 * dx
                    + a2 * dy
                    + a3 * dz
                    + a4 * dx * dy
                    + a5 * dx * dz
                    + a6 * dy * dz
                    + a7 * dx * dy * dz;
                vals[i] = val;
                i += 1;
            }
        }
    }
}

/// Maps a 3d index to a 1d index
pub fn to_1d_index(p: glam::UVec3, dim: glam::UVec3) -> usize {
    (p.x + p.y * dim.x + p.z * dim.x * dim.y) as usize
}
