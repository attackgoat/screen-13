use {
    super::{
        command::{BitmapCommand, Command, ScalableCommand},
        instruction::Instruction,
    },
    crate::{
        gpu::{
            op::{Allocation, DirtyData, DirtyLruData, Lru},
            Texture2d,
        },
        math::{CoordF, Extent, Mat4},
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{borrow::Borrow, marker::PhantomData},
};

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance.
#[non_exhaustive]
pub enum Asm {}

pub struct Compilation<'a, 'c, C, P, T>
where
    C: Borrow<Command<P, T>>,
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
    C: Borrow<Command<P, T>>,
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
    C: Borrow<Command<P, T>>,
    P: 'static + SharedPointerKind,
    T: AsRef<str>,
{
    pub(super) fn next(&mut self) -> Option<Instruction<'_, P>> {
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
    C: Borrow<Command<P, T>>,
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
    bitmap: DirtyLruData<char, P>,
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
        dims: Extent,
    ) -> Compilation<'a, 'c, C, P, T>
    where
        C: Borrow<Command<P, T>>,
        T: AsRef<str>,
    {
        // For all commands:
        // - Bitmap: Check each letter
        //           Tesselate into 4 triangle strip vertices
        //           Store vertices in LRU keyed by letter
        //           Put vertices in data, write data cpu->gpu
        //           Hash each letter and transform
        //           First time seeing a letter/position: Draw it, prepare instancing
        //           Second time seeing a letter/position: Instance it

        for cmd in cmds.iter() {
            match cmd.borrow() {
                Command::Position(cmd) => self.compile_bitmap_position(
                    #[cfg(feature = "debug-names")]
                    name,
                    cmd,
                    dims,
                ),
                Command::SizePosition(cmd) => self.compile_scalable_position(
                    #[cfg(feature = "debug-names")]
                    name,
                    cmd,
                    dims,
                ),
                Command::SizeTransform(cmd) => self.compile_scalable_transform(
                    #[cfg(feature = "debug-names")]
                    name,
                    cmd,
                    dims,
                ),
                Command::Transform(cmd) => self.compile_bitmap_transform(
                    #[cfg(feature = "debug-names")]
                    name,
                    cmd,
                    dims,
                ),
            }
        }

        // // PERF: Should hand roll this
        // // Read as:
        // // 1. Convert layout pixels to normalized coordinates:  pixels ->  0..1
        // // 2. Transform normalized coordinates to NDC:          0..1   -> -1..1
        // Mat4::from_translation(vec3(-1.0, -1.0, 0.0))
        //     * Mat4::from_scale(vec3(2.0, 2.0, 1.0))
        //     * Mat4::from_translation(vec3(
        //         self.layout.x / dims.x as f32,
        //         self.layout.y / dims.y as f32,
        //         0.0,
        //     ))

        Compilation {
            __: PhantomData,
            cmds,
            compiler: self,
            idx: 0,
        }
    }

    fn compile_bitmap_position<T>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        cmd: &BitmapCommand<CoordF, P, T>,
        dims: Extent,
    ) where
        T: AsRef<str>,
    {
    }

    fn compile_bitmap_transform<T>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        cmd: &BitmapCommand<Mat4, P, T>,
        dims: Extent,
    ) where
        T: AsRef<str>,
    {
    }

    fn compile_scalable_position<T>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        cmd: &ScalableCommand<CoordF, P, T>,
        dims: Extent,
    ) where
        T: AsRef<str>,
    {
    }

    fn compile_scalable_transform<T>(
        &self,
        #[cfg(feature = "debug-names")] name: &str,
        cmd: &ScalableCommand<Mat4, P, T>,
        dims: Extent,
    ) where
        T: AsRef<str>,
    {
    }
}

impl<P> Default for Compiler<P>
where
    P: SharedPointerKind,
{
    fn default() -> Self {
        Self {
            bitmap: Default::default(),
            bitmap_outline_textures: Default::default(),
            bitmap_textures: Default::default(),
            code: Default::default(),
        }
    }
}
