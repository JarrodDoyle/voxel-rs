// Based on: https://www.shadertoy.com/view/4dX3zl

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;

fn sdSphere(p: vec3<f32>, d: f32) -> f32 {
    return length(p) - d;
}

fn sdBox(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let d: vec3<f32> = abs(p) - b;
    return min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, vec3<f32>(0.0)));
}

fn getVoxel(c: vec3<i32>) -> bool {
    let p = vec3<f32>(c) + vec3<f32>(0.5);
    let d = min(max(-sdSphere(p, 7.5), sdBox(p, vec3<f32>(6.0))), -sdSphere(p, 25.0));
    return d < 0.0;
}

@compute @workgroup_size(8, 8, 1)
fn compute(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let img_coord = vec2<i32>(global_id.xy);
    let img_dims = textureDimensions(output);
    let img_coord_frac = vec2<f32>(
        f32(img_coord.x) / f32(img_dims.x),
        f32(img_coord.y) / f32(img_dims.y)
    );

    // Camera setup
    let screen_pos = img_coord_frac * 2.0 - vec2<f32>(1.0);
    let camera_dir = vec3<f32>(0.0, 0.0, 0.8);
    let camera_plane_u = vec3<f32>(1.0, 0.0, 0.0);
    let camera_plane_v = vec3<f32>(0.0, 1.0, 0.0) * f32(img_dims.y) / f32(img_dims.x);
    let ray_dir = camera_dir + screen_pos.x * camera_plane_u + screen_pos.y * camera_plane_v;
    let ray_pos = vec3<f32>(0.01, 2.0, -12.0);

    // DDA setup
    var map_pos: vec3<i32> = vec3<i32>(floor(ray_pos + 0.0));
    var delta_dist: vec3<f32> = abs(vec3<f32>(length(ray_dir)) / ray_dir);
    var ray_step: vec3<i32> = vec3<i32>(sign(ray_dir));
    var side_dist: vec3<f32> = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;
    var mask: vec3<bool> = vec3<bool>(false, false, false);

    // DDA loop
    for (var i: i32 = 0; i < 64; i++){
        if (getVoxel(map_pos)) {
            break;
        }
        if (side_dist.x < side_dist.y) {
            if (side_dist.x < side_dist.z) {
                side_dist += vec3<f32>(delta_dist.x, 0.0, 0.0);
                map_pos += vec3<i32>(ray_step.x, 0, 0);
                mask = vec3<bool>(true, false, false);
            }
            else {
                side_dist += vec3<f32>(0.0, 0.0, delta_dist.z);
                map_pos += vec3<i32>(0, 0, ray_step.z);
                mask = vec3<bool>(false, false, true);
            }
        }
        else {
            if (side_dist.y < side_dist.z) {
                side_dist += vec3<f32>(0.0, delta_dist.y, 0.0);
                map_pos += vec3<i32>(0, ray_step.y, 0);
                mask = vec3<bool>(false, true, false);
            }
            else {
                side_dist += vec3<f32>(0.0, 0.0, delta_dist.z);
                map_pos += vec3<i32>(0, 0, ray_step.z);
                mask = vec3<bool>(false, false, true);
            }
        }
    }

    var color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (mask.x) {
        color.x = 1.0;
    }
    if (mask.y) {
        color.y = 1.0;
    }
    if (mask.z) {
        color.z = 1.0;
    }

    textureStore(output, img_coord, color);
}