use {
    super::{
        geom::{
            gen_line, gen_rect_light, gen_spotlight, LINE_STRIDE, POINT_LIGHT, RECT_LIGHT_STRIDE,
            SPOTLIGHT_STRIDE,
        },
        instruction::Instruction,
        key::{Key, LineKey, RectLightKey, SpotlightKey},
        Command,
    },
    crate::{
        camera::Camera,
        gpu::{
            data::{CopyRange, Mapping},
            Data, Lease, Mesh, PoolRef, Texture2d,
        },
    },
    std::{
        cmp::Ordering,
        mem::take,
        ops::{Add, Deref, DerefMut, Range, Sub},
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
    BeginRectLight,
    BeginSpotlight,
    BeginSunlight,
    BindVertexBuffer,
    TransferLineData,
    TransferVertexData,
    CopyVertices,
    DrawLines(Range<usize>),
    DrawPointLights(Range<usize>),
    DrawRectLight(DrawAsm),
    DrawSpotlight(DrawAsm),
    WriteLightVertices,
    WriteLineVertices,
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
    fn copy_vertices(&mut self) -> Instruction {
        let vertex_buf = self.compiler.vertex_buf.as_mut().unwrap();

        Instruction::CopyVertices((
            &mut vertex_buf.data.current,
            vertex_buf.gpu_dirty.as_slice(),
        ))
    }

    fn transfer_line_data(&mut self) -> Instruction {
        let buf = self.compiler.line_buf.as_mut().unwrap();

        Instruction::TransferData((buf.data.previous.as_mut().unwrap(), &mut buf.data.current))
    }

    fn transfer_vertex_data(&mut self) -> Instruction {
        let buf = self.compiler.vertex_buf.as_mut().unwrap();

        Instruction::TransferData((buf.data.previous.as_mut().unwrap(), &mut buf.data.current))
    }

    fn write_light_vertices(&mut self) -> Instruction {
        let buf = self.compiler.vertex_buf.as_mut().unwrap();

        Instruction::WriteVertices((
            &mut buf.data.current,
            buf.cpu_dirty.as_ref().unwrap().clone(),
        ))
    }

    fn write_line_vertices(&mut self) -> Instruction {
        let buf = self.compiler.line_buf.as_mut().unwrap();

        Instruction::WriteVertices((
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
            Asm::CopyVertices => self.copy_vertices(),
            Asm::TransferLineData => self.transfer_line_data(),
            Asm::TransferVertexData => self.transfer_vertex_data(),
            Asm::WriteLightVertices => self.write_light_vertices(),
            Asm::WriteLineVertices => self.write_line_vertices(),
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
    line_buf: Option<DirtyData<Lease<Data>>>,
    line_lru: Vec<Lru<LineKey>>,
    mesh_textures: Vec<Texture2d>,
    point_light_lru: bool,
    rect_light_lru: Vec<Lru<RectLightKey>>,
    spotlight_lru: Vec<Lru<SpotlightKey>>,
    vertex_buf: Option<DirtyData<Lease<Data>>>,
}

impl Compiler {
    /// Allocates or re-allocates leased data of the given size. This could be a function of the DirtyData type, however it only
    /// works because the Compiler happens to know that the host-side of the data
    fn alloc_data(
        #[cfg(debug_assertions)] name: &str,
        pool: &PoolRef,
        buf: &mut Option<DirtyData<Lease<Data>>>,
        len: u64,
    ) -> bool {
        // Early-our if we do not need to resize the buffer
        if let Some(existing) = buf.as_ref() {
            if len <= existing.capacity() {
                #[cfg(debug_assertions)]
                {
                    buf.as_mut().unwrap().set_name(&name);
                }

                return false;
            }
        }

        // We over-allocate the requested capacity to prevent rapid reallocations
        let capacity = (len as f32 * CACHE_CAPACITY_FACTOR) as u64;
        let data = pool.borrow_mut().data(
            #[cfg(debug_assertions)]
            &name,
            capacity,
        );

        if let Some(mut old_buf) = buf.replace(DirtyData::new(data)) {
            // Preserve the old data so that we can copy it directly over before drawing
            let new_buf = &mut buf.as_mut().unwrap();
            new_buf.gpu_usage = old_buf.gpu_usage;
            new_buf.data.previous = Some(old_buf.data.current);
        }

        return true;
    }

    /// Moves cache items into clumps so future items can be appended onto the end without needing to
    /// resize the cache buffer. As a side effect this causes dirty regions to be moved on the GPU.
    ///
    /// Geometry used very often will end up closer to the beginning of the GPU memory over time, and
    /// will have fewer move operations applied to it as a result.
    fn compact_caches(&mut self) {
        if let Some(vertex_buf) = self.vertex_buf.as_mut() {
            let line_lru = &mut self.line_lru;
            let rect_light_lru = &mut self.rect_light_lru;
            let spotlight_lru = &mut self.spotlight_lru;

            // "Forget about" GPU memory regions occupied by unused geometry
            vertex_buf.gpu_usage.retain(|(_, key)| match key {
                Key::Line(line) => {
                    let idx = line_lru
                        .binary_search_by(|probe| probe.key.cmp(&line))
                        .ok()
                        .unwrap();
                    line_lru[idx].recently_used
                }
                Key::RectLight(light) => {
                    let idx = rect_light_lru
                        .binary_search_by(|probe| probe.key.cmp(&light))
                        .ok()
                        .unwrap();
                    rect_light_lru[idx].recently_used
                }
                Key::Spotlight(light) => {
                    let idx = spotlight_lru
                        .binary_search_by(|probe| probe.key.cmp(&light))
                        .ok()
                        .unwrap();
                    spotlight_lru[idx].recently_used
                }
            });

            // We only need to compact the memory in the region preceding the dirty region, because that geometry will
            // be uploaded and used during this compilation (draw) - we will defer that region to the next compilation
            let mut start = POINT_LIGHT.len() as u64;
            let end = vertex_buf.cpu_dirty.as_ref().map_or_else(
                || {
                    vertex_buf
                        .gpu_usage
                        .last()
                        .map_or(start, |(offset, key)| offset + Self::stride(key))
                },
                |dirty| dirty.start,
            );

            // Walk through the GPU memory in order, moving items back to the "empty" region and as we go
            for (offset, key) in &mut vertex_buf.gpu_usage {
                assert!(start <= end);
                assert!(start <= *offset);

                // Early out if we have exceeded the non-dirty region
                if *offset >= end {
                    break;
                }

                // Skip items which should not be moved
                let stride = Self::stride(key);
                if start == *offset {
                    start += stride;
                    continue;
                }

                // Skip items which are too big to fit into the remaining "empty" region
                // (There will be a region of "empty" space here because this is a one-pass algorithm)
                if start + stride >= end {
                    start = *offset + stride;
                    continue;
                }

                // Move this item back to the beginning of the empty region
                vertex_buf.gpu_dirty.push(CopyRange {
                    dst: start,
                    src: *offset..*offset + stride,
                });

                // Update the LRU item for this geometry
                match key {
                    Key::Line(line) => {
                        let idx = line_lru
                            .binary_search_by(|probe| probe.key.cmp(&line))
                            .ok()
                            .unwrap();
                        line_lru[idx].offset = start;
                    }
                    Key::RectLight(light) => {
                        let idx = rect_light_lru
                            .binary_search_by(|probe| probe.key.cmp(&light))
                            .ok()
                            .unwrap();
                        rect_light_lru[idx].offset = start;
                    }
                    Key::Spotlight(light) => {
                        let idx = spotlight_lru
                            .binary_search_by(|probe| probe.key.cmp(&light))
                            .ok()
                            .unwrap();
                        spotlight_lru[idx].offset = start;
                    }
                };

                start += stride;
            }

            // Produce the assembly code that will re-arrange the GPU buffer
            self.code.insert(0, Asm::CopyVertices);
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

        #[cfg(debug_assertions)]
        if let Some(buf) = self.line_buf.as_ref() {
            assert!(buf.is_clean());
        }

        #[cfg(debug_assertions)]
        if let Some(buf) = self.vertex_buf.as_ref() {
            assert!(buf.is_clean());
        }

        // Remove non-visible commands (also prepares mesh commands for sorting by pre-calculating Z)
        Self::cull(camera, &mut cmds);

        // Rearrange the commands so draw order doesn't cause unnecessary resource-switching
        self.sort(cmds);

        // Fill the cache buffers for all requested lines and lights (queues copies from CPU to GPU)
        self.fill_caches(
            #[cfg(debug_assertions)]
            name,
            pool,
            cmds,
        );

        // Remove the least recently used line and light data from the caches (queues moves within GPU)
        self.compact_caches();

        Compilation {
            cmds,
            code_idx: 0,
            compiler: self,
            mesh_sets: Default::default(),
        }
    }

    // TODO: Could return counts here and put a tiny bit of speed-up into the `fill_cache` function - could avoid the first four bin searches fwiw
    /// Cull any commands which are not within the camera frustum. Also adds z-order to meshes.
    fn cull(camera: &impl Camera, cmds: &mut &mut [Command]) {
        let eye = -camera.eye();
        let mut idx = 0;
        let mut end = cmds.len();

        while idx < end {
            if match &mut cmds[idx] {
                Command::Mesh(cmd) => {
                    let res = camera.overlaps_sphere(cmd.mesh.bounds);
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
                    true
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

        // Count how many items of each group we found
        let point_light_count = rect_light_idx - point_light_idx;
        let rect_light_count = spotlight_idx - rect_light_idx;
        let spotlight_count = line_idx - spotlight_idx;
        let line_count = cmds.len() - line_idx;

        if point_light_count + rect_light_count + spotlight_count > 0 {
            let point_light_count = point_light_count - point_light_idx;
            let rect_light_count = rect_light_count - rect_light_idx;
            let spotlight_count = spotlight_count - spotlight_idx;

            // Note that the vertex buffer will always reserve space for a point light icosphere
            let len = (POINT_LIGHT.len()
                + rect_light_count * RECT_LIGHT_STRIDE
                + spotlight_count * SPOTLIGHT_STRIDE) as u64;
            let mut end = (POINT_LIGHT.len()
                + self.line_lru.len() * LINE_STRIDE
                + self.rect_light_lru.len() * RECT_LIGHT_STRIDE
                + self.spotlight_lru.len() * SPOTLIGHT_STRIDE) as u64;

            #[cfg(debug_assertions)]
            let name = format!("{} vertex buffer", name);

            // Resize the vertex buffer as needed
            if Self::alloc_data(
                #[cfg(debug_assertions)]
                &name,
                pool,
                &mut self.vertex_buf,
                len,
            ) {
                self.code.push(Asm::TransferVertexData);
            }

            let mut start = end;
            let vertex_buf = self.vertex_buf.as_mut().unwrap();

            if point_light_count > 0 {
                // Produce the assembly code that will draw all point lights at once
                self.code.push(Asm::DrawPointLights(point_light_idx..rect_light_idx));

                // Add the point light geometry to the buffer as needed (the spot is reserved for it)
                if !self.point_light_lru {
                    self.point_light_lru = true;

                    unsafe {
                        let mut mapped_range =
                            vertex_buf.map_range_mut(0..POINT_LIGHT.len() as _).unwrap(); // TODO: Error handling!

                        copy_nonoverlapping(
                            POINT_LIGHT.as_ptr(),
                            mapped_range.as_mut_ptr(),
                            POINT_LIGHT.len() as _,
                        );

                        Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                    }
                }
            }

            // Produce the assembly code that will draw rectangular lights one at a time
            if rect_light_count > 0 {
                self.code.push(Asm::BeginRectLight);

                for cmd in cmds[rect_light_idx..spotlight_idx].iter() {
                    let (key, scale) = RectLightKey::quantize(cmd.as_rect_light().unwrap());
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
                                        vertex_buf.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!

                                    copy_nonoverlapping(
                                        vertices.as_ptr(),
                                        mapped_range.as_mut_ptr(),
                                        RECT_LIGHT_STRIDE,
                                    );

                                    Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                                }

                                // Create new cache entries for this rectangular light
                                vertex_buf.gpu_usage.push((end, key.into()));
                                self.rect_light_lru.insert(
                                    idx,
                                    Lru::new(key, end),
                                );
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
            }

            // Produce the assembly code that will draw spotlights one at a time
            if spotlight_count > 0 {
                self.code.push(Asm::BeginSpotlight);

                for cmd in cmds[spotlight_idx..line_idx].iter() {
                    let (key, scale) = SpotlightKey::quantize(cmd.as_spotlight().unwrap());
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
                                        vertex_buf.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!

                                    copy_nonoverlapping(
                                        vertices.as_ptr(),
                                        mapped_range.as_mut_ptr(),
                                        SPOTLIGHT_STRIDE,
                                    );

                                    Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                                }

                                // Create a new cache entry for this spotlight
                                self.spotlight_lru.insert(
                                    idx,
                                    Lru::new(key, end),
                                );
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
            }

            // We need to copy these vertices from the CPU to the GPU
            vertex_buf.cpu_dirty = Some(start..end);
            self.code.insert(0, Asm::WriteLightVertices);
        }

        if line_count > 0 {
            let len = (line_count * LINE_STRIDE) as u64;
            let mut end = (self.line_lru.len() * LINE_STRIDE) as u64;

            #[cfg(debug_assertions)]
            let name = format!("{} line buffer", name);

            // Resize the line buffer as needed
            if Self::alloc_data(
                #[cfg(debug_assertions)]
                &name,
                pool,
                &mut self.line_buf,
                len,
            ) {
                self.code.push(Asm::TransferLineData);
            }

            let start = end;
            let line_buf = self.line_buf.as_mut().unwrap();

            for cmd in cmds[line_idx..cmds.len()].iter() {
                let line = cmd.as_line().unwrap();
                let key = LineKey::hash(line);

                // Cache the vertices
                match self.line_lru.binary_search_by(|probe| probe.key.cmp(&key)) {
                    Err(idx) => {
                        // Cache the vertices for this line segment
                        let new_end = end + LINE_STRIDE as u64;
                        let vertices = gen_line(&line.0);

                        unsafe {
                            let mut mapped_range = line_buf.map_range_mut(end..new_end).unwrap(); // TODO: Error handling!

                            copy_nonoverlapping(
                                vertices.as_ptr(),
                                mapped_range.as_mut_ptr(),
                                LINE_STRIDE,
                            );

                            Mapping::flush(&mut mapped_range).unwrap(); // TODO: Error handling!
                        }

                        // Create a new cache entry for this line segment
                        self.line_lru.insert(
                            idx,
                            Lru {
                                key,
                                offset: end,
                                recently_used: true,
                            },
                        );
                        end = new_end;
                    }
                    Ok(idx) => {
                        self.line_lru[idx].recently_used = true;
                    }
                }
            }

            // We need to copy these vertices from the CPU to the GPU
            line_buf.cpu_dirty = Some(start..end);
            self.code.insert(0, Asm::WriteLineVertices);

            // Produce the assembly code that will draw all lines at once
            //self.code.push(Asm::DrawLines)
        }
    }

    /// All commands sort into groups: first meshes, then lights, followed by lines.
    fn group_idx(cmd: &Command) -> GroupIdx {
        // TODO: Transparencies?
        match cmd {
            Command::Mesh(_) => GroupIdx::Mesh,
            Command::PointLight(_) => GroupIdx::PointLight,
            Command::RectLight(_) => GroupIdx::RectLight,
            Command::Spotlight(_) => GroupIdx::Spotlight,
            Command::Sunlight(_) => GroupIdx::Sunlight,
            Command::Line(_) => GroupIdx::Line,
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
        self.mesh_textures.clear();

        // Reset the vertex buffer dirty regions
        if let Some(vertex_buf) = self.vertex_buf.as_mut() {
            vertex_buf.cpu_dirty = None;
            vertex_buf.data.previous = None;
            vertex_buf.gpu_dirty.clear();
        }

        // Finally, reset the "recently used" flags
        self.point_light_lru = false;

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
                    Command::Mesh(lhs) => {
                        let rhs = rhs.as_mesh().unwrap();
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
    }

    /// Returns the amount of `vertex_buf` memory used by a given key.
    fn stride(key: &Key) -> u64 {
        (match key {
            Key::Line(_) => LINE_STRIDE,
            Key::RectLight(_) => RECT_LIGHT_STRIDE,
            Key::Spotlight(_) => SPOTLIGHT_STRIDE,
        }) as _
    }
}

/// Extends the data type so we can track which portions require updates. Does not teach an entire city full
/// of people that dancing is the best thing there is.
struct DirtyData<T>
where
    T: DerefMut<Target = Data>,
{
    cpu_dirty: Option<Range<u64>>, // This range, if present, is the portion that needs to be copied from cpu to gpu
    data: Allocation<T>,
    gpu_dirty: Vec<CopyRange>, // Segments of gpu memory which must be "compacted" (read: copied) within the gpu
    gpu_usage: Vec<(u64, Key)>, // Memory usage on the gpu, sorted by the first field which is the offset.
}

impl<T> DirtyData<T>
where
    T: DerefMut<Target = Data>,
{
    fn new(data: T) -> Self {
        Self {
            cpu_dirty: None,
            data: Allocation {
                current: data,
                previous: None,
            },
            gpu_dirty: vec![],
            gpu_usage: vec![],
        }
    }
}

impl<T> DirtyData<T>
where
    T: DerefMut<Target = Data>,
{
    fn is_clean(&self) -> bool {
        self.cpu_dirty.is_none() && self.gpu_dirty.is_empty()
    }
}

impl<T> Deref for DirtyData<T>
where
    T: DerefMut<Target = Data>,
{
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        &self.data.current
    }
}

impl<T> DerefMut for DirtyData<T>
where
    T: DerefMut<Target = Data>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data.current
    }
}

struct DrawAsm {
    lru_idx: usize,
    scale: f32,
}

/// Evenly numbered because we use `SearchIdx` to quickly locate these groups while filling the cache.
#[derive(Clone, Copy)]
enum GroupIdx {
    Mesh = 0,
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
        // let res = compiler.compile(&camera, &mut cmds);

        // assert!(res.stages_required().is_empty());
        // assert_eq!(res.mesh_sets_required().dual_tex, 0);
        // assert_eq!(res.mesh_sets_required().single_tex, 0);
        // assert_eq!(res.mesh_sets_required().trans, 0);
    }
}
