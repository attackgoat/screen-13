use {
    super::{Command, Instruction},
    crate::{
        gpu::Texture2d,
        math::{CoordF, Mat4, RectF},
        ptr::Shared,
    },
    archery::SharedPointerKind,
    std::borrow::Borrow,
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[derive(Clone, Copy)]
enum Asm {
    BindTextureDescriptorSet(usize),
    WriteTexture(RectF, Mat4),
}

pub struct Compilation<'a, P>
where
    P: 'static + SharedPointerKind,
{
    compiler: &'a mut Compiler<P>,
    idx: usize,
}

impl<P> Compilation<'_, P>
where
    P: SharedPointerKind,
{
    fn bind_texture_descriptor_set(&self, idx: usize) -> Instruction {
        // Probably going to want this back in the future
        // let src = Shared::as_ptr(&self.compiler.cmds[idx].src);
        // let desc_set = self
        //     .compiler
        //     .textures
        //     .binary_search_by(|probe| Shared::as_ptr(&probe).cmp(&src))
        //     .unwrap();

        // Instruction::TextureBindDescriptorSet(desc_set)
        Instruction::TextureBindDescriptorSet(idx)
    }

    fn write_texture(&self, src_region: RectF, transform: Mat4) -> Instruction {
        Instruction::TextureWrite(src_region, transform)
    }

    /// Returns true if no writes are rendered.
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }

    pub fn textures(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler.textures.iter().map(|tex| &**tex)
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs:
// https://github.com/rust-lang/rust/issues/44265
impl<P> Compilation<'_, P>
where
    P: SharedPointerKind,
{
    pub(super) fn next(&mut self) -> Option<Instruction> {
        if self.idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        Some(match self.compiler.code[idx] {
            Asm::BindTextureDescriptorSet(idx) => self.bind_texture_descriptor_set(idx),
            Asm::WriteTexture(src_tile, transform) => self.write_texture(src_tile, transform),
        })
    }
}

impl<P> Drop for Compilation<'_, P>
where
    P: SharedPointerKind,
{
    fn drop(&mut self) {
        // Reset non-critical resources
        self.compiler.code.clear();
    }
}

/// Compiles a series of drawing commands into renderable instructions. The purpose of this
/// structure is two-fold:
/// - Reduce per-draw allocations with line and light caches (they are not cleared after each use)
/// - Store references to the in-use mesh textures during rendering (this cache is cleared after
///   use)
pub struct Compiler<P>
where
    P: 'static + SharedPointerKind,
{
    cmds: Vec<Command<P>>,
    code: Vec<Asm>,
    textures: Vec<Shared<Texture2d, P>>,
}

impl<P> Compiler<P>
where
    P: SharedPointerKind,
{
    /// Compiles a given set of commands into a ready-to-draw list of instructions. Performs these
    /// steps:
    /// - Cull commands which might not be visible in the viewport (if the feature is enabled)
    /// - Sort commands by texture in order to reduce descriptor set switching/usage
    pub(super) unsafe fn compile<C, I>(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        cmds: I,
    ) -> Compilation<'_, P>
    where
        C: Borrow<Command<P>>,
        I: IntoIterator<Item = C>,
    {
        debug_assert!(self.code.is_empty());
        debug_assert!(self.textures.is_empty());

        for cmd in cmds.into_iter() {
            self.cmds.push(cmd.borrow().clone());
        }

        if self.cmds.is_empty() {
            warn!("Empty command list provided");

            return self.empty_compilation();
        }

        // When using auto-culling, we may reduce len in order to account for culled commands.
        #[cfg(feature = "auto-cull")]
        {
            let mut idx = 0;
            let mut len = self.cmds.len();

            // This loop operates on the unsorted command list and:
            // - Culls commands outside of the camera frustum (if the feature is enabled)
            while idx < len {
                // TODO: Implement this!
                let overlaps = true;

                if !overlaps {
                    // Auto-cull this command by swapping it into an area of the vec which we will
                    // discard at the end of this loop
                    len -= 1;
                    if len > 0 {
                        cmds.swap(idx, len);
                    }

                    continue;
                }

                idx += 1;
            }

            self.cmds.truncate(len);

            if self.cmds.is_empty() {
                return self.empty_compilation();
            }
        }

        // Rearrange the commands so draw order doesn't cause unnecessary resource-switching
        self.sort();

        self.code.push(Asm::BindTextureDescriptorSet(0));
        self.textures.push(Shared::clone(&self.cmds[0].src));

        for cmd in self.cmds.iter() {
            // Probably going to want this back in the future
            // let src = Shared::as_ptr(&cmd.src);
            // match self.textures.binary_search_by(|probe| Shared::as_ptr(probe).cmp(&src)) {
            //     Err(idx) => {
            //         self.textures.push(Shared::clone(&cmd.src));
            //     }
            //     Ok()
            // }
            let tex = self.textures.last().unwrap();
            if cmd.src != *tex {
                self.code
                    .push(Asm::BindTextureDescriptorSet(self.textures.len()));
                self.textures.push(Shared::clone(&cmd.src));
            }

            let src_dims: CoordF = cmd.src.dims().into();
            let mut src_tile: RectF = cmd.src_tile;
            src_tile.dims /= src_dims;
            src_tile.pos /= src_dims;

            self.code.push(Asm::WriteTexture(src_tile, cmd.transform));
        }

        Compilation {
            compiler: self,
            idx: 0,
        }
    }

    fn empty_compilation(&mut self) -> Compilation<'_, P> {
        Compilation {
            compiler: self,
            idx: 0,
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile`
    /// function.
    ///
    /// Must NOT be called before the previously drawn frame is completed.
    pub(super) fn reset(&mut self) {
        // Reset critical resources
        self.textures.clear();
    }

    /// Sorts commands into a predictable and efficient order for drawing.
    fn sort(&mut self) {
        // NOTE: Unstable sort because we don't claim to support ordering or blending of the
        // individual writes within each batch
        self.cmds.sort_unstable_by(|lhs, rhs| {
            let lhs = Shared::as_ptr(&lhs.src);
            let rhs = Shared::as_ptr(&rhs.src);
            lhs.cmp(&rhs)
        });
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            cmds: Default::default(),
            code: Default::default(),
            textures: Default::default(),
        }
    }
}
