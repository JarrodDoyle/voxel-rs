/// Linear interpolation.
pub fn lerp(a: f32, b: f32, w: f32) -> f32 {
    assert!(0.0 <= w && w <= 1.0);
    a + (b - a) * w
}

/// Bilinear interpolation.
/// Expected order of `p` is from a nested `for` loop with the outer loop being `y`.
/// `w` is expected to be `[wx, wy]`
pub fn bi_lerp(p: &[f32], w: &[f32]) -> f32 {
    assert_eq!(p.len(), 4);
    assert_eq!(w.len(), 2);

    lerp(lerp(p[0], p[1], w[0]), lerp(p[2], p[3], w[0]), w[1])
}

/// Trilinear interpolation.
/// Expected order of `p` is from a nested `for` loop with the outer loop being `z`.
/// `w` is expected to be `[wx, wy, wz]`.
pub fn tri_lerp(p: &[f32], w: &[f32]) -> f32 {
    assert_eq!(p.len(), 8);
    assert_eq!(w.len(), 3);

    let front = bi_lerp(&[p[0], p[1], p[2], p[3]], &[w[0], w[1]]);
    let back = bi_lerp(&[p[4], p[5], p[6], p[7]], &[w[0], w[1]]);
    lerp(front, back, w[2])
}

/// Maps a 3d index to a 1d index
pub fn to_1d_index(p: glam::UVec3, dim: glam::UVec3) -> usize {
    (p.x + p.y * dim.x + p.z * dim.x * dim.y) as usize
}
