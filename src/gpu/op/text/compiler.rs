use {
    super::{Command, Instruction},
    crate::{gpu::Texture2d, ptr::Shared},
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{borrow::Borrow, marker::PhantomData},
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[non_exhaustive]
pub enum Asm {}

pub struct Compilation<'a, 'c, C, P, T>
where
    C: Borrow<Command<'c, P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    __: PhantomData<T>,
    cmds: &'c [C],
    compiler: &'a mut Compiler<P>,
    idx: usize,
}

impl<'c, C, P, T> Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<'c, P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub fn bitmap_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler.bitmap_textures.iter().map(|tex| &**tex)
    }

    pub fn bitmap_outline_descriptors(&self) -> impl ExactSizeIterator<Item = &Texture2d> {
        self.compiler
            .bitmap_outline_textures
            .iter()
            .map(|tex| &**tex)
    }

    // fn bind_texture_descriptors(&self, idx: usize) -> Instruction {
    //     let src = Shared::as_ptr(&self.compiler.cmds[idx].src);
    //     let desc_set = self
    //         .compiler
    //         .textures
    //         .binary_search_by(|probe| Shared::as_ptr(&probe).cmp(&src))
    //         .unwrap();

    //     Instruction::TextureDescriptors(desc_set)
    // }

    // fn write_texture(&self, transform: Mat4) -> Instruction {
    //     Instruction::TextureWrite(transform)
    // }

    /// Returns true if no writes are rendered.
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs:
// https://github.com/rust-lang/rust/issues/44265
impl<'c, C, P, T> Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<'c, P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub(super) fn next(&mut self) -> Option<Instruction> {
        if self.idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        Some(match &self.compiler.code[idx] {
            // Asm::BindTextureDescriptors(idx) => self.bind_texture_descriptors(*idx),
            // Asm::WriteTexture(transform) => self.write_texture(*transform),
            _ => todo!(),
        })
    }
}

impl<'c, C, P, T> Drop for Compilation<'_, 'c, C, P, T>
where
    C: Borrow<Command<'c, P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
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
    bitmap_textures: Vec<Shared<Texture2d, P>>,
    bitmap_outline_textures: Vec<Shared<Texture2d, P>>,
    code: Vec<Asm>,
    //scalable_bufs: Vec<Shared<Texture2d, P>>,
}

impl<P> Compiler<P>
where
    P: SharedPointerKind,
{
    pub fn compile<'a, 'c, C, T>(
        &'a mut self,
        #[cfg(feature = "debug-names")] name: &str,
        cmds: &'c [C],
    ) -> Compilation<'a, 'c, C, P, T>
    where
        C: Borrow<Command<'c, P, T>>,
        T: AsRef<str>,
    {
        Compilation {
            __: PhantomData,
            cmds,
            compiler: self,
            idx: 0,
        }
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            bitmap_outline_textures: Default::default(),
            bitmap_textures: Default::default(),
            code: Default::default(),
        }
    }
}
