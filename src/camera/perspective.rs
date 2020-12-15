use {
    super::{Camera, Category},
    crate::math::{vec3_is_finite, Cone, CoordF, Mat4, Sphere, Vec2, Vec3},
    std::ops::Range,
};

// TODO: Play around with this and see if depth should be RangeInclusive instead?

#[derive(Clone)]
pub struct Perspective {
    aspect_ratio: f32,
    depth: Range<f32>,
    eye: Vec3,
    fov: f32, // Stored as radians, measures field of view of the Y axis from our z reference vector (so half the field of view)
    fov_tan: f32,
    proj: Mat4,
    sphere_factor: Vec2,
    target: Vec3,
    up: Vec3,
    view: Mat4,
    view_inv: Mat4,
    x: Vec3, // Reference vector aka "local x" or "right"
    y: Vec3, // Reference vector aka "local y" or "up"
    z: Vec3, // Reference vector aka "local z" - this points straight through our "lens"
}

impl Perspective {
    /// Creates a new perspective camera using +y as the default "up" vector.
    ///
    /// # Arguments
    ///
    /// * `eye` - Position this camera is pointing from.
    /// * `target` - Position this camera is pointing towards.
    /// * `depth` - Range of distance this camera can see.
    /// * `fov` - Full field of view on the X axis, in degrees.
    /// * `aspect_ratio` - Width of the view of this camera divided by height.
    pub fn new(eye: Vec3, target: Vec3, depth: Range<f32>, fov: f32, aspect_ratio: f32) -> Self {
        let mut res = Self {
            aspect_ratio,
            depth,
            eye,
            fov: Default::default(),
            fov_tan: Default::default(),
            proj: Default::default(),
            sphere_factor: Default::default(),
            target,
            up: -Vec3::unit_y(),
            view: Default::default(),
            view_inv: Default::default(),
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
        };
        res.set_fov(fov);
        res.update_view();
        res
    }

    /// Creates a new perspective camera using +y as the default "up" vector.
    ///
    /// # Arguments
    ///
    /// * `eye` - Position this camera is pointing from.
    /// * `target` - Position this camera is pointing towards.
    /// * `depth` - Range of distance this camera can see.
    /// * `fov` - Full field of view on the X axis, in degrees.
    /// * `shape` - Defines the aspect ratio of the view of this camera.
    pub fn new_view<S: Into<CoordF>>(
        eye: Vec3,
        target: Vec3,
        depth: Range<f32>,
        fov: f32,
        shape: S,
    ) -> Self {
        let shape = shape.into();

        assert!(shape.is_finite());
        assert!(shape.x > 0.0);
        assert!(shape.y > 0.0);

        Self::new(eye, target, depth, fov, shape.x / shape.y)
    }

    /// Returns the width of the view of this camera compared to the height.
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    /// Returns the axial category and sign of a non-overlapping point, or `None`. Tests in
    /// Z-Y-X order.
    fn classify_point(&self, p: Vec3) -> Option<Category> {
        let dir = p - self.eye;

        // The point must be between our near and far planes, which are parallel
        let mut len = dir.dot(self.z);
        if len < self.depth.start {
            return Some(Category::Z(false));
        } else if len > self.depth.end {
            return Some(Category::Z(true));
        }

        len *= self.fov_tan;

        // Compare point to the top and bottom planes, which are not parallel
        let axis = dir.dot(self.y);
        if axis < -len {
            return Some(Category::Y(false));
        } else if axis > len {
            return Some(Category::Y(true));
        }

        len *= self.aspect_ratio;

        // Compare point to the left and right planes, which are not parallel
        let axis = dir.dot(self.x);
        if axis < -len {
            return Some(Category::X(false));
        } else if axis > len {
            return Some(Category::X(true));
        }

        None
    }

    /// Returns the axial category and sign of a non-overlapping sphere, or `None`. Tests in
    /// Z-Y-X order.
    fn classify_sphere(&self, s: Sphere) -> Option<Category> {
        // Note: This implementation is based on the 'radar' approach detailed here:
        // http://www.lighthouse3d.com/tutorials/view-frustum-culling/
        let dir = s.center() - self.eye;

        // The sphere must be between our near and far planes, which are parallel
        let mut len = dir.dot(self.z);
        if len < self.depth.start - s.radius() {
            return Some(Category::Z(false));
        } else if len > self.depth.end + s.radius() {
            return Some(Category::Z(true));
        }

        len *= self.fov_tan;

        // Compare sphere to the top and bottom planes, which are not parallel
        let axis = dir.dot(self.y);
        let radius = self.sphere_factor.y * s.radius();
        if axis < -len - radius {
            return Some(Category::Y(false));
        } else if axis > len + radius {
            return Some(Category::Y(true));
        }

        len *= self.aspect_ratio;

        // Compare sphere to the left and right planes, which are not parallel
        let axis = dir.dot(self.x);
        let radius = self.sphere_factor.x * s.radius();
        if axis < -len - radius {
            return Some(Category::X(false));
        } else if axis > len + radius {
            return Some(Category::X(true));
        }

        // Not overlapping
        None
    }

    /// Returns the position this camera is pointing from.
    pub fn eye(&self) -> Vec3 {
        self.eye
    }

    /// Returns the maximum distance this camera can see.
    pub const fn far(&self) -> f32 {
        self.depth.end
    }

    /// Returns the full field of view of the X axis, in degrees.
    pub fn fov(&self) -> f32 {
        self.fov.to_degrees() * 2.0 * self.aspect_ratio
    }

    /// Returns the minimum distance this camera can see.
    pub const fn near(&self) -> f32 {
        self.depth.start
    }

    /// Returns the position this camera is pointing towards.
    pub fn target(&self) -> Vec3 {
        self.target
    }

    /// Returns the orientation of the view of this camera, which is +y by default.
    pub fn up(&self) -> Vec3 {
        self.up
    }

    /// Modifies the shape of this camera.
    ///
    /// # Arguments
    ///
    /// * `val` - Width of the output of this camera divided by height.
    pub fn set_aspect_ratio(&mut self, val: f32) {
        assert!(val.is_finite());
        assert!(val > 0.0);

        self.aspect_ratio = val;
        self.update_proj();
    }

    /// Modifies the near and far planes of this camera, which defines the distance this camera can see.
    pub fn set_depth(&mut self, val: Range<f32>) {
        self.depth = val;
        self.update_proj();
    }

    /// Modifies the position which this camera is pointing from.
    pub fn set_eye(&mut self, val: Vec3) {
        self.eye = val;
        self.update_view();
    }

    /// Modifies the field of view of this camera.
    pub fn set_fov(&mut self, val: f32) {
        assert!(val.is_finite());
        assert!(val > 0.0);
        assert!(val < 180.0);

        self.fov = val.to_radians() * 0.5 / self.aspect_ratio;
        self.update_proj();
    }

    /// Modifies the position which this camera is pointing towards.
    pub fn set_target(&mut self, val: Vec3) {
        self.target = val;
        self.update_view();
    }

    /// Modifies the orientation of the view of this camera.
    pub fn set_up(&mut self, val: Vec3) {
        self.up = val;
        self.update_view();
    }

    /// Modifies the shape and field of view of this camera.
    ///
    /// # Arguments
    ///
    /// * `shape` - Defines the aspect ratio of the view of this camera.
    /// * `fov` - Full field of view on the X axis, in degrees.
    pub fn set_view<S: Into<CoordF>>(&mut self, shape: S, fov: f32) {
        let shape = shape.into();

        assert!(shape.is_finite());
        assert!(shape.x > 0.0);
        assert!(shape.y > 0.0);
        assert!(fov.is_finite());
        assert!(fov > 0.0);
        assert!(fov < 180.0);

        self.aspect_ratio = shape.x / shape.y;
        self.fov = fov.to_radians() * 0.5 / self.aspect_ratio;
        self.update_proj();
    }

    fn update_proj(&mut self) {
        assert!(self.aspect_ratio.is_finite());
        assert!(self.aspect_ratio > 0.0);
        assert!(self.depth.end.is_finite());
        assert!(self.depth.start.is_finite());
        assert!(self.depth.start > 0.0);
        assert!(self.fov.is_finite());
        assert!(self.depth.start < self.depth.end);

        // Update the projection matrix
        self.proj = Mat4::perspective_rh_gl(
            self.aspect_ratio,
            self.fov * 2.0,
            self.depth.start,
            self.depth.end,
        );

        // Update values we use for frustum-sphere intersection checks
        self.fov_tan = self.fov.tan();
        self.sphere_factor.x = 1.0 / (self.fov_tan * self.aspect_ratio).atan().cos();
        self.sphere_factor.y = 1.0 / self.fov.cos();
    }

    fn update_view(&mut self) {
        assert!(vec3_is_finite(self.eye));
        assert!((self.eye - self.target).length_squared() > 0.0);
        assert!(vec3_is_finite(self.target));
        assert!(vec3_is_finite(self.up));
        assert!(self.up.length_squared() > 0.0);

        // Update the view matrices
        self.view = Mat4::look_at_rh(self.eye, self.target, self.up);
        self.view_inv = self.view.inverse();

        // Update the local X/Y/Z axes aka the reference vectors
        self.z = (self.eye - self.target).normalize();
        self.x = self.up.cross(self.z).normalize();
        self.y = -self.z.cross(self.x);
    }
}

impl Camera for Perspective {
    fn depth(&self) -> &Range<f32> {
        &self.depth
    }

    fn eye(&self) -> Vec3 {
        self.eye
    }

    fn overlaps_cone(&self, c: Cone) -> bool {
        // Test the apex point, hoping to early-out if it overlaps
        let apex = self.classify_point(c.apex());
        if apex.is_none() {
            return true;
        }

        // Test a sphere around the base, hoping to early-out if it overlaps
        let base =
            self.classify_sphere(Sphere::new(c.apex() + c.normal() * c.height(), c.radius()));
        if apex.is_none() {
            return true;
        }

        let apex = apex.unwrap();
        let base = base.unwrap();

        // The cone *may* overlap, so the next step is to throw out the common case we are certain of:
        // - If `apex` and `base` are on the same side of the same axis then they cannot overlap.
        match apex {
            Category::X(apex) => {
                if let Category::X(base) = base {
                    return apex != base;
                }
            }
            Category::Y(apex) => {
                if let Category::Y(base) = base {
                    return apex != base;
                }
            }
            Category::Z(apex) => {
                if let Category::Z(base) = base {
                    return apex != base;
                }
            }
        }

        // TODO: Further refine by classifying yet-unknown X and Y cases - Will require re-doing the math for Z and Y

        // Not sure if they overlap yet, but we can switch to a loose-fitting sphere test which is good enough
        // TODO: Not correct, need better sphere!
        let s = Sphere::new(c.apex() + c.normal() * c.height(), c.radius());
        self.classify_sphere(s).is_none()
    }

    fn overlaps_point(&self, p: Vec3) -> bool {
        self.classify_point(p).is_none()
    }

    fn overlaps_sphere(&self, s: Sphere) -> bool {
        self.classify_sphere(s).is_none()
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
        self.proj.inverse().transform_point3(p) // TODO: Oh no no no
    }

    fn view(&self) -> Mat4 {
        self.view
    }

    fn view_inv(&self) -> Mat4 {
        self.view_inv
    }
}
