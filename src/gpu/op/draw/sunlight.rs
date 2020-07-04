use crate::{
    color::Color,
    math::{Mat4, Vec3},
};

#[derive(Debug)]
pub struct SunlightCommand {
    normal_inv: Vec3,
    diffuse: Color,
    power: f32,
    light_space: Mat4,
}

// impl SunlightCommand {
//     fn new<C>(camera: C, e: &Sunlight) -> Self
//     where
//         C: Camera,
//     {
//         let view_inv = camera.view_inv();

//         // TODO: Calculate this with object AABBs once those are ready (any AABB inside both the camera and shadow projections)
//         // Calculate the world-space coords of the eight points that make up our camera frustum
//         // and calculate the min/max/mid coordinates of them
//         let camera_world = [
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, -1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(-1.0, 1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, -1.0, 1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, -1.0)), 1.0))
//                 .truncate(),
//             (view_inv * vec4_from_vec3(camera.unproject_point(vec3(1.0, 1.0, 1.0)), 1.0))
//                 .truncate(),
//         ];
//         let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
//             let p0 = camera_world[0];
//             (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
//         };
//         for pi in &camera_world {
//             min_x = pi.x().min(min_x);
//             min_y = pi.y().min(min_y);
//             min_z = pi.z().min(min_z);
//             max_x = pi.x().max(max_x);
//             max_y = pi.y().max(max_y);
//             max_z = pi.z().max(max_z);
//         }
//         let mid_x = (max_x + min_x) / 2.0;
//         let mid_y = (max_y + min_y) / 2.0;
//         let mid_z = (max_z + min_z) / 2.0;
//         let position = vec3(mid_x, mid_y, mid_z);
//         let target = position + e.normal;
//         let n_dot_x = e.normal.dot(Vec3::unit_x()).abs();
//         let n_dot_y = e.normal.dot(Vec3::unit_y()).abs();
//         let up = if n_dot_x < n_dot_y {
//             Vec3::unit_x()
//         } else {
//             Vec3::unit_y()
//         };
//         let light_view = Mat4::look_at_rh(position, target, up);
//         let light_world = [
//             (light_view * vec4_from_vec3(camera_world[0], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[1], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[2], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[3], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[4], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[5], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[6], 1.0)).truncate(),
//             (light_view * vec4_from_vec3(camera_world[7], 1.0)).truncate(),
//         ];
//         let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) = {
//             let p0 = light_world[0];
//             (p0.x(), p0.y(), p0.z(), p0.x(), p0.y(), p0.z())
//         };
//         for pi in &light_world {
//             min_x = pi.x().min(min_x);
//             min_y = pi.y().min(min_y);
//             min_z = pi.z().min(min_z);
//             max_x = pi.x().max(max_x);
//             max_y = pi.y().max(max_y);
//             max_z = pi.z().max(max_z);
//         }
//         let light_space =
//             Mat4::orthographic_rh(min_x, max_x, min_y, max_y, min_z, max_z) * light_view;

//         Self {
//             normal_inv: -e.normal,
//             diffuse: e.diffuse,
//             power: e.power,
//             light_space,
//         }
//     }
// }
