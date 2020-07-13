use {
    super::{instruction::Instruction, Command, Material},
    crate::{camera::Camera, gpu::Mesh, math::Mat4},
    bitflags::bitflags,
};

/// The Compiler struct uses Asm 'assembly' instances internally to
/// store the operations needed to generate the correct instructions
#[derive(Debug)]
enum Asm {
    Line(LineAsm),
    Mesh(MeshAsm),
    // Spotlight(SpotlightCommand),
    // Sunlight(SunlightCommand),
    // Transparency((f32, MeshCommand<'a>)),
}

pub struct Compilation<'c, 'm> {
    compiler: &'c Compiler,
    idx: usize,
    mesh_refs: Vec<&'m Mesh>,
    mesh_sets: MeshSets,
    stages: Stages,
    view_proj: Mat4,
}

impl Compilation<'_, '_> {
    pub fn line_buf_len(&self) -> usize {
        self.compiler.line_buf.len()
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

/// Compiles a series of drawing commands into renderable instructions. Uses an assembly language of Asm instances
/// in order to describe the state.
#[derive(Debug, Default)]
pub struct Compiler {
    code: Vec<Asm>,
    line_buf: Vec<u8>,
}

impl Compiler {
    pub fn compile<'a, 'c, C>(
        &'a mut self,
        camera: &impl Camera,
        cmds: &mut [C],
    ) -> Compilation<'a, 'c>
    where
        C: Into<Command<'c>>,
    {
        let _len = cmds.len();
        let view_proj = camera.view() * camera.projection();
        let mesh_refs = vec![];
        // for cmd in cmds {
        //     match cmd.into() {
        //         Command::Mesh(cmd) => mesh_refs.push(cmd.mesh),
        //         _ => (),
        //     }
        // }

        // let camera_planes = vec![
        //     //Plane::new(normal: Unit<Vector<N>>)
        // ];

        let idx = 0;
        // while idx < len {}

        Compilation {
            compiler: self,
            idx,
            mesh_refs,
            mesh_sets: Default::default(),
            stages: Stages::empty(),
            view_proj,
        }
    }

    pub fn reset(&mut self) {
        self.code.clear();
    }

    // fn mesh_dual_tex_sets_required<'c, C>(mut cmds: C) -> usize
    // where
    //     C: Iterator<Item = &'c Instruction<'c>>,
    // {
    //     let first_cmd = cmds.next().unwrap().as_mesh();
    //     if first_cmd.is_none() {
    //         return 0;
    //     }

    //     let mut sets = 1;
    //     // TODO: let mut diffuse_id = first_cmd.unwrap().mesh.diffuse_id;
    //     // while let Some(cmd) = cmds.next().unwrap().as_mesh() {
    //     //     if cmd.mesh.diffuse_id != diffuse_id {
    //     //         diffuse_id = cmd.mesh.diffuse_id;
    //     //         sets += 1;
    //     //     }
    //     // }

    //     sets
    // }

    // fn mesh_single_tex_sets_required<'c, C>(mut cmds: C) -> usize
    // where
    //     C: Iterator<Item = &'c Instruction<'c>>,
    // {
    //     let first_cmd = cmds.next().unwrap().as_mesh();
    //     if first_cmd.is_none() {
    //         return 0;
    //     }

    //     let mut sets = 1;
    //     // TODO: let mut diffuse_id = first_cmd.unwrap().mesh.diffuse_id;
    //     // while let Some(cmd) = cmds.next().unwrap().as_mesh() {
    //     //     if cmd.mesh.diffuse_id != diffuse_id {
    //     //         diffuse_id = cmd.mesh.diffuse_id;
    //     //         sets += 1;
    //     //     }
    //     // }

    //     sets
    // }
}

#[derive(Debug)]
enum LineAsm {
    Draw(usize),
    SetSize(f32),
}

#[derive(Debug)]
enum MeshAsm {
    Draw((usize, Mat4)),
    SetMaterial(Material),
}

#[derive(Default)]
pub struct MeshSets {
    pub dual_tex: usize,
    pub single_tex: usize,
    pub trans: usize,
}

/*/ Converts a bunch of client-specified drawing commands into drawable instructions
pub fn compile<'c, C>(camera: &impl Camera, cmds: impl Iterator<Item = C>) -> Vec<Instruction<'c>>
where
    C: Into<Command<'c>>,
{
    // Step 1: Filter out commands which are not within the camera frustum
    // TODO!

    // Step 2: Convert to Instructions, which adds required info for Mesh types
    let to_eye = -camera.eye();
    let mut cmds = cmds.map(|cmd| match cmd.into() {
        Command::Line(cmd) => Instruction::Line(cmd),
        Command::Sunlight(cmd) => Instruction::Sunlight(cmd),
        Command::Mesh(cmd) => {
            // Distance from camera (squared; for comparison only)
            let z = cmd
                .transform
                .transform_vector3(to_eye)
                .length_squared();

            if cmd.mesh.has_alpha {
                Instruction::Mesh((z, cmd))
            } else {
                Instruction::Transparency((z, cmd))
            }
        }
        Command::Spotlight(cmd) => Instruction::Spotlight(cmd),
    })
    .collect::<Vec<_>>();

    // Step 3: Sort instructions into draw order
    // - Lines (by width)
    // - Single-Tex Meshes (by texture)
    // - Dual-Tex Meshes (by textures)
    // - Lights
    // - Transparent Meshes (by texture)
    cmds.sort_unstable_by(|lhs: &Instruction, rhs: &Instruction| -> Ordering {
        use {
            Instruction::{Line, Mesh, Spotlight, Sunlight, Transparency},
            Ordering::{Equal, Greater, Less},
        };
        match lhs {
            Line(lhs) => match rhs {
                Line(rhs) => lhs.width.partial_cmp(&rhs.width).unwrap(),
                _ => Less,
            },
            Mesh((lhs_z, lhs)) => match rhs {
                Line(_) => Greater,
                Mesh((rhs_z, rhs)) => {
                    match lhs.mesh.bitmaps.len().cmp(&rhs.mesh.bitmaps.len()) {
                        Equal => {
                            for idx in 0..lhs.mesh.bitmaps.len() {
                                let res = lhs.mesh.bitmaps[idx].0.cmp(&rhs.mesh.bitmaps[idx].0);
                                if res != Equal {
                                    return res;
                                }
                            }

                            lhs_z.partial_cmp(rhs_z).unwrap()
                        }
                        Greater => Greater,
                        Less => Less,
                    }
                }
                _ => Less,
            },
            Sunlight(_) => match rhs {
                Mesh(_) | Spotlight(_) => Greater,
                Sunlight(_) => Equal,
                _ => Less,
            },
            Spotlight(_) => match rhs {
                Mesh(_) => Greater,
                Spotlight(_) => Equal,
                _ => Less,
            },
            Transparency((lhs, _)) => match rhs {
                // TODO: Sort like mesh!
                Transparency((rhs, _)) => lhs.partial_cmp(rhs).unwrap(),
                _ => Less,
            },
            _ => panic!(),
        }
    });

    // Step 4: Use a stop command to indicate we're done
    cmds.push(Instruction::Stop);

    cmds
}*/

// TODO: This is fun but completely not needed; should just store a few bools and move on with life?
bitflags! {
    /// NOTE: I don't bother adding the mesh types here because the only usage of this
    /// is to detect the need to instantiate graphics pipelines; however for the mesh
    /// types we use the 'sets > 0' test to see if we need one.
    pub struct Stages: usize {
        const LINE = Self::bit(0);
        const SPOTLIGHT = Self::bit(1);
        const SUNLIGHT = Self::bit(2);
    }
}

impl Stages {
    /// Returns a usize with the given zero-index bit set to one
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
            let aspect = 45.0;
            let fov = 1.0;
            let near = 1.0;
            let far = 100.0;
            Perspective::new(eye, target, aspect, fov, near, far)
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
