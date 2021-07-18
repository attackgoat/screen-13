use {
    super::driver::{Image2d, ImageView},
    crate::math::Extent,
    gfx_hal::{
        command::CommandBuffer,
        format::{Aspects, Format, Swizzle},
        image::{Access, Layout, SubresourceRange, Usage, ViewKind},
        memory::{Barrier, Dependencies},
        pso::PipelineStage,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        cell::{Ref, RefCell},
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        iter::once,
        ops::Deref,
    },
};

#[derive(Clone, Eq, Hash, PartialEq)]
struct ImageViewKey {
    view_kind: ViewKind,
    format: Format,
    swizzle: Swizzle,
    range: SubresourceRange,
}

struct State {
    access_mask: Access,
    layout: Layout,
    pipeline_stage: PipelineStage,
}

// TODO: Remove backend image and replace with image<T>

/// A generic structure which can hold an N dimensional GPU texture.
pub struct Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    dims: Extent,
    fmt: Format,
    image: I,
    state: RefCell<State>,
    usage: Usage,
    views: RefCell<HashMap<ImageViewKey, ImageView>>,
}

impl<I> Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub(crate) unsafe fn as_view(
        &self,
        view_kind: ViewKind,
        format: Format,
        swizzle: Swizzle,
        range: SubresourceRange,
    ) -> ImageViewRef {
        let key = ImageViewKey {
            view_kind,
            format,
            swizzle,
            range: range.clone(),
        };

        {
            let mut views = self.views.borrow_mut();
            if !views.contains_key(&key) {
                let view = ImageView::new(
                    self.image.as_ref(),
                    view_kind,
                    format,
                    swizzle,
                    self.usage,
                    range,
                );
                views.insert(key.clone(), view);
            }
        }

        ImageViewRef {
            key,
            views: self.views.borrow(),
        }
    }

    pub(crate) fn format(&self) -> Format {
        self.fmt
    }

    /// # Safety
    /// None
    /// TODO: Swap order of last two params, better name, layout_barrier?
    pub(crate) unsafe fn set_layout(
        &self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        layout: Layout,
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        let mut state = self.state.borrow_mut();
        cmd_buf.pipeline_barrier(
            state.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            once(Barrier::Image {
                states: (state.access_mask, state.layout)..(access_mask, layout),
                target: self.image.as_ref(),
                families: None,
                range: SubresourceRange {
                    aspects: if self.fmt.is_depth() {
                        Aspects::DEPTH
                    } else {
                        Aspects::COLOR
                    },
                    ..Default::default()
                },
            }),
        );

        state.access_mask = access_mask;
        state.layout = layout;
        state.pipeline_stage = pipeline_stage;
    }
}

impl Texture<Image2d> {
    // TODO: Make a builder pattern for this!
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn new(
        #[cfg(feature = "debug-names")] name: &str,
        dims: Extent,
        fmt: Format,
        layout: Layout,
        usage: Usage,
        layers: u16,
        samples: u8,
        mips: u8,
    ) -> Self {
        let access_mask = if layout == Layout::Preinitialized {
            Access::HOST_WRITE
        } else {
            Access::empty()
        };
        let image = Image2d::new_optimal(
            #[cfg(feature = "debug-names")]
            name,
            dims,
            layers,
            samples,
            mips,
            fmt,
            usage,
        );

        let res = Self {
            dims,
            fmt,
            image,
            state: RefCell::new(State {
                access_mask,
                layout,
                pipeline_stage: PipelineStage::TOP_OF_PIPE,
            }),
            usage,
            views: Default::default(),
        };

        // Pre-cache the default view
        res.as_2d_color();

        res
    }

    pub(crate) unsafe fn as_2d_color(&self) -> ImageViewRef {
        self.as_2d_color_format(self.format())
    }

    pub(crate) unsafe fn as_2d_color_format(&self, fmt: Format) -> ImageViewRef {
        self.as_view(
            ViewKind::D2,
            fmt,
            Default::default(),
            SubresourceRange {
                aspects: Aspects::COLOR,
                ..Default::default()
            },
        )
    }

    pub(crate) unsafe fn as_2d_depth(&self) -> ImageViewRef {
        self.as_2d_depth_format(self.format())
    }

    pub(crate) unsafe fn as_2d_depth_format(&self, fmt: Format) -> ImageViewRef {
        self.as_view(
            ViewKind::D2,
            fmt,
            Default::default(),
            SubresourceRange {
                aspects: Aspects::DEPTH,
                ..Default::default()
            },
        )
    }

    /// Gets the dimensions, in pixels, of this `Texture`.
    pub fn dims(&self) -> Extent {
        self.dims
    }
}

impl<I> AsMut<<_Backend as Backend>::Image> for Texture<I>
where
    I: AsMut<<_Backend as Backend>::Image> + AsRef<<_Backend as Backend>::Image>,
{
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Image {
        self.image.as_mut()
    }
}

impl<I> AsRef<<_Backend as Backend>::Image> for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn as_ref(&self) -> &<_Backend as Backend>::Image {
        self.image.as_ref()
    }
}

impl<I> Debug for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Texture")
    }
}

impl<I> Drop for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        // TODO: Do we *need* to drop the views before the image?
        self.views.borrow_mut().clear();
    }
}

pub struct ImageViewRef<'a> {
    key: ImageViewKey,
    views: Ref<'a, HashMap<ImageViewKey, ImageView>>,
}

impl<'a> Deref for ImageViewRef<'a> {
    type Target = ImageView;

    fn deref(&self) -> &Self::Target {
        &self.views[&self.key]
    }
}
