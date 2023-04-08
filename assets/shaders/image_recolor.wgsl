@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn compute(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let img_coord = vec2<i32>(global_id.xy);
    let img_dims = textureDimensions(output);
    let img_coord_frac = vec2<f32>(
        f32(img_coord.x) / f32(img_dims.x),
        f32(img_coord.y) / f32(img_dims.y)
    );
    textureStore(output, img_coord, vec4<f32>(img_coord_frac, 1.0, 1.0));
}