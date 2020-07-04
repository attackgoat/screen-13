use crate::math::{Mat4, Vec3};

use super::Camera;

#[derive(Clone, Copy)]
pub struct Orthographic {
    eye: Vec3,
    proj: Mat4,
    target: Vec3,
    view: Mat4,
    view_inv: Mat4,
}

impl Orthographic {
    pub fn new(eye: Vec3, proj: Mat4, target: Vec3) -> Self {
        let mut result = Self {
            eye,
            proj,
            target,
            view: Mat4::identity(),
            view_inv: Mat4::identity(),
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

    pub fn target(&self) -> Vec3 {
        self.target
    }

    pub fn set_eye(&mut self, value: Vec3) {
        self.eye = value;
        self.update_view();
    }

    // pub fn set_far(&mut self, value: f32) {
    //     self.proj.set_zfar(value);
    // }

    // pub fn set_near(&mut self, value: f32) {
    //     self.proj.set_znear(value);
    // }

    pub fn set_target(&mut self, value: Vec3) {
        self.target = value;
        self.update_view();
    }

    // pub fn top(&self) -> f32 {
    //     self.proj.top()
    // }

    fn update_view(&mut self) {
        self.view = Mat4::look_at_rh(self.eye, self.target, Vec3::unit_y());
        self.view_inv = self.view.inverse();
    }
}

impl Camera for Orthographic {
    fn eye(&self) -> Vec3 {
        self.eye
    }

    fn project_point(&self, p: Vec3) -> Vec3 {
        self.proj.transform_point3(p)
    }

    fn projection(&self) -> Mat4 {
        self.proj
    }

    fn unproject_point(&self, p: Vec3) -> Vec3 {
        self.proj.inverse().transform_point3(p) // TODO: Oh no no no
    }

    fn view(&self) -> Mat4 {
        self.view
    }

    fn view_inv(&self) -> Mat4 {
        self.view_inv
    }
}
