mod orthographic;
mod perspective;

pub use self::{orthographic::Orthographic, perspective::Perspective};

use crate::math::{vec3, vec4_from_vec3, Cone, Mat4, Sphere, Vec3};

// TODO: Remove unused functions from below and also make sure the impls haven't got conflicting functions!!!
pub trait Camera {
    
    fn intersects_sphere(&self, sphere: Sphere) -> bool;

    // NOTE: These functions might change!
    fn eye(&self) -> Vec3;
    fn intersects_cone(&self, cone: Cone) -> bool;
    fn project_point(&self, p: Vec3) -> Vec3;
    fn projection(&self) -> Mat4;
    fn unproject_point(&self, p: Vec3) -> Vec3;
    fn view(&self) -> Mat4;
    fn view_inv(&self) -> Mat4;

    // TODO: Add tests or an example; otherwise just remove this
    /// Gets the world-space corner positions of the viewing frustum
    fn corners(&self) -> [Vec3; 8] {
        let view_inv = self.view_inv();

        // Unproject and transform the NDC double-cube coordinates
        const P: f32 = 1.0;
        const N: f32 = -1.0;
        [
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, N, N)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, N, P)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, P, N)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(N, P, P)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, N, N)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, N, P)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, P, N)), 1.0))
                .truncate()
                .into(),
            (view_inv * vec4_from_vec3(self.unproject_point(vec3(P, P, P)), 1.0))
                .truncate()
                .into(),
        ]
    }
}

#[cfg(test)]
mod test {
    // use {super::*, crate::math::vec3};

    #[test]
    fn test_camera_corners() {
        // TODO!
    }
}
