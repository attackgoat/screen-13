use {
    super::Camera,
    crate::math::{Cone, CoordF, Mat4, Sphere, Vec3},
    std::ops::Range,
};

/// A standard orthographic camera.
#[derive(Clone)]
pub struct Orthographic {
    depth: Range<f32>,
    eye: Vec3,
    proj: Mat4,
    proj_inv: Mat4,
    target: Vec3,
    view: Mat4,
    view_inv: Mat4,
}

impl Orthographic {
    /// Constructs an `Orthographic` instance from the given values.
    pub fn new<T: Into<CoordF>>(eye: Vec3, target: Vec3, dims: T, depth: Range<f32>) -> Self {
        let dims = dims.into();
        let mut result = Self {
            depth: depth.clone(),
            eye,
            proj: Mat4::orthographic_rh_gl(0.0, dims.x, dims.y, 0.0, depth.start, depth.end),
            proj_inv: Mat4::IDENTITY, // TODO: Fix this up!
            target,
            view: Mat4::IDENTITY,
            view_inv: Mat4::IDENTITY,
        };
        result.update_view();
        result
    }

    // pub fn bottom(&self) -> f32 {
    //     self.proj.bottom()
    // }

    // pub fn far(&self) -> f32 {
    //     self.proj.zfar()
    // }

    // pub fn left(&self) -> f32 {
    //     self.proj.left()
    // }

    // pub fn near(&self) -> f32 {
    //     self.proj.znear()
    // }

    /// Gets the position this camera is looking at.
    pub fn target(&self) -> Vec3 {
        self.target
    }

    /// Sets the position this camera is looking from.
    pub fn set_eye(&mut self, eye: Vec3) {
        self.eye = eye;
        self.update_view();
    }

    // pub fn set_far(&mut self, value: f32) {
    //     self.proj.set_zfar(value);
    // }

    // pub fn set_near(&mut self, value: f32) {
    //     self.proj.set_znear(value);
    // }

    /// Sets the camera target position.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
        self.update_view();
    }

    // pub fn top(&self) -> f32 {
    //     self.proj.top()
    // }

    fn update_view(&mut self) {
        self.view = Mat4::look_at_rh(self.eye, self.target, Vec3::Y);
        self.view_inv = self.view.inverse();
    }
}

impl Camera for Orthographic {
    fn depth(&self) -> &Range<f32> {
        &self.depth
    }

    fn eye(&self) -> Vec3 {
        self.eye
    }

    fn overlaps_cone(&self, _c: Cone) -> bool {
        true
    }

    fn overlaps_point(&self, _p: Vec3) -> bool {
        true
    }

    fn overlaps_sphere(&self, _s: Sphere) -> bool {
        true
    }

    fn project_point(&self, p: Vec3) -> Vec3 {
        // TODO: These should be view projection transforms!
        self.proj.transform_point3(p)
    }

    fn projection(&self) -> Mat4 {
        self.proj
    }

    fn unproject_point(&self, p: Vec3) -> Vec3 {
        // TODO: These should be view projection transforms!
        self.proj_inv.transform_point3(p)
    }

    fn view(&self) -> Mat4 {
        self.view
    }

    fn view_inv(&self) -> Mat4 {
        self.view_inv
    }
}
