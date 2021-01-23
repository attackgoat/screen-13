use {
    super::{
        command::{Command, CommandIter, Material, ModelCommand},
        geom::{
            gen_line, gen_rect_light, gen_spotlight, LINE_STRIDE, POINT_LIGHT, POINT_LIGHT_LEN,
            RECT_LIGHT_STRIDE, SPOTLIGHT_STRIDE,
        },
        instruction::{
            DataComputeInstruction, DataCopyInstruction, DataTransferInstruction,
            DataWriteInstruction, DataWriteRefInstruction, Instruction, LightBindInstruction,
            LineDrawInstruction, MeshBindInstruction, MeshDrawInstruction,
            PointLightDrawInstruction, RectLightDrawInstruction, SpotlightDrawInstruction,
            VertexAttrsDescriptorsInstruction,
        },
        key::{Line, RectLight, Spotlight},
    },
    crate::{
        camera::Camera,
        gpu::{
            data::Mapping,
            def::CalcVertexAttrsComputeMode,
            op::{DirtyData, DirtyLruData, Lru, Stride},
            pool::Pool,
            Data, Lease, Model,
        },
        pak::IndexType,
        ptr::Shared,
    },
    a_r_c_h_e_r_y::SharedPointerKind,
    std::{
        borrow::Borrow,
        cell::Ref,
        cmp::{Ord, Ordering},
        ops::{Range, RangeFrom},
        ptr::copy_nonoverlapping,
    },
};

// TODO: Stop compaction after a certain number of cycles or % complete, maybe only 10%.

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance; it exists
// because we can't store references but we do want to cache the vector of instructions the compiler
// creates. Each `Asm` is just a pointer to the `cmds` slice provided by the client which actually
// contains the references. `Asm` also points to the leased `Data` held by `Compiler`.
#[non_exhaustive]
enum Asm {
    BeginCalcVertexAttrs(CalcVertexAttrsComputeMode),
    BeginLight,
    BeginModel,
    BeginRectLight,
    BeginSpotlight,
    BindModelBuffers(usize),
    BindModelDescriptors(usize),
    BindVertexAttrsDescriptors(BindVertexAttrsDescriptorsAsm),
    BindRectLightBuffer,
    BindSpotlightBuffer,
    TransferLineData,
    TransferRectLightData,
    TransferSpotlightData,
    CalcVertexAttrs(CalcVertexAttrsAsm),
    CopyLineVertices,
    CopyRectLightVertices,
    CopySpotlightVertices,
    DrawLines(u32),
    DrawModel(usize),
    DrawPointLights(Range<usize>),
    DrawRectLight((usize, usize)),
    DrawSpotlight((usize, usize)),
    DrawSunlights(Range<usize>),
    WriteLineVertices,
    WriteModelIndices(usize),
    WriteModelVertices(usize),
    WritePointLightVertices,
    WriteRectLightVertices,
    WriteSpotlightVertices,
}

struct BindVertexAttrsDescriptorsAsm {
    idx: usize,
    mode: CalcVertexAttrsComputeMode,
}

pub struct CalcVertexAttrsDescriptors<'a, P>
where
    P: SharedPointerKind,
{
    pub dst: Ref<'a, Lease<Data, P>>,
    pub dst_len: u64,
    pub idx_buf: Ref<'a, Lease<Data, P>>,
    pub idx_len: u64,
    pub src: &'a Lease<Data, P>,
    pub src_len: u64,
    pub write_mask: &'a Lease<Data, P>,
    pub write_mask_len: u64,
}

struct CalcVertexAttrsAsm {
    base_idx: u32,
    base_vertex: u32,
    dispatch: u32,
}

struct CalcVertexAttrsData<P>
where
    P: SharedPointerKind,
{
    /// Staging data (position + tex coord, optional joints + weights)
    buf: Lease<Data, P>,

    /// Command index
    idx: usize,

    /// Length of the staging data, in bytes
    len: u64,

    write_mask: Lease<Data, P>,
}

struct CalcVertexAttrsDescriptorsIter<'a, P>
where
    P: 'static + SharedPointerKind,
{
    cmds: &'a [Command<P>],
    data: &'a [CalcVertexAttrsData<P>],
    idx: usize,
    usage: &'a Vec<usize>,
}

impl<'a, P> ExactSizeIterator for CalcVertexAttrsDescriptorsIter<'a, P>
where
    P: SharedPointerKind,
{
    fn len(&self) -> usize {
        self.usage.len()
    }
}

impl<'a, P> Iterator for CalcVertexAttrsDescriptorsIter<'a, P>
where
    P: SharedPointerKind,
{
    type Item = CalcVertexAttrsDescriptors<'a, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.usage.get(self.idx).map(|idx| {
            self.idx += 1;

            // Get the vertex zattribute calculation data for this command index
            let src_idx = self
                .data
                .binary_search_by(|probe| probe.idx.cmp(&idx))
                .unwrap();
            let src = &self.data[src_idx];

            // Get the GPU model for this command index
            let cmd = &self.cmds[*idx].as_model().unwrap();
            let model = &*cmd.model;
            let idx_ty = model.idx_ty();
            let (idx_buf, idx_len) = model.idx_buf_ref();
            let (dst, dst_len) = model.vertex_buf_ref();

            // We didn't store the length of the write mask because we have the data to calculate
            // here
            let (part, shift) = match idx_ty {
                IndexType::U16 => (63, 6),
                IndexType::U32 => (31, 5),
            };
            let write_mask_len = (idx_len + part) >> shift << 2;

            CalcVertexAttrsDescriptors {
                dst,
                dst_len,
                idx_buf,
                idx_len,
                src: &src.buf,
                src_len: src.len,
                write_mask: &src.write_mask,
                write_mask_len,
            }
        })
    }
}

// TODO: The note below is good but reset is not enough, we need some sort of additional function to
// also drop the data, like and `undo` or `rollback`
/// Note: If the instructions produced by this command are not completed succesfully the state of
/// the `Compiler` instance will be undefined, and so `reset()` must be called on it. This is
/// because copy operations that don't complete will leave the buffers with incorrect data.
pub struct Compilation<'a, P>
where
    P: 'static + SharedPointerKind,
{
    compiler: &'a mut Compiler<P>,
    contains_lines: bool,
    idx: usize,
}

impl<P> Compilation<'_, P>
where
    P: SharedPointerKind,
{
    fn begin_calc_vertex_attrs(&self, mode: CalcVertexAttrsComputeMode) -> Instruction<'_, P> {
        Instruction::VertexAttrsBegin(mode)
    }

    fn bind_light<T: Stride>(buf: &DirtyData<T, P>) -> Instruction<'_, P> {
        Instruction::LightBind(LightBindInstruction {
            buf: &buf.data.current,
            buf_len: buf
                .usage
                .last()
                .map_or(0, |(offset, _)| offset + T::stride()),
        })
    }

    fn bind_model_buffers(&self, idx: usize) -> Instruction<'_, P> {
        let cmd = self.compiler.cmds[idx].as_model().unwrap();
        let idx_ty = cmd.model.idx_ty();
        let (idx_buf, idx_buf_len) = cmd.model.idx_buf_ref();
        let (vertex_buf, vertex_buf_len) = cmd.model.vertex_buf_ref();

        Instruction::MeshBind(MeshBindInstruction {
            idx_buf,
            idx_buf_len,
            idx_ty,
            vertex_buf,
            vertex_buf_len,
        })
    }

    fn bind_model_descriptors(&self, idx: usize) -> Instruction<'_, P> {
        let cmd = &self.compiler.cmds[idx].as_model().unwrap();
        let desc_set = self
            .compiler
            .materials
            .binary_search_by(|probe| probe.cmp(&cmd.material))
            .unwrap();

        Instruction::MeshDescriptors(desc_set)
    }

    fn bind_vertex_attrs_descriptors(
        &self,
        asm: &BindVertexAttrsDescriptorsAsm,
    ) -> Instruction<'_, P> {
        let usage = match asm.mode {
            CalcVertexAttrsComputeMode::U16 => &self.compiler.u16_vertex_cmds,
            CalcVertexAttrsComputeMode::U16_SKIN => &self.compiler.u16_skin_vertex_cmds,
            CalcVertexAttrsComputeMode::U32 => &self.compiler.u32_vertex_cmds,
            CalcVertexAttrsComputeMode::U32_SKIN => &self.compiler.u32_skin_vertex_cmds,
        };
        let desc_set = usage.binary_search(&asm.idx).unwrap();

        Instruction::VertexAttrsDescriptors(VertexAttrsDescriptorsInstruction {
            desc_set,
            mode: asm.mode,
        })
    }

    pub fn calc_vertex_attrs_u16_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors<'_, P>> {
        CalcVertexAttrsDescriptorsIter {
            data: &self.compiler.calc_vertex_attrs,
            cmds: &self.compiler.cmds,
            idx: 0,
            usage: &self.compiler.u16_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u16_skin_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors<'_, P>> {
        CalcVertexAttrsDescriptorsIter {
            data: &self.compiler.calc_vertex_attrs,
            cmds: &self.compiler.cmds,
            idx: 0,
            usage: &self.compiler.u16_skin_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u32_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors<'_, P>> {
        CalcVertexAttrsDescriptorsIter {
            cmds: &self.compiler.cmds,
            data: &self.compiler.calc_vertex_attrs,
            idx: 0,
            usage: &self.compiler.u32_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u32_skin_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors<'_, P>> {
        CalcVertexAttrsDescriptorsIter {
            cmds: &self.compiler.cmds,
            data: &self.compiler.calc_vertex_attrs,
            idx: 0,
            usage: &self.compiler.u32_skin_vertex_cmds,
        }
    }

    fn calc_vertex_attrs(&self, asm: &CalcVertexAttrsAsm) -> Instruction<'_, P> {
        Instruction::VertexAttrsCalc(DataComputeInstruction {
            base_idx: asm.base_idx,
            base_vertex: asm.base_vertex,
            dispatch: asm.dispatch,
        })
    }

    pub fn contains_lines(&self) -> bool {
        self.contains_lines
    }

    fn copy_vertices<T>(buf: &mut DirtyData<T, P>) -> Instruction<'_, P> {
        Instruction::VertexCopy(DataCopyInstruction {
            buf: &mut buf.data.current,
            ranges: buf.pending_copies.as_slice(),
        })
    }

    fn draw_lines(buf: &mut DirtyData<Line, P>, line_count: u32) -> Instruction<'_, P> {
        Instruction::LineDraw(LineDrawInstruction {
            buf: &mut buf.data.current,
            line_count,
        })
    }

    fn draw_model(&self, idx: usize) -> Instruction<'_, P> {
        let cmd = self.compiler.cmds[idx].as_model().unwrap();
        let meshes = cmd.model.meshes_filter_is(cmd.mesh_filter);

        Instruction::MeshDraw(MeshDrawInstruction {
            meshes,
            transform: cmd.transform,
        })
    }

    fn draw_point_lights(&self, range: Range<usize>) -> Instruction<'_, P> {
        let buf = self.compiler.point_light_buf.as_ref().unwrap();

        Instruction::PointLightDraw(PointLightDrawInstruction {
            buf,
            lights: CommandIter::new(&self.compiler.cmds[range]),
        })
    }

    fn draw_rect_light(&self, idx: usize, lru_idx: usize) -> Instruction<'_, P> {
        let light = self.compiler.cmds[idx].as_rect_light().unwrap();
        let lru = &self.compiler.rect_light.lru[lru_idx];
        let offset = (lru.offset / RECT_LIGHT_STRIDE as u64) as u32;

        Instruction::RectLightDraw(RectLightDrawInstruction { light, offset })
    }

    fn draw_spotlight(&self, idx: usize, lru_idx: usize) -> Instruction<'_, P> {
        let light = self.compiler.cmds[idx].as_spotlight().unwrap();
        let lru = &self.compiler.spotlight.lru[lru_idx];
        let offset = (lru.offset / SPOTLIGHT_STRIDE as u64) as u32;

        Instruction::SpotlightDraw(SpotlightDrawInstruction { light, offset })
    }

    fn draw_sunlights(&self, range: Range<usize>) -> Instruction<'_, P> {
        Instruction::SunlightDraw(CommandIter::new(&self.compiler.cmds[range]))
    }

    /// Returns true if no models or lines are rendered.
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }

    pub fn mesh_materials(&self) -> impl ExactSizeIterator<Item = &Material<P>> {
        self.compiler.materials.iter()
    }

    fn transfer_data<T>(buf: &mut DirtyData<T, P>) -> Instruction<'_, P> {
        let (src, src_len) = buf.data.previous.as_mut().unwrap();

        Instruction::DataTransfer(DataTransferInstruction {
            src,
            src_range: 0..*src_len,
            dst: &mut buf.data.current,
        })
    }

    fn write_light_vertices<T>(buf: &mut DirtyData<T, P>) -> Instruction<'_, P> {
        Instruction::VertexWrite(DataWriteInstruction {
            buf: &mut buf.data.current,
            range: buf.pending_write.as_ref().unwrap().clone(),
        })
    }

    fn write_model_indices(&self, idx: usize) -> Instruction<'_, P> {
        let cmd = self.compiler.cmds[idx].as_model().unwrap();
        let (buf, len) = cmd.model.idx_buf_mut();

        Instruction::IndexWriteRef(DataWriteRefInstruction { buf, range: 0..len })
    }

    fn write_model_vertices(&self, idx: usize) -> Instruction<'_, P> {
        let cmd = self.compiler.cmds[idx].as_model().unwrap();
        let (buf, len) = cmd.model.vertex_buf_mut();

        Instruction::VertexWriteRef(DataWriteRefInstruction { buf, range: 0..len })
    }

    fn write_point_light_vertices(&mut self) -> Instruction<'_, P> {
        Instruction::VertexWrite(DataWriteInstruction {
            buf: self.compiler.point_light_buf.as_mut().unwrap(),
            range: 0..POINT_LIGHT_LEN,
        })
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs:
// https://github.com/rust-lang/rust/issues/44265
impl<P> Compilation<'_, P>
where
    P: SharedPointerKind,
{
    pub(super) fn next(&mut self) -> Option<Instruction<'_, P>> {
        if self.idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        Some(match &self.compiler.code[idx] {
            Asm::BeginCalcVertexAttrs(mode) => self.begin_calc_vertex_attrs(*mode),
            Asm::BeginLight => Instruction::LightBegin,
            Asm::BeginModel => Instruction::MeshBegin,
            Asm::BeginRectLight => Instruction::RectLightBegin,
            Asm::BeginSpotlight => Instruction::SpotlightBegin,
            Asm::BindModelBuffers(idx) => self.bind_model_buffers(*idx),
            Asm::BindModelDescriptors(idx) => self.bind_model_descriptors(*idx),
            Asm::BindVertexAttrsDescriptors(asm) => self.bind_vertex_attrs_descriptors(asm),
            Asm::BindRectLightBuffer => {
                Self::bind_light(self.compiler.rect_light.buf.as_ref().unwrap())
            }
            Asm::BindSpotlightBuffer => {
                Self::bind_light(self.compiler.spotlight.buf.as_ref().unwrap())
            }
            Asm::CalcVertexAttrs(asm) => self.calc_vertex_attrs(asm),
            Asm::CopyLineVertices => Self::copy_vertices(self.compiler.line.buf.as_mut().unwrap()),
            Asm::CopyRectLightVertices => {
                Self::copy_vertices(self.compiler.rect_light.buf.as_mut().unwrap())
            }
            Asm::CopySpotlightVertices => {
                Self::copy_vertices(self.compiler.spotlight.buf.as_mut().unwrap())
            }
            Asm::DrawLines(count) => {
                Self::draw_lines(self.compiler.line.buf.as_mut().unwrap(), *count)
            }
            Asm::DrawModel(idx) => self.draw_model(*idx),
            Asm::DrawPointLights(range) => self.draw_point_lights(range.clone()),
            Asm::DrawRectLight((idx, light)) => self.draw_rect_light(*idx, *light),
            Asm::DrawSpotlight((idx, light)) => self.draw_spotlight(*idx, *light),
            Asm::DrawSunlights(range) => self.draw_sunlights(range.clone()),
            Asm::TransferLineData => Self::transfer_data(self.compiler.line.buf.as_mut().unwrap()),
            Asm::TransferRectLightData => {
                Self::transfer_data(self.compiler.rect_light.buf.as_mut().unwrap())
            }
            Asm::TransferSpotlightData => {
                Self::transfer_data(self.compiler.spotlight.buf.as_mut().unwrap())
            }
            Asm::WriteModelIndices(idx) => self.write_model_indices(*idx),
            Asm::WriteModelVertices(idx) => self.write_model_vertices(*idx),
            Asm::WritePointLightVertices => self.write_point_light_vertices(),
            Asm::WriteRectLightVertices => {
                Self::write_light_vertices(self.compiler.rect_light.buf.as_mut().unwrap())
            }
            Asm::WriteSpotlightVertices => {
                Self::write_light_vertices(self.compiler.spotlight.buf.as_mut().unwrap())
            }
            Asm::WriteLineVertices => {
                Self::write_light_vertices(self.compiler.line.buf.as_mut().unwrap())
            }
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
        self.compiler.u16_vertex_cmds.clear();
        self.compiler.u16_skin_vertex_cmds.clear();
        self.compiler.u32_vertex_cmds.clear();
        self.compiler.u32_skin_vertex_cmds.clear();
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
    line: DirtyLruData<Line, P>,
    materials: Vec<Material<P>>,
    point_light_buf: Option<Lease<Data, P>>,
    rect_light: DirtyLruData<RectLight, P>,
    rect_lights: Vec<RectLight>,
    spotlight: DirtyLruData<Spotlight, P>,
    spotlights: Vec<Spotlight>,

    // These store which command indices use which vertex attribute calculation type (sorted)
    u16_skin_vertex_cmds: Vec<usize>,
    u16_vertex_cmds: Vec<usize>,
    u32_skin_vertex_cmds: Vec<usize>,
    u32_vertex_cmds: Vec<usize>,

    // This stores the data (staging + write mask buffers) needed to cacluate additional vertex
    // attributes (normal + tangent)
    calc_vertex_attrs: Vec<CalcVertexAttrsData<P>>,
}

impl<P> Compiler<P>
where
    P: SharedPointerKind,
{
    /// Compiles a given set of commands into a ready-to-draw list of instructions. Performs these
    /// steps:
    /// - Cull commands which might not be visible to the camera (if the feature is enabled)
    /// - Sort commands into predictable groupings (opaque meshes, lights, transparent meshes,
    ///   lines)
    /// - Sort mesh commands further by texture(s) in order to reduce descriptor set switching/usage
    /// - Prepare a single buffer of all line and light vertices which can be copied to the GPU all
    ///   at once
    pub(super) unsafe fn compile<'a, C, I>(
        &'a mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        camera: &impl Camera,
        cmds: I,
    ) -> Compilation<'a, P>
    where
        C: Borrow<Command<P>>,
        I: IntoIterator<Item = C>,
    {
        debug_assert!(self.code.is_empty());
        debug_assert!(self.materials.is_empty());

        for cmd in cmds {
            self.cmds.push(cmd.borrow().clone());
        }

        if self.cmds.is_empty() {
            warn!("Empty command list provided");

            return self.empty_compilation();
        }

        let eye = -camera.eye();

        // When using auto-culling, we may reduce len in order to account for culled commands.
        let mut idx = 0;
        let len = self.cmds.len();

        #[cfg(feature = "auto-cull")]
        let mut len = len;

        // This loop operates on the unsorted command list and:
        // - Sets camera Z order
        // - Culls commands outside of the camera frustum (if the feature is enabled)
        while idx < len {
            let _overlaps = match &mut self.cmds[idx] {
                Command::Model(ref mut cmd) => {
                    // Assign a relative measure of distance from the camera for all mesh commands
                    // which allows us to submit draw commands in the best order for the z-buffering
                    // algorithm (we use a depth map with comparisons that discard covered
                    // fragments)
                    cmd.camera_order = cmd.transform.transform_vector3(eye).length_squared();

                    #[cfg(feature = "auto-cull")]
                    {
                        // TODO: Add some sort of caching which exploits temporal cohesion. Possibly
                        // as simple as not checking items for X runs, after a positive check?
                        camera.overlaps_sphere(cmd.model.bounds())
                    }
                    #[cfg(not(feature = "auto-cull"))]
                    #[allow(clippy::unused_unit)]
                    {
                        ()
                    }
                }
                Command::PointLight(_light) => {
                    #[cfg(feature = "auto-cull")]
                    {
                        camera.overlaps_sphere(_light.bounds())
                    }

                    #[cfg(not(feature = "auto-cull"))]
                    #[allow(clippy::unused_unit)]
                    {
                        ()
                    }
                }
                Command::RectLight(_light) => {
                    #[cfg(feature = "auto-cull")]
                    {
                        camera.overlaps_sphere(_light.bounds())
                    }

                    #[cfg(not(feature = "auto-cull"))]
                    #[allow(clippy::unused_unit)]
                    {
                        ()
                    }
                }
                Command::Spotlight(_light) => {
                    #[cfg(feature = "auto-cull")]
                    {
                        camera.overlaps_cone(_light.bounds())
                    }

                    #[cfg(not(feature = "auto-cull"))]
                    #[allow(clippy::unused_unit)]
                    {
                        ()
                    }
                }
                _ => continue,
            };

            #[cfg(feature = "auto-cull")]
            if !_overlaps {
                // Auto-cull this command by swapping it into an area of the slice which we will
                // discard at the end of this loop
                len -= 1;
                if len > 0 {
                    self.cmds.swap(idx, len);
                }

                continue;
            }

            idx += 1;
        }

        #[cfg(feature = "auto-cull")]
        if self.cmds.is_empty() {
            return self.empty_compilation();
        } else {
            self.cmds.truncate(len);
        }

        // Rearrange the commands so draw order doesn't cause unnecessary resource-switching
        Self::sort(&mut self.cmds);

        // Locate the groups - we know these `SearchIdx` values will not be found as they are gaps
        // in between the groups
        let search_group_idx = |range: RangeFrom<usize>, group: SearchIdx| -> usize {
            self.cmds[range]
                .binary_search_by(|probe| (Self::group_idx(probe) as usize).cmp(&(group as _)))
                .unwrap_err()
        };
        let point_light_idx = search_group_idx(0.., SearchIdx::PointLight);
        let rect_light_idx =
            point_light_idx + search_group_idx(point_light_idx.., SearchIdx::RectLight);
        let spotlight_idx =
            rect_light_idx + search_group_idx(rect_light_idx.., SearchIdx::Spotlight);
        let sunlight_idx = spotlight_idx + search_group_idx(spotlight_idx.., SearchIdx::Sunlight);
        let line_idx = spotlight_idx + search_group_idx(spotlight_idx.., SearchIdx::Line);
        let model_count = point_light_idx;
        let point_light_count = rect_light_idx - point_light_idx;
        let rect_light_count = spotlight_idx - rect_light_idx;
        let spotlight_count = sunlight_idx - spotlight_idx;
        let sunlight_count = line_idx - sunlight_idx;
        let line_count = self.cmds.len() - line_idx;

        // debug!("point_light_idx {}", point_light_idx);
        // debug!("rect_light_idx {}", rect_light_idx);
        // debug!("spotlight_idx {}", spotlight_idx);
        // debug!("sunlight_idx {}", sunlight_idx);
        // debug!("line_idx {}", line_idx);
        // debug!("model_count {}", model_count);
        // debug!("point_light_count {}", point_light_count);
        // debug!("rect_light_count {}", rect_light_count);
        // debug!("spotlight_count {}", spotlight_count);
        // debug!("sunlight_count {}", sunlight_count);
        // debug!("line_count {}", line_count);

        // Model drawing
        if model_count > 0 {
            self.compile_models(0..model_count);
        }

        // Emit 'start light drawing' assembly code
        self.code.push(Asm::BeginLight);

        // Point light drawing
        if point_light_count > 0 {
            let point_lights = point_light_idx..rect_light_idx;
            self.compile_point_lights(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                point_lights,
            );
        }

        // Rect light drawing
        if rect_light_count > 0 {
            let rect_lights = rect_light_idx..spotlight_idx;
            self.compile_rect_lights(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                rect_lights,
            );
        }

        // Spotlight drawing
        if spotlight_count > 0 {
            let spotlights = spotlight_idx..sunlight_idx;
            self.compile_spotlights(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                spotlights,
            );
        }

        // Sunlight drawing
        if sunlight_count > 0 {
            let sunlights = sunlight_idx..line_idx;
            self.code.push(Asm::DrawSunlights(sunlights));
        }

        // Line drawing
        if line_count > 0 {
            let lines = line_idx..self.cmds.len();
            self.compile_lines(
                #[cfg(feature = "debug-names")]
                name,
                pool,
                lines,
            );
        }

        Compilation {
            compiler: self,
            contains_lines: line_count > 0,
            idx: 0,
        }
    }

    unsafe fn compile_lines(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        range: Range<usize>,
    ) {
        // Allocate enough `buf` to hold everything in the existing cache and everything we could
        // possibly draw
        let line_count = range.end - range.start;
        self.line.alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} line vertex buffer", name),
            pool,
            (self.line.lru.len() * LINE_STRIDE + line_count * LINE_STRIDE) as _,
        );
        let buf = self.line.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferLineData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        buf.compact_cache(&mut self.line.lru, pool.lru_expiry);
        if !buf.pending_copies.is_empty() {
            self.code.push(Asm::CopyLineVertices);
        }

        // start..end is the back of the buffer where we push new lines
        let start = buf
            .usage
            .last()
            .map_or(0, |(offset, _)| offset + LINE_STRIDE as u64);
        let mut end = start;

        for cmd in self.cmds[range].iter() {
            let line = cmd.as_line().unwrap();
            let key = Line::hash(line);

            // Cache the vertices
            match self.line.lru.binary_search_by(|probe| probe.key.cmp(&key)) {
                Err(idx) => {
                    // Cache the vertices for this line segment
                    let new_end = end + LINE_STRIDE as u64;
                    let vertices = gen_line(&line.vertices);

                    {
                        let mut mapped_range =
                            buf.data.current.map_range_mut(end..new_end).unwrap();
                        copy_nonoverlapping(
                            vertices.as_ptr(),
                            mapped_range.as_mut_ptr(),
                            LINE_STRIDE,
                        );

                        Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                    }

                    // Create a new cache entry for this line segment
                    self.line
                        .lru
                        .insert(idx, Lru::new(key, end, pool.lru_expiry));
                    end = new_end;
                }
                Ok(idx) => self.line.lru[idx].expiry = pool.lru_expiry,
            }
        }

        // We may need to write these vertices from the CPU to the GPU
        if end > start {
            buf.pending_write = Some(start..end);
            self.code.push(Asm::WriteLineVertices);
        }

        // Produce the assembly code that will draw all lines at once
        self.code.push(Asm::DrawLines(line_count as _));
    }

    fn compile_models(&mut self, range: Range<usize>) {
        debug_assert!(!range.is_empty());

        let mut material: Option<&Material<P>> = None;
        let mut model: Option<&Shared<Model<P>, P>> = None;

        // Emit 'start model drawing' assembly code
        self.code.push(Asm::BeginModel);

        let mut vertex_calc_mode = None;
        for (idx, cmd) in self.cmds[range].iter().enumerate() {
            let cmd = cmd.as_model().unwrap();

            if let Some((buf, len, write_mask)) = cmd.model.take_pending_writes() {
                // Emit 'write model buffers' assembly codes
                self.code.push(Asm::WriteModelIndices(idx));
                self.code.push(Asm::WriteModelVertices(idx));

                // Store the instance of the leased data which contains the packed/staging vertices
                // (This lease will be returned to the pool after this operation completes)
                self.calc_vertex_attrs.insert(
                    self.calc_vertex_attrs
                        .binary_search_by(|probe| probe.idx.cmp(&idx))
                        .unwrap_err(),
                    CalcVertexAttrsData {
                        buf,
                        idx,
                        len,
                        write_mask,
                    },
                );

                let idx_ty = cmd.model.idx_ty();
                let mut vertex_calc_bind = true;

                for mesh in cmd.model.meshes() {
                    let mode = CalcVertexAttrsComputeMode {
                        idx_ty,
                        skin: mesh.is_animated(),
                    };

                    // Emit 'start vertex attribute calculations' assembly code when the mode
                    // changes
                    if match vertex_calc_mode {
                        None => true,
                        Some(m) if m != mode => true,
                        _ => false,
                    } {
                        self.code.push(Asm::BeginCalcVertexAttrs(mode));
                        vertex_calc_bind = true;
                        vertex_calc_mode = Some(mode);
                    }

                    // Emit 'the current compute descriptor set has changed' assembly code
                    if vertex_calc_bind {
                        self.code.push(Asm::BindVertexAttrsDescriptors(
                            BindVertexAttrsDescriptorsAsm { idx, mode },
                        ));
                        vertex_calc_bind = false;

                        // This keeps track of the fact that this command index uses the current
                        // calculation mode
                        let usage = match mode {
                            CalcVertexAttrsComputeMode::U16 => &mut self.u16_vertex_cmds,
                            CalcVertexAttrsComputeMode::U16_SKIN => &mut self.u16_skin_vertex_cmds,
                            CalcVertexAttrsComputeMode::U32 => &mut self.u32_vertex_cmds,
                            CalcVertexAttrsComputeMode::U32_SKIN => &mut self.u32_skin_vertex_cmds,
                        };
                        if let Err(usage_idx) = usage.binary_search(&idx) {
                            usage.insert(usage_idx, idx);
                        }
                    }

                    // Emit code to cause the normal and tangent vertex attributes of each mesh to
                    // be calculated (source is leased data, destination lives as long as the model
                    // does)
                    self.code.push(Asm::CalcVertexAttrs(CalcVertexAttrsAsm {
                        base_idx: mesh.indices.start,
                        base_vertex: mesh.vertex_offset() >> 2,
                        dispatch: (mesh.indices.end - mesh.indices.start) / 3,
                    }));
                }
            }

            // Emit 'the current graphics descriptor set has changed' assembly code
            if let Some(curr_material) = material.as_ref() {
                if *curr_material != &cmd.material {
                    // Cache a clone of this material if needed
                    if let Err(idx) = self
                        .materials
                        .binary_search_by(|probe| probe.cmp(&cmd.material))
                    {
                        self.materials.insert(idx, Material::clone(&cmd.material));
                    }

                    self.code.push(Asm::BindModelDescriptors(idx));
                    material = Some(&cmd.material);
                }
            } else {
                self.code.push(Asm::BindModelDescriptors(idx));
                material = Some(&cmd.material);
                self.materials.push(Material::clone(&cmd.material));
            }

            // Emit 'model buffers have changed' assembly code
            if let Some(curr_model) = model.as_ref() {
                if !Shared::ptr_eq(curr_model, &cmd.model) {
                    self.code.push(Asm::BindModelBuffers(idx));
                    model = Some(&cmd.model);
                }
            } else {
                self.code.push(Asm::BindModelBuffers(idx));
                model = Some(&cmd.model);
            }

            // Emit 'draw model' assembly code
            self.code.push(Asm::DrawModel(idx));
        }
    }

    unsafe fn compile_point_lights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        range: Range<usize>,
    ) {
        if self.point_light_buf.as_ref().is_none() {
            // Emit 'write point light vertices' assembly code (only when we don't yet have a
            // buffer)
            self.code.push(Asm::WritePointLightVertices);

            let mut buf = pool.data(
                #[cfg(feature = "debug-names")]
                &format!("{} point light vertex buffer", name),
                POINT_LIGHT_LEN,
            );

            {
                let mut mapped_range = buf.map_range_mut(0..POINT_LIGHT_LEN).unwrap();
                copy_nonoverlapping(
                    POINT_LIGHT.as_ptr(),
                    mapped_range.as_mut_ptr(),
                    POINT_LIGHT_LEN as _,
                );

                Mapping::flush(&mut mapped_range).unwrap();
            }

            self.point_light_buf = Some(buf);
        }

        // Emit 'draw this range of lights' assembly code (must happen after vertex write!)
        self.code.push(Asm::DrawPointLights(range));
    }

    unsafe fn compile_rect_lights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        range: Range<usize>,
    ) {
        assert!(self.rect_lights.is_empty());

        // Allocate enough `buf` to hold everything in the existing cache and everything we could
        // possibly draw
        self.rect_light.alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} rect light vertex buffer", name),
            pool,
            ((self.rect_light.lru.len() + range.end - range.start) * RECT_LIGHT_STRIDE) as _,
        );
        let buf = self.rect_light.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferRectLightData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        buf.compact_cache(&mut self.rect_light.lru, pool.lru_expiry);
        if !buf.pending_copies.is_empty() {
            self.code.push(Asm::CopyRectLightVertices);
        }

        // start..end is the back of the buffer where we push new lights
        let start = buf
            .usage
            .last()
            .map_or(0, |(offset, _)| offset + RECT_LIGHT_STRIDE as u64);
        let mut end = start;

        let write_idx = self.code.len();
        self.code.push(Asm::BeginRectLight);
        self.code.push(Asm::BindRectLightBuffer);

        // First we make sure all rectangular lights are in the lru data ...
        for cmd in self.cmds[range.clone()].iter_mut() {
            let light = cmd.as_rect_light_mut().unwrap();
            let (key, scale) = RectLight::quantize(light);
            self.rect_lights.push(key);
            light.scale = scale;

            match self
                .rect_light
                .lru
                .binary_search_by(|probe| probe.key.cmp(&key))
            {
                Err(idx) => {
                    // Cache the normalized geometry for this rectangular light
                    let new_end = end + RECT_LIGHT_STRIDE as u64;
                    let vertices = gen_rect_light(key.dims(), key.range(), key.radius());

                    {
                        let mut mapped_range =
                            buf.data.current.map_range_mut(end..new_end).unwrap();
                        copy_nonoverlapping(
                            vertices.as_ptr(),
                            mapped_range.as_mut_ptr(),
                            RECT_LIGHT_STRIDE,
                        );

                        Mapping::flush(&mut mapped_range).unwrap();
                    }

                    // Create new cache entries for this rectangular light
                    buf.usage.push((end, key));
                    self.rect_light
                        .lru
                        .insert(idx, Lru::new(key, end, pool.lru_expiry));
                    end = new_end;
                }
                Ok(idx) => {
                    self.rect_light.lru[idx].expiry = pool.lru_expiry;
                }
            }
        }

        // ... now we can draw them using index
        let base = range.start;
        for (idx, _) in self.cmds[range].iter().enumerate() {
            let key = self.rect_lights[idx];
            self.code.push(Asm::DrawRectLight((
                base + idx,
                self.rect_light
                    .lru
                    .binary_search_by(|probe| probe.key.cmp(&key))
                    .unwrap(),
            )));
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            buf.pending_write = Some(start..end);
            self.code.insert(write_idx, Asm::WriteRectLightVertices);
        }
    }

    unsafe fn compile_spotlights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        pool: &mut Pool<P>,
        range: Range<usize>,
    ) {
        assert!(self.spotlights.is_empty());

        // Allocate enough `buf` to hold everything in the existing cache and everything we could
        // possibly draw
        self.spotlight.alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} spotlight vertex buffer", name),
            pool,
            (self.spotlight.lru.len() * SPOTLIGHT_STRIDE
                + (range.end - range.start) * SPOTLIGHT_STRIDE) as _,
        );
        let buf = self.spotlight.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferSpotlightData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        buf.compact_cache(&mut self.spotlight.lru, pool.lru_expiry);
        if !buf.pending_copies.is_empty() {
            self.code.push(Asm::CopySpotlightVertices);
        }

        // start..end is the back of the buffer where we push new lights
        let start = buf
            .usage
            .last()
            .map_or(0, |(offset, _)| offset + SPOTLIGHT_STRIDE as u64);
        let mut end = start;

        let write_idx = self.code.len();
        self.code.push(Asm::BeginSpotlight);
        self.code.push(Asm::BindSpotlightBuffer);

        // First we make sure all spotlights are in the lru data ...
        for cmd in self.cmds[range.clone()].iter_mut() {
            let light = cmd.as_spotlight_mut().unwrap();
            let (key, scale) = Spotlight::quantize(light);
            self.spotlights.push(key);
            light.scale = scale;

            match self
                .spotlight
                .lru
                .binary_search_by(|probe| probe.key.cmp(&key))
            {
                Err(idx) => {
                    // Cache the normalized geometry for this spotlight
                    let new_end = end + SPOTLIGHT_STRIDE as u64;
                    let vertices = gen_spotlight(key.radius(), key.range());

                    {
                        let mut mapped_range =
                            buf.data.current.map_range_mut(end..new_end).unwrap();
                        copy_nonoverlapping(
                            vertices.as_ptr(),
                            mapped_range.as_mut_ptr(),
                            SPOTLIGHT_STRIDE,
                        );

                        Mapping::flush(&mut mapped_range).unwrap();
                    }

                    // Create a new cache entry for this spotlight
                    self.spotlight
                        .lru
                        .insert(idx, Lru::new(key, end, pool.lru_expiry));
                    end = new_end;
                }
                Ok(idx) => {
                    self.spotlight.lru[idx].expiry = pool.lru_expiry;
                }
            }
        }

        // ... now we can draw them using index
        let base = range.start;
        for (idx, _) in self.cmds[range].iter().enumerate() {
            let key = self.spotlights[idx];
            self.code.push(Asm::DrawSpotlight((
                base + idx,
                self.spotlight
                    .lru
                    .binary_search_by(|probe| probe.key.cmp(&key))
                    .unwrap(),
            )));
        }

        // We may need to write these vertices from the CPU to the GPU
        if start != end {
            buf.pending_write = Some(start..end);
            self.code.insert(write_idx, Asm::WriteSpotlightVertices);
        }
    }

    fn empty_compilation(&mut self) -> Compilation<'_, P> {
        Compilation {
            compiler: self,
            contains_lines: false,
            idx: 0,
        }
    }

    /// All commands sort into groups: first models, then lights, followed by lines.
    fn group_idx(cmd: &Command<P>) -> GroupIdx {
        match cmd {
            Command::Model(_) => GroupIdx::Model,
            Command::PointLight(_) => GroupIdx::PointLight,
            Command::RectLight(_) => GroupIdx::RectLight,
            Command::Spotlight(_) => GroupIdx::Spotlight,
            Command::Sunlight(_) => GroupIdx::Sunlight,
            Command::Line(_) => GroupIdx::Line,
        }
    }

    /// Models sort into sub-groups: static followed by animated.
    fn model_group_idx(cmd: &ModelCommand<P>) -> ModelGroupIdx {
        if cmd.pose.is_some() {
            ModelGroupIdx::Animated
        } else {
            ModelGroupIdx::Static
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile`
    /// function.
    ///
    /// Must NOT be called before the previously drawn frame is completed.
    pub(super) fn reset(&mut self) {
        // Reset critical resources
        self.materials.clear();
        self.rect_lights.clear();
        self.spotlights.clear();
        self.calc_vertex_attrs.clear();

        // Advance the least-recently-used caching algorithm one step forward
        self.line.step();
        self.rect_light.step();
        self.spotlight.step();
    }

    /// Sorts commands into a predictable and efficient order for drawing.
    fn sort(cmds: &mut [Command<P>]) {
        cmds.sort_unstable_by(|lhs, rhs| {
            use Ordering::Equal as eq;

            // Compare groups
            let lhs_group = Self::group_idx(lhs);
            let rhs_group = Self::group_idx(rhs);
            match lhs_group.cmp(&rhs_group) {
                eq => match lhs {
                    Command::Model(lhs) => {
                        let rhs = rhs.as_model().unwrap();

                        // Compare model groups (draw static meshes hoping to cover animated ones)
                        let lhs_group = Self::model_group_idx(lhs);
                        let rhs_group = Self::model_group_idx(rhs);
                        match lhs_group.cmp(&rhs_group) {
                            eq => {
                                // Compare models (reduce vertex/index buffer switching)
                                let lhs_model = Shared::as_ptr(&lhs.model);
                                let rhs_model = Shared::as_ptr(&rhs.model);
                                match lhs_model.cmp(&rhs_model) {
                                    eq => {
                                        // Compare materials (reduce descriptor set switching)
                                        match lhs.material.cmp(&rhs.material) {
                                            eq => {
                                                // Compare z-order (sorting in closer to further)
                                                lhs.camera_order
                                                    .partial_cmp(&rhs.camera_order)
                                                    .unwrap_or(eq)
                                            }
                                            ne => ne,
                                        }
                                    }
                                    ne => ne,
                                }
                            }
                            ne => ne,
                        }
                    }
                    _ => eq,
                },
                ne => ne,
            }
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
            line: Default::default(),
            materials: Default::default(),
            point_light_buf: Default::default(),
            rect_light: Default::default(),
            rect_lights: Default::default(),
            spotlight: Default::default(),
            spotlights: Default::default(),
            u16_skin_vertex_cmds: Default::default(),
            u16_vertex_cmds: Default::default(),
            u32_skin_vertex_cmds: Default::default(),
            u32_vertex_cmds: Default::default(),
            calc_vertex_attrs: Default::default(),
        }
    }
}

/// Evenly numbered because we use `SearchIdx` to quickly locate these groups while filling the
/// cache.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum GroupIdx {
    Model = 0,
    PointLight = 2,
    RectLight = 4,
    Spotlight = 6,
    Sunlight = 8,
    Line = 10,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ModelGroupIdx {
    Static = 0,
    Animated,
}

/// These oddly numbered indices are the spaces in between the `GroupIdx` values. This was more
/// efficient than finding the actual group index because we would have to walk to the front and
/// back of each group after any binary search in order to find the whole group.
#[derive(Clone, Copy)]
enum SearchIdx {
    PointLight = 1,
    RectLight = 3,
    Spotlight = 5,
    Sunlight = 7,
    Line = 9,
}
