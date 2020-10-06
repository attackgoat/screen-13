use {
    super::{
        geom::{
            gen_line, gen_rect_light, gen_spotlight, LINE_STRIDE, POINT_LIGHT, RECT_LIGHT_STRIDE,
            SPOTLIGHT_STRIDE,
        },
        instruction::Instruction,
        key::{Line, RectLight, Spotlight},
        Command,
    },
    crate::{
        camera::Camera,
        gpu::{
            data::{CopyRange, Mapping},
            Data, Lease, Model, PoolRef, Texture2d,
        },
    },
    std::{
        cmp::{Ord, Ordering},
        collections::BTreeSet,
        mem::take,
        ops::Range,
        ptr::copy_nonoverlapping,
    },
};

// Always ask for a bigger cache capacity than needed; it reduces the need to completely replace
// the existing cache and then have to copy all the old data over.
const CACHE_CAPACITY_FACTOR: f32 = 2.0;

// TODO: This code is ready to accept the addition of index buffers, once things stabilize it would be fantastic to add this
// TODO: Maybe store 'LRU' as a number, 4 or so? Right now it's a bool so if you don't use something each frame it gets removed.
// TODO: Also stop compaction after a certain number of cycles or % complete, maybe only 10%.

/// Used to keep track of data allocated during compilation and also the previous value which we will
/// copy over during the drawing operation.
struct Allocation<T> {
    current: T,
    previous: Option<T>,
}

// `Asm` is the "assembly op code" that is used to create an `Instruction` instance; it exists because we can't store references
// but we do want to cache the vector of instructions the compiler creates. Each `Asm` is just a pointer to the `cmds` slice
// provided by the client which actually contains the references. `Asm` also points to the leased `Data` held by `Compiler`.
enum Asm {
    BeginSpotlight,
    BeginSunlight,
    BindVertexBuffer,
    TransferLineData,
    TransferRectLightData,
    TransferSpotlightData,
    CopyLineVertices,
    CopyRectLightVertices,
    CopySpotlightVertices,
    DrawLines(u32),
    DrawPointLights((usize, usize)),
    DrawRectLightBegin,
    DrawRectLight(DrawAsm),
    DrawRectLightEnd,
    DrawSpotlightBegin,
    DrawSpotlight(DrawAsm),
    DrawSpotlightEnd,
    WriteLineVertices,
    WritePointLightVertices,
    WriteRectLightVertices,
    WriteSpotlightVertices,
}

// TODO: The note below is good but reset is not enough, we need some sort of additional function to also drop the data, like and `undo` or `rollback`
/// Note: If the instructions produced by this command are not completed succesfully the state of the `Compiler` instance will
/// be undefined, and so `reset()` must be called on it. This is because copy operations that don't complete will leave the
/// buffers with incorrect data.
pub struct Compilation<'a> {
    cmds: &'a [Command<'a>],
    code_idx: usize,
    compiler: &'a mut Compiler, //TODO: Mutable for mut access to vertex_buf, let's revisit this if can be made read-only
    mesh_sets: MeshSets,
}

impl Compilation<'_> {
    fn copy_vertices<T>(buf: &mut DirtyData<T>) -> Instruction {
        Instruction::CopyVertices((&mut buf.data.current, buf.gpu_dirty.as_slice()))
    }

    fn draw_lines(buf: &mut DirtyData<Line>, count: u32) -> Instruction {
        Instruction::DrawLines((&mut buf.data.current, count))
    }

    // fn draw_point_lights(buf: &mut Lease<Data>, range: usize) -> Instruction {
    //     Instruction::DrawPointLights((&mut buf, count))
    // }

    // fn draw_rect_light(buf: &mut DirtyData<Line>, count: usize) -> Instruction {
    //     Instruction::DrawLines((&mut buf.data.current, count))
    // }

    fn transfer_data<T>(buf: &mut DirtyData<T>) -> Instruction {
        Instruction::TransferData((buf.data.previous.as_mut().unwrap(), &mut buf.data.current))
    }

    fn write_point_light_vertices(&mut self) -> Instruction {
        Instruction::WriteVertces((
            self.compiler.point_light_buf.as_mut().unwrap(),
            0..POINT_LIGHT.len() as _,
        ))
    }

    fn write_vertices<T>(buf: &mut DirtyData<T>) -> Instruction {
        Instruction::WriteVertces((
            &mut buf.data.current,
            buf.cpu_dirty.as_ref().unwrap().clone(),
        ))
    }
}

// TODO: Workaround impl of "Iterator for" until we (soon?) have GATs: https://github.com/rust-lang/rust/issues/44265
impl Compilation<'_> {
    pub fn next(&mut self) -> Option<Instruction> {
        if self.code_idx == self.compiler.code.len() {
            return None;
        }

        let idx = self.code_idx;
        self.code_idx += 1;

        Some(match &self.compiler.code[idx] {
            Asm::CopyLineVertices => Self::copy_vertices(self.compiler.line_buf.as_mut().unwrap()),
            Asm::CopyRectLightVertices => {
                Self::copy_vertices(self.compiler.rect_light_buf.as_mut().unwrap())
            }
            Asm::CopySpotlightVertices => {
                Self::copy_vertices(self.compiler.spotlight_buf.as_mut().unwrap())
            }
            Asm::DrawLines(count) => {
                Self::draw_lines(self.compiler.line_buf.as_mut().unwrap(), *count)
            }
            // Asm::DrawPointLights(range) => {
            //     Self::draw_point_lights(self.compiler.point_light_buf.as_mut().unwrap(), *range)
            // }
            Asm::TransferLineData => Self::transfer_data(self.compiler.line_buf.as_mut().unwrap()),
            Asm::TransferRectLightData => {
                Self::transfer_data(self.compiler.rect_light_buf.as_mut().unwrap())
            }
            Asm::TransferSpotlightData => {
                Self::transfer_data(self.compiler.spotlight_buf.as_mut().unwrap())
            }
            Asm::WritePointLightVertices => self.write_point_light_vertices(),
            Asm::WriteRectLightVertices => {
                Self::write_vertices(self.compiler.rect_light_buf.as_mut().unwrap())
            }
            Asm::WriteSpotlightVertices => {
                Self::write_vertices(self.compiler.spotlight_buf.as_mut().unwrap())
            }
            Asm::WriteLineVertices => {
                Self::write_vertices(self.compiler.line_buf.as_mut().unwrap())
            }
            _ => todo!(),
        })
    }
}

/// Compiles a series of drawing commands into renderable instructions. The purpose of this structure is
/// two-fold:
/// - Reduce per-draw allocations with line and light caches (they are not cleared after each use)
/// - Store references to the in-use mesh textures during rendering (this cache is cleared after use)
#[derive(Default)]
pub struct Compiler {
    code: Vec<Asm>,
    cull_groups: BTreeSet<usize>,
    line_buf: Option<DirtyData<Line>>,
    line_lru: Vec<Lru<Line>>,
    mesh_textures: Vec<Texture2d>,
    point_light_buf: Option<Lease<Data>>,
    rect_light_buf: Option<DirtyData<RectLight>>,
    rect_light_lru: Vec<Lru<RectLight>>,
    spotlight_buf: Option<DirtyData<Spotlight>>,
    spotlight_lru: Vec<Lru<Spotlight>>,
}

impl Compiler {
    /// Allocates or re-allocates leased data of the given size. This could be a function of the DirtyData type, however it only
    /// works because the Compiler happens to know that the host-side of the data
    fn alloc_data<T>(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        buf: &mut Option<DirtyData<T>>,
        len: u64,
    ) {
        #[cfg(debug_assertions)]
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
        let data = pool.borrow_mut().data(
            #[cfg(debug_assertions)]
            &name,
            capacity,
        );

        if let Some(old_buf) = buf.replace(data.into()) {
            // Preserve the old data so that we can copy it directly over before drawing
            let new_buf = &mut buf.as_mut().unwrap();
            new_buf.gpu_usage = old_buf.gpu_usage;
            new_buf.data.previous = Some(old_buf.data.current);
        }
    }

    /// Moves cache items into clumps so future items can be appended onto the end without needing to
    /// resize the cache buffer. As a side effect this causes dirty regions to be moved on the GPU.
    ///
    /// Geometry used very often will end up closer to the beginning of the GPU memory over time, and
    /// will have fewer move operations applied to it as a result.
    fn compact_cache<T>(buf: &mut DirtyData<T>, lru: &mut Vec<Lru<T>>, stride: u64)
    where
        T: Ord,
    {
        // "Forget about" GPU memory regions occupied by unused geometry
        buf.gpu_usage.retain(|(_, key)| {
            let idx = lru
                .binary_search_by(|probe| probe.key.cmp(&key))
                .ok()
                .unwrap();
            lru[idx].recently_used
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
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        camera: &impl Camera,
        mut cmds: &'b mut [Command<'b>],
    ) -> Compilation<'a> {
        assert!(self.code.is_empty());
        assert!(self.mesh_textures.is_empty());
        assert_ne!(cmds.len(), 0);

        // Remove non-visible commands (also prepares mesh commands for sorting by pre-calculating Z)
        self.cull(camera, &mut cmds);

        // Rearrange the commands so draw order doesn't cause unnecessary resource-switching
        self.sort(cmds);

        // Fill the cache buffers for all requested lines and lights (queues copies from CPU to GPU)
        self.fill_caches(
            #[cfg(debug_assertions)]
            name,
            pool,
            cmds,
        );

        Compilation {
            cmds,
            code_idx: 0,
            compiler: self,
            mesh_sets: Default::default(),
        }
    }

    // TODO: Could return counts here and put a tiny bit of speed-up into the `fill_cache` function - could avoid the first four bin searches fwiw
    /// Cull any commands which are not within the camera frustum. Also adds z-order to meshes.
    fn cull(&mut self, camera: &impl Camera, cmds: &mut &mut [Command]) {
        // We use `cull_groups` as a cache for this function only
        self.cull_groups.clear();

        let eye = -camera.eye();
        let mut idx = 0;
        let mut end = cmds.len();

        while idx < end {
            if match &mut cmds[idx] {
                Command::Model(cmd) => {
                    let res = camera.overlaps_sphere(cmd.model.bounds);
                    if res {
                        // Assign a relative measure of distance from the camera for all mesh commands which allows us to submit draw commands
                        // in the best order for the z-buffering algorithm (we use a depth map with comparisons that discard covered fragments)
                        cmd.camera_z = cmd.transform.transform_vector3(eye).length_squared();
                    }

                    res
                }
                Command::PointLight(cmd) => camera.overlaps_sphere(cmd.bounds()),
                Command::RectLight(cmd) => camera.overlaps_sphere(cmd.bounds()),
                Command::Spotlight(cmd) => camera.overlaps_cone(cmd.bounds()),
                _ => {
                    // Lines and Sunlight do not get culled; we assume they are visible and draw them
                    // TODO: Test the effect of adding in line culling with lots and lots of lines, make it a feature or argument?
                    false
                }
            } {
                // The command at `idx` has been culled and won't be drawn (put it at the end of the list/no-man's land)
                end -= 1;
                cmds.swap(idx, end);
            } else {
                // The command at `idx` is visible and will draw normally
                idx += 1;
            }
        }

        // Safely replace `cmds` with a subslice, this drops the references to the culled commands but not their values
        *cmds = &mut take(cmds)[0..end];
    }

    /// Gets this compiler ready to use the given commands by pre-filling vertex cache buffers. Also records the ranges of vertex data
    /// which must be copied from CPU to the GPU.
    fn fill_caches(
        &mut self,
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        cmds: &mut [Command],
    ) {
        // Locate the groups - we know these `SearchIdx` values will not be found as they are gaps in between the groups
        let point_light_idx = cmds
            .binary_search_by(|probe| {
                (Self::group_idx(probe) as isize).cmp(&(SearchIdx::PointLight as _))
            })
            .unwrap_err();
        let rect_light_idx = cmds[point_light_idx..]
            .binary_search_by(|probe| {
                (Self::group_idx(probe) as isize).cmp(&(SearchIdx::RectLight as _))
            })
            .unwrap_err();
        let spotlight_idx = cmds[rect_light_idx..]
            .binary_search_by(|probe| {
                (Self::group_idx(probe) as isize).cmp(&(SearchIdx::Spotlight as _))
            })
            .unwrap_err();
        let line_idx = cmds[spotlight_idx..]
            .binary_search_by(|probe| {
                (Self::group_idx(probe) as isize).cmp(&(SearchIdx::Line as _))
            })
            .unwrap_err();

        // Point light drawing
        let point_light_count = rect_light_idx - point_light_idx;
        if point_light_count > 0 {
            #[cfg(debug_assertions)]
            let name = format!("{} point light vertex buffer", name);

            // Produce the assembly code that will draw all point lights at once
            self.code
                .push(Asm::DrawPointLights((point_light_idx, rect_light_idx)));

            // On the first (it is also the only) allocation we will copy in the icosphere vertices
            if self.point_light_buf.as_ref().is_none() {
                let mut buf = pool.borrow_mut().data(
                    #[cfg(debug_assertions)]
                    &name,
                    POINT_LIGHT.len() as _,
                );

                // Add the point light geometry to the buffer as needed (the spot is reserved for it)
                unsafe {
                    let mut mapped_range = buf.map_range_mut(0..POINT_LIGHT.len() as _).unwrap(); // TODO: Error handling!

                    copy_nonoverlapping(
                        POINT_LIGHT.as_ptr(),
                        mapped_range.as_mut_ptr(),
                        POINT_LIGHT.len() as _,
                    );

                    Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                }

                self.point_light_buf = Some(buf);
            }
        }

        // Rect light drawing
        let rect_light_count = spotlight_idx - rect_light_idx;
        if rect_light_count > 0 {
            #[cfg(debug_assertions)]
            let name = format!("{} rect light vertex buffer", name);

            // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
            let len = (self.rect_light_lru.len() * RECT_LIGHT_STRIDE
                + rect_light_count * RECT_LIGHT_STRIDE) as u64;
            Self::alloc_data(&name, pool, &mut self.rect_light_buf, len);
            let buf = self.rect_light_buf.as_mut().unwrap();

            // Copy data from the previous GPU buffer to the new one
            if buf.data.previous.is_some() {
                self.code.push(Asm::TransferRectLightData);
            }

            // Copy data from the uncompacted end of the buffer back to linear data
            Self::compact_cache(buf, &mut self.rect_light_lru, RECT_LIGHT_STRIDE as _);
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
            self.code.push(Asm::DrawRectLightBegin);

            for cmd in cmds[rect_light_idx..spotlight_idx].iter() {
                let (key, scale) = RectLight::quantize(cmd.as_rect_light().unwrap());
                self.code.push(Asm::DrawRectLight(DrawAsm {
                    lru_idx: match self
                        .rect_light_lru
                        .binary_search_by(|probe| probe.key.cmp(&key))
                    {
                        Err(idx) => {
                            // Cache the normalized geometry for this rectangular light
                            let new_end = end + RECT_LIGHT_STRIDE as u64;
                            let vertices = gen_rect_light(key.dims(), key.range(), key.radius());

                            unsafe {
                                let mut mapped_range =
                                    buf.data.current.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!

                                copy_nonoverlapping(
                                    vertices.as_ptr(),
                                    mapped_range.as_mut_ptr(),
                                    RECT_LIGHT_STRIDE,
                                );

                                Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                            }

                            // Create new cache entries for this rectangular light
                            buf.gpu_usage.push((end, key));
                            self.rect_light_lru.insert(idx, Lru::new(key, end));
                            end = new_end;

                            idx
                        }
                        Ok(idx) => {
                            self.spotlight_lru[idx].recently_used = true;

                            idx
                        }
                    },
                    scale,
                }));
            }

            self.code.push(Asm::DrawRectLightEnd);

            // We may need to copy these vertices from the CPU to the GPU
            if start != end {
                buf.cpu_dirty = Some(start..end);
                self.code.insert(write_idx, Asm::WriteRectLightVertices);
            }
        }

        // Spotlight drawing
        let spotlight_count = line_idx - spotlight_idx;
        if spotlight_count > 0 {
            #[cfg(debug_assertions)]
            let name = format!("{} spotlight vertex buffer", name);

            // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
            let len = (self.spotlight_lru.len() * SPOTLIGHT_STRIDE
                + spotlight_count * SPOTLIGHT_STRIDE) as u64;
            Self::alloc_data(&name, pool, &mut self.spotlight_buf, len);
            let buf = self.spotlight_buf.as_mut().unwrap();

            // Copy data from the previous GPU buffer to the new one
            if buf.data.previous.is_some() {
                self.code.push(Asm::TransferSpotlightData);
            }

            // Copy data from the uncompacted end of the buffer back to linear data
            Self::compact_cache(buf, &mut self.spotlight_lru, SPOTLIGHT_STRIDE as _);
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
            self.code.push(Asm::DrawSpotlightBegin);

            for cmd in cmds[spotlight_idx..line_idx].iter() {
                let (key, scale) = Spotlight::quantize(cmd.as_spotlight().unwrap());
                self.code.push(Asm::DrawSpotlight(DrawAsm {
                    lru_idx: match self
                        .spotlight_lru
                        .binary_search_by(|probe| probe.key.cmp(&key))
                    {
                        Err(idx) => {
                            // Cache the normalized geometry for this spotlight
                            let new_end = end + SPOTLIGHT_STRIDE as u64;
                            let vertices = gen_spotlight(key.radius(), key.range());

                            unsafe {
                                let mut mapped_range =
                                    buf.data.current.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!

                                copy_nonoverlapping(
                                    vertices.as_ptr(),
                                    mapped_range.as_mut_ptr(),
                                    SPOTLIGHT_STRIDE,
                                );

                                Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                            }

                            // Create a new cache entry for this spotlight
                            self.spotlight_lru.insert(idx, Lru::new(key, end));
                            end = new_end;

                            idx
                        }
                        Ok(idx) => {
                            self.spotlight_lru[idx].recently_used = true;

                            idx
                        }
                    },
                    scale,
                }));
            }

            self.code.push(Asm::DrawSpotlightEnd);

            // We may need to copy these vertices from the CPU to the GPU
            if start != end {
                buf.cpu_dirty = Some(start..end);
                self.code.insert(write_idx, Asm::WriteSpotlightVertices);
            }
        }

        // Line drawing
        let line_count = cmds.len() - line_idx;
        if line_count > 0 {
            #[cfg(debug_assertions)]
            let name = format!("{} line vertex buffer", name);

            // Allocate enough `buf` to hold everything in the existing cache and everything we could possibly draw
            let len = (self.line_lru.len() * LINE_STRIDE + line_count * LINE_STRIDE) as u64;
            Self::alloc_data(&name, pool, &mut self.line_buf, len);
            let buf = self.line_buf.as_mut().unwrap();

            // Copy data from the previous GPU buffer to the new one
            if buf.data.previous.is_some() {
                self.code.push(Asm::TransferLineData);
            }

            // Copy data from the uncompacted end of the buffer back to linear data
            Self::compact_cache(buf, &mut self.line_lru, LINE_STRIDE as _);
            if !buf.gpu_dirty.is_empty() {
                self.code.push(Asm::CopyLineVertices);
            }

            // start..end is the back of the buffer where we push new lines
            let start = buf
                .gpu_usage
                .last()
                .map_or(0, |(offset, _)| offset + LINE_STRIDE as u64);
            let mut end = start;

            for cmd in cmds[line_idx..cmds.len()].iter() {
                let line = cmd.as_line().unwrap();
                let key = Line::hash(line);

                // Cache the vertices
                match self.line_lru.binary_search_by(|probe| probe.key.cmp(&key)) {
                    Err(idx) => {
                        // Cache the vertices for this line segment
                        let new_end = end + LINE_STRIDE as u64;
                        let vertices = gen_line(&line.0);

                        unsafe {
                            let mut mapped_range =
                                buf.data.current.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!
                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                LINE_STRIDE,
                            );

                            Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                        }

                        // Create a new cache entry for this line segment
                        self.line_lru.insert(idx, Lru::new(key, end));
                        end = new_end;
                    }
                    Ok(idx) => self.line_lru[idx].recently_used = true,
                }
            }

            // We may need to copy these vertices from the CPU to the GPU
            if end > start {
                buf.cpu_dirty = Some(start..end);
                self.code.push(Asm::WriteLineVertices);
            }

            // Produce the assembly code that will draw all lines at once
            self.code.push(Asm::DrawLines(line_count as _));
        }
    }

    /// All commands sort into groups: first meshes, then lights, followed by lines.
    fn group_idx(cmd: &Command) -> GroupIdx {
        // TODO: Transparencies?
        match cmd {
            Command::Model(_) => GroupIdx::Model,
            Command::PointLight(_) => GroupIdx::PointLight,
            Command::RectLight(_) => GroupIdx::RectLight,
            Command::Spotlight(_) => GroupIdx::Spotlight,
            Command::Sunlight(_) => GroupIdx::Sunlight,
            Command::Line(_) => GroupIdx::Line,
        }
    }

    /// Models sort into sub-groups: first animated, then single texture, followed by dual texture.
    fn model_group_idx(model: &Model) -> usize {
        // TODO: Transparencies?
        if model.is_animated() {
            0
        } else if model.is_single_texture() {
            1
        } else {
            2
        }
    }

    /// Returns the index of a given texture in our `mesh texture` list, adding it as needed.
    fn mesh_texture_idx(&mut self, tex: &Texture2d) -> usize {
        let tex_ptr = tex.as_ptr();
        match self
            .mesh_textures
            .binary_search_by(|probe| probe.as_ptr().cmp(&tex_ptr))
        {
            Err(idx) => {
                // Not in the list - add and return the new index
                self.mesh_textures.insert(idx, Texture2d::clone(tex));

                idx
            }
            Ok(idx) => idx,
        }
    }

    /// Resets the internal caches so that this compiler may be reused by calling the `compile` function.
    pub fn reset(&mut self) {
        self.code.clear();
        self.mesh_textures.clear();

        if let Some(buf) = self.line_buf.as_mut() {
            buf.cpu_dirty = None;
            buf.gpu_dirty.clear();
        }

        if let Some(buf) = self.rect_light_buf.as_mut() {
            buf.cpu_dirty = None;
            buf.gpu_dirty.clear();
        }

        if let Some(buf) = self.spotlight_buf.as_mut() {
            buf.cpu_dirty = None;
            buf.gpu_dirty.clear();
        }

        for item in self.line_lru.iter_mut() {
            item.recently_used = false;
        }

        for item in self.rect_light_lru.iter_mut() {
            item.recently_used = false;
        }

        for item in self.spotlight_lru.iter_mut() {
            item.recently_used = false;
        }
    }

    /// Sorts commands into a predictable and efficient order for drawing.
    fn sort(&mut self, cmds: &mut [Command]) {
        // TODO: Sorting meshes by material also - helpful or not?
        cmds.sort_unstable_by(|lhs, rhs| {
            // Shorthand - we only care about equal or not-equal here
            use Ordering::Equal as eq;

            let lhs_idx = Self::group_idx(lhs) as isize;
            let rhs_idx = Self::group_idx(rhs) as isize;

            // Compare group indices
            match lhs_idx.cmp(&rhs_idx) {
                eq => match lhs {
                    Command::Model(lhs) => {
                        let rhs = rhs.as_model().unwrap();
                        let lhs_idx = Self::model_group_idx(lhs.model);
                        let rhs_idx = Self::model_group_idx(rhs.model);

                        // Compare mesh group indices
                        match lhs_idx.cmp(&rhs_idx) {
                            eq => {
                                for (lhs_tex, rhs_tex) in
                                    lhs.model.textures().zip(rhs.model.textures())
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

#[derive(Clone, Copy)]
struct DrawAsm {
    lru_idx: usize,
    scale: f32,
}

/// Evenly numbered because we use `SearchIdx` to quickly locate these groups while filling the cache.
#[derive(Clone, Copy)]
enum GroupIdx {
    Model = 0,
    Sunlight = 2,
    PointLight = 4,
    RectLight = 6,
    Spotlight = 8,
    Line = 10,
}

/// Individual item of a least-recently-used cache vector. Allows tracking the usage of a key which lives at some memory offset.
struct Lru<T> {
    key: T,
    offset: u64,
    recently_used: bool, // TODO: Should this hold a number instead?
}

impl<T> Lru<T> {
    fn new(key: T, offset: u64) -> Self {
        Self {
            key,
            offset,
            recently_used: false,
        }
    }
}

#[derive(Default)]
pub struct MeshSets {
    pub dual_tex: usize,
    pub single_tex: usize,
    pub trans: usize,
}

/// These oddly numbered indices are the spaces in between the `GroupIdx` values. This was more efficient than
/// finding the actual group index because we would have to walk to the front and back of each group after any
/// binary search in order to find the whole group.
#[derive(Clone, Copy)]
enum SearchIdx {
    PointLight = 3,
    RectLight = 5,
    Spotlight = 7,
    Line = 9,
}
