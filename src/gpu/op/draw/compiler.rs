use {
    super::{instruction::Instruction, Command},
    crate::{
        camera::Camera,
        gpu::{Mesh, Texture2d},
        math::Mat4,
    },
    bitflags::bitflags,
    std::cmp::Ordering,
};

pub struct Compilation<'c, 'm> {
    cmds: &'m [Command<'m>],
    compiler: &'c Compiler,
    idx: usize,
    mesh_sets: MeshSets,
    stages: Stages,
    view_proj: Mat4,
}

impl Compilation<'_, '_> {
    /// Returns the accumulated line vertex data for this compilation. It is a blob of all requested lines jammed together.
    pub fn line_buf(&self) -> &[u8] {
        &self.compiler.line_buf
    }

    pub fn mesh_sets_required(&self) -> &MeshSets {
        &self.mesh_sets
    }

    pub fn stages_required(&self) -> Stages {
        self.stages
    }

    pub fn view_proj(&self) -> Mat4 {
        self.view_proj
    }
}

impl<'c> Iterator for Compilation<'c, '_> {
    type Item = Instruction<'c>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Self::Item::Stop)
    }
}

/// Compiles a series of drawing commands into renderable instructions. The purpose of this structure is
/// two-fold:
/// - Reduce per-draw allocations with line and spotlight caches (they are not cleared after each use)
/// - Store references to the in-use mesh textures during rendering (this cache is cleared after use)
#[derive(Debug, Default)]
pub struct Compiler {
    line_buf: Vec<u8>, // Needs the same treatment as spotlights
    mesh_textures: Vec<Texture2d>,
    spotlight_buf: Vec<u8>, // This will store the vertex data for individually rendered spotlights - it will be persistent across frames and it will need to store a seperate list of existing spotlights (to bin search for a match) and which indices make up that light, is it new, etc, so we can not have to re-upload spotlight data frame-to-frame. Will need a maximum size or something which dequeues stale items.
}

impl Compiler {
    /// Compiles a given set of commands into a ready-to-draw list of instructions. Performs these steps:
    /// - Cull commands which might not be visible to the camera
    /// - Sort commands into predictable groupings (opaque meshes, lights, transparent meshes, lines)
    /// - Sort mesh commands further by texture(s) in order to reduce descriptor set switching/usage
    /// - Prepare a single buffer of all line vertex data which can be copied to the GPU all at once
    pub fn compile<'a, 'c>(
        &'a mut self,
        camera: &impl Camera,
        cmds: &'c mut [Command<'c>],
    ) -> Compilation<'a, 'c> {
        assert!(self.line_buf.is_empty());
        assert!(self.mesh_textures.is_empty());

        // Cull any commands which are not within the camera frustum. Use `len` to keep track of the number of active `cmds`.
        let mut idx = 0;
        let mut len = cmds.len();
        while idx < len {
            if match &cmds[idx] {
                Command::Mesh(cmd) => camera.intersects_sphere(cmd.mesh.bounds),
                Command::PointLight(cmd) => camera.intersects_sphere(cmd.bounds()),
                Command::RectLight(cmd) => camera.intersects_sphere(cmd.bounds()),
                Command::Spotlight(cmd) => camera.intersects_cone(cmd.bounds()),
                _ => {
                    // Lines and Sunlight do not get culled; we assume they are visible and draw them
                    // TODO: Test the effect of adding in line culling with lots and lots of lines, make it a feature or argument?
                    true
                }
            } {
                // The command at `idx` has been culled and won't be drawn (put it at the end of the list/no-mans land)
                len -= 1;
                cmds.swap(idx, len);
            } else {
                // The command at `idx` is visible and will draw normally
                idx += 1;
            }
        }

        // Keep track of the stages needed to draw these commands
        // TODO: Roll this into one of the other loops
        let mut stages = Stages::empty();
        for cmd in &cmds[0..len] {
            match cmd {
                Command::Line(_) => stages |= Stages::LINE,
                Command::Mesh(_) => {
                    // TODO: Actual logic!
                    stages |= Stages::MESH_SINGLE_TEX;
                }
                Command::PointLight(_) => stages |= Stages::POINTLIGHT,
                _ => todo!(),
            }
        }

        // Assign a relative measure of distance from the camera for all mesh commands which allows us to submit draw commands
        // in the best order for the z-buffering algorithm (we use a depth map with comparisons that discard covered fragments)
        let to_eye = -camera.eye();
        for cmd in &mut cmds[0..len] {
            if let Command::Mesh(cmd) = cmd {
                // Distance from camera (squared; for comparison only)
                cmd.camera_z = cmd.transform.transform_vector3(to_eye).length_squared();
            }
        }

        // TODO: Sorting meshes by material also - helpful or not?
        // Sort the commands into a predictable and efficient order for drawing
        cmds[0..len].sort_unstable_by(|lhs, rhs| {
            // Shorthand - we only care about equal or not-equal here
            use Ordering::Equal as eq;

            let lhs_idx = Self::group_idx(lhs);
            let rhs_idx = Self::group_idx(rhs);

            // Compare group indices
            match lhs_idx.cmp(&rhs_idx) {
                eq => match lhs {
                    Command::Line(lhs) => {
                        let rhs = rhs.as_line_cmd();

                        // Compare line widths
                        lhs.width.partial_cmp(&rhs.width).unwrap_or(eq)
                    }
                    Command::Mesh(lhs) => {
                        let rhs = rhs.as_mesh_cmd();
                        let lhs_idx = Self::mesh_group_idx(lhs.mesh);
                        let rhs_idx = Self::mesh_group_idx(rhs.mesh);

                        // Compare mesh group indices
                        match lhs_idx.cmp(&rhs_idx) {
                            eq => {
                                for (lhs_tex, rhs_tex) in
                                    lhs.mesh.textures().zip(rhs.mesh.textures())
                                {
                                    let lhs_idx = self.mesh_texture_idx(lhs_tex);
                                    let rhs_idx = self.mesh_texture_idx(rhs_tex);

                                    // Compare mesh texture indices
                                    match lhs_idx.cmp(&rhs_idx) {
                                        eq => continue,
                                        ne => return ne,
                                    }
                                }

                                // Compare z-order (sorting in closer to further)
                                lhs.camera_z.partial_cmp(&rhs.camera_z).unwrap_or(eq)
                            }
                            ne => ne,
                        }
                    }
                    _ => eq,
                },
                ne => ne,
            }
        });

        Compilation {
            cmds: &cmds[0..len],
            compiler: self,
            idx: 0,
            mesh_sets: Default::default(),
            stages,
            view_proj: camera.view() * camera.projection(),
        }
    }

    /// All commands sort into groups: first meshes (all types), then sunlights, rect lights, spotlights, point lights, followed by lines.
    fn group_idx(cmd: &Command) -> usize {
        // TODO: Transparencies?
        match cmd {
            Command::Mesh(_) => 0,
            Command::Sunlight(_) => 1,
            Command::RectLight(_) => 2,
            Command::Spotlight(_) => 3,
            Command::PointLight(_) => 4,
            Command::Line(_) => 5,
        }
    }

    /// Meshes sort into sub-groups: first animated, then single texture, followed by dual texture.
    fn mesh_group_idx(mesh: &Mesh) -> usize {
        // TODO: Transparencies?
        if mesh.is_animated() {
            0
        } else if mesh.is_single_texture() {
            1
        } else {
            2
        }
    }

    /// Returns the index of a given texture in our `mesh texture` list, adding it as needed.
    fn mesh_texture_idx(&mut self, tex: &Texture2d) -> usize {
        // TODO: Use `weak_into_raw` feature when available
        // HACK: This is the same crappy sorting problem/solution as WriteOp
        for (idx, val) in self.mesh_textures.iter().enumerate() {
            if Texture2d::ptr_eq(tex, val) {
                return idx;
            }
        }

        // Not in the list - add and return the new index
        let len = self.mesh_textures.len();
        self.mesh_textures.push(Texture2d::clone(tex));
        len
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile` function.
    pub fn reset(&mut self) {
        self.line_buf.clear();
        self.mesh_textures.clear();
    }
}

#[derive(Default)]
pub struct MeshSets {
    pub dual_tex: usize,
    pub single_tex: usize,
    pub trans: usize,
}

bitflags! {
    pub struct Stages: usize {
        const LINE = Self::bit(0);
        const MESH_ANIMATED = Self::bit(1);
        const MESH_DUAL_TEX = Self::bit(2);
        const MESH_SINGLE_TEX = Self::bit(3);
        const MESH_TRANSPARENT = Self::bit(4);
        const POINTLIGHT = Self::bit(5);
        const RECTLIGHT = Self::bit(6);
        const SPOTLIGHT = Self::bit(7);
        const SUNLIGHT = Self::bit(8);
    }
}

impl Stages {
    /// Returns a usize with the given zero-indexed bit set to one
    const fn bit(b: usize) -> usize {
        1 << b
    }
}

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{camera::Perspective, math::vec3},
    };

    #[test]
    fn test_no_commands() {
        let camera = {
            let eye = vec3(-10.0, 0.0, 0.0);
            let target = vec3(10.0, 0.0, 0.0);
            let width = 320.0;
            let height = 200.0;
            let fov = 45.0;
            let near = 1.0;
            let far = 100.0;
            Perspective::new_view(eye, target, near..far, fov, (width, height))
        };
        let mut compiler = Compiler::default();
        let mut cmds: Vec<Command> = vec![];
        let res = compiler.compile(&camera, &mut cmds);

        assert!(res.stages_required().is_empty());
        assert_eq!(res.mesh_sets_required().dual_tex, 0);
        assert_eq!(res.mesh_sets_required().single_tex, 0);
        assert_eq!(res.mesh_sets_required().trans, 0);
    }
}
