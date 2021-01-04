use {
    super::{
        command::{Command, CommandIter, ModelCommand},
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
        key::{Line, RectLight, Spotlight, Stride},
        Material,
    },
    crate::{
        camera::Camera,
        gpu::{
            data::{CopyRange, Mapping},
            def::CalcVertexAttrsComputeMode,
            pool::Pool,
            Data, driver::Device, Lease, ModelRef,
        },
        pak::IndexType,
    },
    std::{
        cell::Ref,
        cmp::{Ord, Ordering},
        ops::{Range, RangeFrom},
        ptr::copy_nonoverlapping,
    },
};

// Always ask for a bigger cache capacity than needed; it reduces the need to completely replace
// the existing cache and then have to copy all the old data over.
const CACHE_CAPACITY_FACTOR: f32 = 2.0;

// TODO: Stop compaction after a certain number of cycles or % complete, maybe only 10%.

/// Used to keep track of data allocated during compilation and also the previous value which we will
/// copy over during the drawing operation.
struct Allocation<T> {
    current: T,
    previous: Option<(T, u64)>,
}

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance; it exists because we can't store references
// but we do want to cache the vector of instructions the compiler creates. Each `Asm` is just a pointer to the `cmds` slice
// provided by the client which actually contains the references. `Asm` also points to the leased `Data` held by `Compiler`.
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

pub struct CalcVertexAttrsDescriptors<'a> {
    pub dst: Ref<'a, Lease<Data>>,
    pub dst_len: u64,
    pub idx_buf: Ref<'a, Lease<Data>>,
    pub idx_len: u64,
    pub src: &'a Lease<Data>,
    pub src_len: u64,
    pub write_mask: &'a Lease<Data>,
    pub write_mask_len: u64,
}

struct CalcVertexAttrsAsm {
    base_idx: u32,
    base_vertex: u32,
    dispatch: u32,
}

struct CalcVertexAttrsData {
    /// Staging data (position + tex coord, optional joints + weights)
    buf: Lease<Data>,

    /// Command index
    idx: usize,

    /// Length of the staging data, in bytes
    len: u64,

    write_mask: Lease<Data>,
}

struct CalcVertexAttrsDescriptorsIter<'a> {
    cmds: &'a [Command],
    data: &'a [CalcVertexAttrsData],
    idx: usize,
    usage: &'a Vec<usize>,
}

impl<'a> ExactSizeIterator for CalcVertexAttrsDescriptorsIter<'a> {
    fn len(&self) -> usize {
        self.usage.len()
    }
}

impl<'a> Iterator for CalcVertexAttrsDescriptorsIter<'a> {
    type Item = CalcVertexAttrsDescriptors<'a>;

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
            let model = cmd.model.as_ref();
            let idx_ty = model.idx_ty();
            let (idx_buf, idx_len) = model.idx_buf_ref();
            let (dst, dst_len) = model.vertex_buf_ref();

            // We didn't store the length of the write mask because we have the data to calculate here
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

// TODO: The note below is good but reset is not enough, we need some sort of additional function to also drop the data, like and `undo` or `rollback`
/// Note: If the instructions produced by this command are not completed succesfully the state of the `Compiler` instance will
/// be undefined, and so `reset()` must be called on it. This is because copy operations that don't complete will leave the
/// buffers with incorrect data.
pub struct Compilation<'a> {
    cmds: &'a [Command],
    compiler: &'a mut Compiler,
    contains_lines: bool,
    idx: usize,
}

impl<'a> Compilation<'a> {
    fn begin_calc_vertex_attrs(mode: CalcVertexAttrsComputeMode) -> Instruction<'static> {
        Instruction::VertexAttrsBegin(mode)
    }

    fn bind_light<T: Stride>(buf: &DirtyData<T>) -> Instruction {
        Instruction::LightBind(LightBindInstruction {
            buf: &buf.data.current,
            buf_len: buf
                .gpu_usage
                .last()
                .map_or(0, |(offset, _)| offset + T::stride()),
        })
    }

    fn bind_model_buffers(&self, idx: usize) -> Instruction {
        let cmd = self.cmds[idx].as_model().unwrap();
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

    fn bind_model_descriptors(&self, idx: usize) -> Instruction {
        let cmd = &self.cmds[idx].as_model().unwrap();
        let desc_set = self
            .compiler
            .materials
            .binary_search_by(|probe| probe.cmp(&cmd.material))
            .unwrap();

        Instruction::MeshDescriptors(desc_set)
    }

    fn bind_vertex_attrs_descriptors(&self, asm: &BindVertexAttrsDescriptorsAsm) -> Instruction {
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
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors> {
        CalcVertexAttrsDescriptorsIter {
            data: &self.compiler.calc_vertex_attrs,
            cmds: &self.cmds,
            idx: 0,
            usage: &self.compiler.u16_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u16_skin_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors> {
        CalcVertexAttrsDescriptorsIter {
            data: &self.compiler.calc_vertex_attrs,
            cmds: &self.cmds,
            idx: 0,
            usage: &self.compiler.u16_skin_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u32_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors> {
        CalcVertexAttrsDescriptorsIter {
            cmds: &self.cmds,
            data: &self.compiler.calc_vertex_attrs,
            idx: 0,
            usage: &self.compiler.u32_vertex_cmds,
        }
    }

    pub fn calc_vertex_attrs_u32_skin_descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = CalcVertexAttrsDescriptors> {
        CalcVertexAttrsDescriptorsIter {
            cmds: &self.cmds,
            data: &self.compiler.calc_vertex_attrs,
            idx: 0,
            usage: &self.compiler.u32_skin_vertex_cmds,
        }
    }

    fn calc_vertex_attrs(&self, asm: &CalcVertexAttrsAsm) -> Instruction {
        Instruction::VertexAttrsCalc(DataComputeInstruction {
            base_idx: asm.base_idx,
            base_vertex: asm.base_vertex,
            dispatch: asm.dispatch,
        })
    }

    pub fn contains_lines(&self) -> bool {
        self.contains_lines
    }

    fn copy_vertices<T>(buf: &mut DirtyData<T>) -> Instruction {
        Instruction::VertexCopy(DataCopyInstruction {
            buf: &mut buf.data.current,
            ranges: buf.gpu_dirty.as_slice(),
        })
    }

    fn draw_lines(buf: &mut DirtyData<Line>, line_count: u32) -> Instruction {
        Instruction::LineDraw(LineDrawInstruction {
            buf: &mut buf.data.current,
            line_count,
        })
    }

    fn draw_model(&self, idx: usize) -> Instruction {
        let cmd = self.cmds[idx].as_model().unwrap();
        let meshes = cmd.model.meshes_filter_is(cmd.mesh_filter);

        Instruction::MeshDraw(MeshDrawInstruction {
            meshes,
            transform: cmd.transform,
        })
    }

    fn draw_point_lights(&self, range: Range<usize>) -> Instruction {
        let buf = self.compiler.point_light_buf.as_ref().unwrap();

        Instruction::PointLightDraw(PointLightDrawInstruction {
            buf,
            lights: CommandIter::new(&self.cmds[range]),
        })
    }

    fn draw_rect_light(&self, idx: usize, lru_idx: usize) -> Instruction {
        let light = self.cmds[idx].as_rect_light().unwrap();
        let lru = &self.compiler.rect_light.lru[lru_idx];
        let offset = (lru.offset / RECT_LIGHT_STRIDE as u64) as u32;

        Instruction::RectLightDraw(RectLightDrawInstruction { light, offset })
    }

    fn draw_spotlight(&self, idx: usize, lru_idx: usize) -> Instruction {
        let light = self.cmds[idx].as_spotlight().unwrap();
        let lru = &self.compiler.spotlight.lru[lru_idx];
        let offset = (lru.offset / SPOTLIGHT_STRIDE as u64) as u32;

        Instruction::SpotlightDraw(SpotlightDrawInstruction { light, offset })
    }

    fn draw_sunlights(&self, range: Range<usize>) -> Instruction {
        Instruction::SunlightDraw(CommandIter::new(&self.cmds[range]))
    }

    /// Returns true if no actual models or lines are rendered
    pub fn is_empty(&self) -> bool {
        self.compiler.code.is_empty()
    }

    pub fn mesh_materials(&self) -> impl ExactSizeIterator<Item = &Material> {
        self.compiler.materials.iter()
    }

    fn transfer_data<T>(buf: &mut DirtyData<T>) -> Instruction {
        let (src, src_len) = buf.data.previous.as_mut().unwrap();

        Instruction::DataTransfer(DataTransferInstruction {
            src,
            src_range: 0..*src_len,
            dst: &mut buf.data.current,
        })
    }

    fn write_light_vertices<T>(buf: &mut DirtyData<T>) -> Instruction {
        Instruction::VertexWrite(DataWriteInstruction {
            buf: &mut buf.data.current,
            range: buf.cpu_dirty.as_ref().unwrap().clone(),
        })
    }

    fn write_model_indices(&self, idx: usize) -> Instruction {
        let cmd = self.cmds[idx].as_model().unwrap();
        let (buf, len) = cmd.model.idx_buf_mut();

        Instruction::IndexWriteRef(DataWriteRefInstruction { buf, range: 0..len })
    }

    fn write_model_vertices(&self, idx: usize) -> Instruction {
        let cmd = self.cmds[idx].as_model().unwrap();
        let (buf, len) = cmd.model.vertex_buf_mut();

        Instruction::VertexWriteRef(DataWriteRefInstruction { buf, range: 0..len })
    }

    fn write_point_light_vertices(&mut self) -> Instruction {
        Instruction::VertexWrite(DataWriteInstruction {
            buf: self.compiler.point_light_buf.as_mut().unwrap(),
            range: 0..POINT_LIGHT_LEN,
        })
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs: https://github.com/rust-lang/rust/issues/44265
impl Compilation<'_> {
    pub(super) fn next(&mut self) -> Option<Instruction> {
        if self.idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.idx;
        self.idx += 1;

        Some(match &self.compiler.code[idx] {
            Asm::BeginCalcVertexAttrs(mode) => Self::begin_calc_vertex_attrs(*mode),
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

impl Drop for Compilation<'_> {
    fn drop(&mut self) {
        // Reset non-critical resources
        self.compiler.code.clear();
        self.compiler.u16_vertex_cmds.clear();
        self.compiler.u16_skin_vertex_cmds.clear();
        self.compiler.u32_vertex_cmds.clear();
        self.compiler.u32_skin_vertex_cmds.clear();
    }
}

/// Compiles a series of drawing commands into renderable instructions. The purpose of this structure is
/// two-fold:
/// - Reduce per-draw allocations with line and light caches (they are not cleared after each use)
/// - Store references to the in-use mesh textures during rendering (this cache is cleared after use)
#[derive(Default)]
pub struct Compiler {
    code: Vec<Asm>,
    line: DirtyLruData<Line>,
    materials: Vec<Material>,
    point_light_buf: Option<Lease<Data>>,
    rect_light: DirtyLruData<RectLight>,
    rect_lights: Vec<RectLight>,
    spotlight: DirtyLruData<Spotlight>,
    spotlights: Vec<Spotlight>,

    // These store which command indices use which vertex attribute calculation type (sorted)
    u16_vertex_cmds: Vec<usize>,
    u16_skin_vertex_cmds: Vec<usize>,
    u32_vertex_cmds: Vec<usize>,
    u32_skin_vertex_cmds: Vec<usize>,

    // This stores the data (staging + write mask buffers) needed to cacluate additional vertex
    // attributes (normal + tangent)
    calc_vertex_attrs: Vec<CalcVertexAttrsData>,
}

impl Compiler {
    /// Allocates or re-allocates leased data of the given size. This could be a function of the DirtyData type, however it only
    /// works because the Compiler happens to know that the host-side of the data
    fn alloc_data<T: Stride>(
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        buf: &mut Option<DirtyData<T>>,
        len: u64,
    ) {
        #[cfg(feature = "debug-names")]
        if let Some(buf) = buf.as_mut() {
            buf.data.current.set_name(&name);
        }

        // Early-our if we do not need to resize the buffer
        if let Some(existing) = buf.as_ref() {
            if len <= existing.data.current.capacity() {
                return;
            }
        }

        #[cfg(debug_assertions)]
        {
            info!(
                "Reallocating {} to {}",
                buf.as_ref().map_or(0, |buf| buf.data.current.capacity()),
                len
            );
        }

        // We over-allocate the requested capacity to prevent rapid reallocations
        let capacity = (len as f32 * CACHE_CAPACITY_FACTOR) as u64;
        let data = pool.data(
            #[cfg(feature = "debug-names")]
            &name,
            device,
            capacity,
        );

        if let Some(old_buf) = buf.replace(data.into()) {
            // Preserve the old data so that we can copy it directly over before drawing
            let old_buf_len = old_buf
                .gpu_usage
                .last()
                .map_or(0, |(offset, _)| offset + T::stride());
            let new_buf = &mut buf.as_mut().unwrap();
            new_buf.gpu_usage = old_buf.gpu_usage;
            new_buf.data.previous = Some((old_buf.data.current, old_buf_len));
        }
    }

    /// Moves cache items into clumps so future items can be appended onto the end without needing to
    /// resize the cache buffer. As a side effect this causes dirty regions to be moved on the GPU.
    ///
    /// Geometry used very often will end up closer to the beginning of the GPU memory over time, and
    /// will have fewer move operations applied to it as a result.
    fn compact_cache<T: Stride>(buf: &mut DirtyData<T>, lru: &mut Vec<Lru<T>>)
    where
        T: Ord,
    {
        let stride = T::stride();

        // "Forget about" GPU memory regions occupied by unused geometry
        buf.gpu_usage.retain(|(_, key)| {
            let idx = lru
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            lru[idx].recently_used > 0
        });

        // We only need to compact the memory in the region preceding the dirty region, because that geometry will
        // be uploaded and used during this compilation (draw) - we will defer that region to the next compilation
        let mut start = 0;
        let end = buf.cpu_dirty.as_ref().map_or_else(
            || {
                buf.gpu_usage
                    .last()
                    .map_or(0, |(offset, _)| offset + stride)
            },
            |dirty| dirty.start,
        );

        // Walk through the GPU memory in order, moving items back to the "empty" region and as we go
        for (offset, key) in &mut buf.gpu_usage {
            // Early out if we have exceeded the non-dirty region
            if *offset >= end {
                break;
            }

            // Skip items which should not be moved
            if start == *offset {
                start += stride;
                continue;
            }

            // Move this item back to the beginning of the empty region
            if let Some(range) = buf.gpu_dirty.last_mut() {
                if range.dst == start - stride && range.src.end == *offset - stride {
                    *range = CopyRange {
                        dst: range.dst,
                        src: range.src.start..*offset + stride,
                    };
                } else {
                    buf.gpu_dirty.push(CopyRange {
                        dst: start,
                        src: *offset..*offset + stride,
                    });
                }
            } else {
                buf.gpu_dirty.push(CopyRange {
                    dst: start,
                    src: *offset..*offset + stride,
                });
            }

            // Update the LRU item for this geometry
            let idx = lru
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            lru[idx].offset = start;

            start += stride;
        }
    }

    /// Compiles a given set of commands into a ready-to-draw list of instructions. Performs these steps:
    /// - Cull commands which might not be visible to the camera
    /// - Sort commands into predictable groupings (opaque meshes, lights, transparent meshes, lines)
    /// - Sort mesh commands further by texture(s) in order to reduce descriptor set switching/usage
    /// - Prepare a single buffer of all line and light vertices which can be copied to the GPU all at once
    pub(super) fn compile<'a, 'b: 'a>(
        &'a mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        camera: &impl Camera,
        cmds: &'b mut [Command],
    ) -> Compilation<'a> {
        assert!(self.code.is_empty());
        assert!(self.materials.is_empty());
        assert!(!cmds.is_empty());

        // Set model-specific things
        let eye = -camera.eye();
        for cmd in cmds.iter_mut() {
            if let Command::Model(cmd) = cmd {
                // Assign a relative measure of distance from the camera for all mesh commands which allows us to submit draw commands
                // in the best order for the z-buffering algorithm (we use a depth map with comparisons that discard covered fragments)
                cmd.camera_order = cmd.transform.transform_vector3(eye).length_squared();
            }
        }

        // Rearrange the commands so draw order doesn't cause unnecessary resource-switching
        self.sort(cmds);

        // Locate the groups - we know these `SearchIdx` values will not be found as they are gaps in between the groups
        let search_group_idx = |range: RangeFrom<usize>, group: SearchIdx| -> usize {
            cmds[range]
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
        let line_count = cmds.len() - line_idx;

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
            self.compile_models(&cmds[0..model_count]);
        }

        // Emit 'start light drawing' assembly code
        self.code.push(Asm::BeginLight);

        // Point light drawing
        if point_light_count > 0 {
            self.compile_point_lights(
                #[cfg(feature = "debug-names")]
                name,
                device,
                pool,
                point_light_idx..rect_light_idx,
            );
        }

        // Rect light drawing
        if rect_light_count > 0 {
            let rect_lights = rect_light_idx..spotlight_idx;
            self.compile_rect_lights(
                #[cfg(feature = "debug-names")]
                name,
                device,
                pool,
                &cmds[rect_lights],
                rect_light_idx,
            );
        }

        // Spotlight drawing
        if spotlight_count > 0 {
            let spotlights = spotlight_idx..sunlight_idx;
            self.compile_spotlights(
                #[cfg(feature = "debug-names")]
                name,
                device,
                pool,
                &cmds[spotlights],
                spotlight_idx,
            );
        }

        // Sunlight drawing
        if sunlight_count > 0 {
            let sunlights = sunlight_idx..line_idx;
            self.code.push(Asm::DrawSunlights(sunlights));
        }

        // Line drawing
        if line_count > 0 {
            let lines = line_idx..cmds.len();
            self.compile_lines(
                #[cfg(feature = "debug-names")]
                name,
                device,
                pool,
                &cmds[lines],
            );
        }

        Compilation {
            cmds,
            compiler: self,
            contains_lines: line_count > 0,
            idx: 0,
        }
    }

    fn compile_lines(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        cmds: &[Command],
    ) {
        // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
        Self::alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} line vertex buffer", name),
            device,
            pool,
            &mut self.line.buf,
            (self.line.lru.len() * LINE_STRIDE + cmds.len() * LINE_STRIDE) as _,
        );
        let buf = self.line.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferLineData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        Self::compact_cache(buf, &mut self.line.lru);
        if !buf.gpu_dirty.is_empty() {
            self.code.push(Asm::CopyLineVertices);
        }

        // start..end is the back of the buffer where we push new lines
        let start = buf
            .gpu_usage
            .last()
            .map_or(0, |(offset, _)| offset + LINE_STRIDE as u64);
        let mut end = start;

        for cmd in cmds.iter() {
            let line = cmd.as_line().unwrap();
            let key = Line::hash(line);

            // Cache the vertices
            match self.line.lru.binary_search_by(|probe| probe.key.cmp(&key)) {
                Err(idx) => {
                    // Cache the vertices for this line segment
                    let new_end = end + LINE_STRIDE as u64;
                    let vertices = gen_line(&line.vertices);

                    unsafe {
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
                        .insert(idx, Lru::new(key, end, pool.lru_threshold));
                    end = new_end;
                }
                Ok(idx) => self.line.lru[idx].recently_used = pool.lru_threshold,
            }
        }

        // We may need to copy these vertices from the CPU to the GPU
        if end > start {
            buf.cpu_dirty = Some(start..end);
            self.code.push(Asm::WriteLineVertices);
        }

        // Produce the assembly code that will draw all lines at once
        self.code.push(Asm::DrawLines(cmds.len() as _));
    }

    fn compile_models(&mut self, cmds: &[Command]) {
        debug_assert!(!cmds.is_empty());

        let mut material: Option<&Material> = None;
        let mut model: Option<&ModelRef> = None;

        // Emit 'start model drawing' assembly code
        self.code.push(Asm::BeginModel);

        let mut vertex_calc_mode = None;
        for (idx, cmd) in cmds.iter().enumerate() {
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

                    // Emit 'start vertex attribute calculations' assembly code when the mode changes
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

                        // This keeps track of the fact that this command index uses the current calculation mode
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

                    // Emit code to cause the normal and tangent vertex attributes of each mesh to be
                    // calculated (source is leased data, destination lives as long as the model does)
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
                if !ModelRef::ptr_eq(curr_model, &cmd.model) {
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

    fn compile_point_lights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        range: Range<usize>,
    ) {
        if self.point_light_buf.as_ref().is_none() {
            // Emit 'write point light vertices' assembly code (only when we don't yet have a buffer)
            self.code.push(Asm::WritePointLightVertices);

            let mut buf = pool.data(
                #[cfg(feature = "debug-names")]
                &format!("{} point light vertex buffer", name),
                device,
                POINT_LIGHT_LEN,
            );

            unsafe {
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

    fn compile_rect_lights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        cmds: &[Command],
        base_idx: usize,
    ) {
        assert!(self.rect_lights.is_empty());

        // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
        Self::alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} rect light vertex buffer", name),
            device,
            pool,
            &mut self.rect_light.buf,
            ((self.rect_light.lru.len() + cmds.len()) * RECT_LIGHT_STRIDE) as _,
        );
        let buf = self.rect_light.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferRectLightData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        Self::compact_cache(buf, &mut self.rect_light.lru);
        if !buf.gpu_dirty.is_empty() {
            self.code.push(Asm::CopyRectLightVertices);
        }

        // start..end is the back of the buffer where we push new lights
        let start = buf
            .gpu_usage
            .last()
            .map_or(0, |(offset, _)| offset + RECT_LIGHT_STRIDE as u64);
        let mut end = start;

        let write_idx = self.code.len();
        self.code.push(Asm::BeginRectLight);
        self.code.push(Asm::BindRectLightBuffer);

        // First we make sure all rectangular lights are in the lru data ...
        for cmd in cmds.iter() {
            let key = RectLight::quantize(cmd.as_rect_light().unwrap());
            self.rect_lights.push(key);

            match self
                .rect_light
                .lru
                .binary_search_by(|probe| probe.key.cmp(&key))
            {
                Err(idx) => {
                    // Cache the normalized geometry for this rectangular light
                    let new_end = end + RECT_LIGHT_STRIDE as u64;
                    let vertices = gen_rect_light(key.dims(), key.range(), key.radius());

                    unsafe {
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
                    buf.gpu_usage.push((end, key));
                    self.rect_light
                        .lru
                        .insert(idx, Lru::new(key, end, pool.lru_threshold));
                    end = new_end;
                }
                Ok(idx) => {
                    self.rect_light.lru[idx].recently_used = pool.lru_threshold;
                }
            }
        }

        // ... now we can draw them using index
        for (idx, _) in cmds.iter().enumerate() {
            let key = self.rect_lights[idx];
            self.code.push(Asm::DrawRectLight((
                base_idx + idx,
                self.rect_light
                    .lru
                    .binary_search_by(|probe| probe.key.cmp(&key))
                    .unwrap(),
            )));
        }

        // We may need to copy these vertices from the CPU to the GPU
        if start != end {
            buf.cpu_dirty = Some(start..end);
            self.code.insert(write_idx, Asm::WriteRectLightVertices);
        }
    }

    fn compile_spotlights(
        &mut self,
        #[cfg(feature = "debug-names")] name: &str,
        device: Device,
        pool: &mut Pool,
        cmds: &[Command],
        base_idx: usize,
    ) {
        assert!(self.spotlights.is_empty());

        // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
        Self::alloc_data(
            #[cfg(feature = "debug-names")]
            &format!("{} spotlight vertex buffer", name),
            device,
            pool,
            &mut self.spotlight.buf,
            (self.spotlight.lru.len() * SPOTLIGHT_STRIDE + cmds.len() * SPOTLIGHT_STRIDE) as _,
        );
        let buf = self.spotlight.buf.as_mut().unwrap();

        // Copy data from the previous GPU buffer to the new one
        if buf.data.previous.is_some() {
            self.code.push(Asm::TransferSpotlightData);
        }

        // Copy data from the uncompacted end of the buffer back to linear data
        Self::compact_cache(buf, &mut self.spotlight.lru);
        if !buf.gpu_dirty.is_empty() {
            self.code.push(Asm::CopySpotlightVertices);
        }

        // start..end is the back of the buffer where we push new lights
        let start = buf
            .gpu_usage
            .last()
            .map_or(0, |(offset, _)| offset + SPOTLIGHT_STRIDE as u64);
        let mut end = start;

        let write_idx = self.code.len();
        self.code.push(Asm::BeginSpotlight);
        self.code.push(Asm::BindSpotlightBuffer);

        // First we make sure all spotlights are in the lru data ...
        for cmd in cmds.iter() {
            let key = Spotlight::quantize(cmd.as_spotlight().unwrap());
            self.spotlights.push(key);

            match self
                .spotlight
                .lru
                .binary_search_by(|probe| probe.key.cmp(&key))
            {
                Err(idx) => {
                    // Cache the normalized geometry for this spotlight
                    let new_end = end + SPOTLIGHT_STRIDE as u64;
                    let vertices = gen_spotlight(key.radius(), key.range());

                    unsafe {
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
                        .insert(idx, Lru::new(key, end, pool.lru_threshold));
                    end = new_end;
                }
                Ok(idx) => {
                    self.spotlight.lru[idx].recently_used = pool.lru_threshold;
                }
            }
        }

        // ... now we can draw them using index
        for (idx, _) in cmds.iter().enumerate() {
            let key = self.spotlights[idx];
            self.code.push(Asm::DrawSpotlight((
                base_idx + idx,
                self.spotlight
                    .lru
                    .binary_search_by(|probe| probe.key.cmp(&key))
                    .unwrap(),
            )));
        }

        // We may need to copy these vertices from the CPU to the GPU
        if start != end {
            buf.cpu_dirty = Some(start..end);
            self.code.insert(write_idx, Asm::WriteSpotlightVertices);
        }
    }

    /// All commands sort into groups: first models, then lights, followed by lines.
    fn group_idx(cmd: &Command) -> GroupIdx {
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
    fn model_group_idx(cmd: &ModelCommand) -> ModelGroupIdx {
        if cmd.pose.is_some() {
            ModelGroupIdx::Animated
        } else {
            ModelGroupIdx::Static
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile` function.
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
    fn sort(&mut self, cmds: &mut [Command]) {
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
                                let lhs_model = ModelRef::as_ptr(&lhs.model);
                                let rhs_model = ModelRef::as_ptr(&rhs.model);
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

/// Extends the data type so we can track which portions require updates. Does not teach an entire city full
/// of people that dancing is the best thing there is.
struct DirtyData<Key> {
    cpu_dirty: Option<Range<u64>>, // This range, if present, is the portion that needs to be copied from cpu to gpu
    data: Allocation<Lease<Data>>,
    gpu_dirty: Vec<CopyRange>, // Segments of gpu memory which must be "compacted" (read: copied) within the gpu
    gpu_usage: Vec<(u64, Key)>, // Memory usage on the gpu, sorted by the first field which is the offset.
}

impl<Key> DirtyData<Key> {
    fn reset(&mut self) {
        self.cpu_dirty = None;
        self.gpu_dirty.clear();
    }
}

impl<T> From<Lease<Data>> for DirtyData<T> {
    fn from(val: Lease<Data>) -> Self {
        Self {
            cpu_dirty: None,
            data: Allocation {
                current: val,
                previous: None,
            },
            gpu_dirty: vec![],
            gpu_usage: vec![],
        }
    }
}

struct DirtyLruData<Key> {
    buf: Option<DirtyData<Key>>,
    lru: Vec<Lru<Key>>,
}

impl<K> DirtyLruData<K> {
    fn step(&mut self) {
        if let Some(buf) = self.buf.as_mut() {
            buf.reset();
        }

        // TODO: This should keep a 'frame' value per item and just increment a single 'age' value, O(1) not O(N)!
        for item in self.lru.iter_mut() {
            item.recently_used = item.recently_used.saturating_sub(1);
        }
    }
}

// #[derive(Default)] did not work due to Key being unconstrained
impl<Key> Default for DirtyLruData<Key> {
    fn default() -> Self {
        Self {
            buf: None,
            lru: vec![],
        }
    }
}

/// Evenly numbered because we use `SearchIdx` to quickly locate these groups while filling the cache.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum GroupIdx {
    Model = 0,
    PointLight = 2,
    RectLight = 4,
    Spotlight = 6,
    Sunlight = 8,
    Line = 10,
}

/// Individual item of a least-recently-used cache vector. Allows tracking the usage of a key which lives at some memory offset.
struct Lru<T> {
    key: T,
    offset: u64,
    recently_used: usize,
}

impl<T> Lru<T> {
    fn new(key: T, offset: u64, lru_threshold: usize) -> Self {
        Self {
            key,
            offset,
            recently_used: lru_threshold,
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ModelGroupIdx {
    Static = 0,
    Animated,
}

/// These oddly numbered indices are the spaces in between the `GroupIdx` values. This was more efficient than
/// finding the actual group index because we would have to walk to the front and back of each group after any
/// binary search in order to find the whole group.
#[derive(Clone, Copy)]
enum SearchIdx {
    PointLight = 1,
    RectLight = 3,
    Spotlight = 5,
    Sunlight = 7,
    Line = 9,
}
