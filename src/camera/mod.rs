mod orthographic;
mod perspective;

use crate::math::{vec3, vec4_from_vec3, Mat4, Vec3};

pub use self::orthographic::Orthographic;
pub use self::perspective::Perspective;

pub trait Camera {
    fn eye(&self) -> Vec3;
    fn project_point(&self, p: Vec3) -> Vec3;
    fn projection(&self) -> Mat4;
    fn unproject_point(&self, p: Vec3) -> Vec3;
    fn view(&self) -> Mat4;
    fn view_inv(&self) -> Mat4;

    /// Gets the world-space corner positions of the viewing frustum
    fn corners(&self) -> [Vec3; 8] {
        let view_inv = self.view_inv();

        // Unproject and transform the NDC double-cube coordinates
        const P: f32 = 1.0;
        const N: f32 = -1.0;
        [
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, N, N)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, N, P)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, P, N)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, P, P)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, N, N)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, N, P)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, P, N)), 1.0)).truncate(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, P, P)), 1.0)).truncate(),
        ]
    }
}
