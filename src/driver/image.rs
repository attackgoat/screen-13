//! Image resource types

use {
    super::{
        access_type_from_u8, access_type_into_u8, device::Device, format_aspect_mask, DriverError,
    },
    ash::vk,
    derive_builder::{Builder, UninitializedFieldError},
    gpu_allocator::{
        vulkan::{Allocation, AllocationCreateDesc, AllocationScheme},
        MemoryLocation,
    },
    log::{trace, warn},
    parking_lot::Mutex,
    std::{
        collections::{hash_map::Entry, HashMap},
        fmt::{Debug, Formatter},
        mem::take,
        ops::Deref,
        ptr::null,
        sync::{
            atomic::{AtomicU8, Ordering},
            Arc,
        },
        thread::panicking,
    },
    vk_sync::AccessType,
};

/// Smart pointer handle to an [image] object.
///
/// Also contains information about the object.
///
/// ## `Deref` behavior
///
/// `Image` automatically dereferences to [`vk::Image`] (via the [`Deref`] trait), so you can
/// call `vk::Image`'s methods on a value of type `Image`. To avoid name clashes with `vk::Image`'s
/// methods, the methods of `Image` itself are associated functions, called using
/// [fully qualified syntax]:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use ash::vk;
/// # use screen_13::driver::{AccessType, DriverError};
/// # use screen_13::driver::device::{Device, DeviceInfo};
/// # use screen_13::driver::image::{Image, ImageInfo};
/// # fn main() -> Result<(), DriverError> {
/// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
/// # let info = ImageInfo::new_1d(vk::Format::R8_UINT, 1, vk::ImageUsageFlags::STORAGE);
/// # let my_image = Image::create(&device, info)?;
/// let prev = Image::access(&my_image, AccessType::AnyShaderWrite);
/// # Ok(()) }
/// ```
///
/// [image]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkImage.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
pub struct Image {
    allocation: Option<Allocation>, // None when we don't own the image (Swapchain images)
    device: Arc<Device>,
    image: vk::Image,
    #[allow(clippy::type_complexity)]
    image_view_cache: Mutex<HashMap<ImageViewInfo, ImageView>>,

    /// Information used to create this object.
    pub info: ImageInfo,

    /// A name for debugging purposes.
    pub name: Option<String>,

    prev_access: AtomicU8,
}

impl Image {
    /// Creates a new image on the given device.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::DriverError;
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// let info = ImageInfo::new_2d(vk::Format::R8G8B8A8_UNORM, 32, 32, vk::ImageUsageFlags::SAMPLED);
    /// let image = Image::create(&device, info)?;
    ///
    /// assert_ne!(*image, vk::Image::null());
    /// assert_eq!(image.info.width, 32);
    /// assert_eq!(image.info.height, 32);
    /// # Ok(()) }
    /// ```
    #[profiling::function]
    pub fn create(device: &Arc<Device>, info: impl Into<ImageInfo>) -> Result<Self, DriverError> {
        let info: ImageInfo = info.into();

        //trace!("create: {:?}", &info);
        trace!("create");

        assert!(
            !info.usage.is_empty(),
            "Unspecified image usage {:?}",
            info.usage
        );

        let device = Arc::clone(device);
        let create_info = info
            .image_create_info()
            .queue_family_indices(&device.physical_device.queue_family_indices);
        let image = unsafe {
            device.create_image(&create_info, None).map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?
        };
        let requirements = unsafe { device.get_image_memory_requirements(image) };
        let allocation = {
            profiling::scope!("allocate");

            device
                .allocator
                .as_ref()
                .unwrap()
                .lock()
                .allocate(&AllocationCreateDesc {
                    name: "image",
                    requirements,
                    location: MemoryLocation::GpuOnly,
                    linear: false,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })
        }?;

        unsafe {
            device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|err| {
                    warn!("{err}");

                    DriverError::Unsupported
                })?;
        }

        Ok(Self {
            allocation: Some(allocation),
            device,
            image,
            image_view_cache: Mutex::new(Default::default()),
            info,
            name: None,
            prev_access: AtomicU8::new(access_type_into_u8(AccessType::Nothing)),
        })
    }

    /// Keeps track of some `next_access` which affects this object.
    ///
    /// Returns the previous access for which a pipeline barrier should be used to prevent data
    /// corruption.
    ///
    /// # Note
    ///
    /// Used to maintain object state when passing a _Screen 13_-created `vk::Image` handle to
    /// external code such as [_Ash_] or [_Erupt_] bindings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use ash::vk;
    /// # use screen_13::driver::{AccessType, DriverError};
    /// # use screen_13::driver::device::{Device, DeviceInfo};
    /// # use screen_13::driver::image::{Image, ImageInfo};
    /// # fn main() -> Result<(), DriverError> {
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::new())?);
    /// # let info = ImageInfo::new_1d(vk::Format::R8_UINT, 1, vk::ImageUsageFlags::STORAGE);
    /// # let my_image = Image::create(&device, info)?;
    /// // Initially we want to "Read Other"
    /// let next = AccessType::AnyShaderReadOther;
    /// let prev = Image::access(&my_image, next);
    /// assert_eq!(prev, AccessType::Nothing);
    ///
    /// // External code may now "Read Other"; no barrier required
    ///
    /// // Subsequently we want to "Write"
    /// let next = AccessType::FragmentShaderWrite;
    /// let prev = Image::access(&my_image, next);
    /// assert_eq!(prev, AccessType::AnyShaderReadOther);
    ///
    /// // A barrier on "Read Other" before "Write" is required!
    /// # Ok(()) }
    /// ```
    ///
    /// [_Ash_]: https://crates.io/crates/ash
    /// [_Erupt_]: https://crates.io/crates/erupt
    #[profiling::function]
    pub fn access(this: &Self, next_access: AccessType) -> AccessType {
        access_type_from_u8(
            this.prev_access
                .swap(access_type_into_u8(next_access), Ordering::Relaxed),
        )
    }

    #[profiling::function]
    pub(super) fn clone_raw(this: &Self) -> Self {
        // Moves the image view cache from the current instance to the clone!
        let mut image_view_cache = this.image_view_cache.lock();
        let image_view_cache = take(&mut *image_view_cache);
        let Self { image, info, .. } = *this;

        Self {
            allocation: None,
            device: Arc::clone(&this.device),
            image,
            image_view_cache: Mutex::new(image_view_cache),
            info,
            name: this.name.clone(),
            prev_access: AtomicU8::new(access_type_into_u8(AccessType::Nothing)),
        }
    }

    #[profiling::function]
    fn drop_allocation(this: &Self, allocation: Allocation) {
        {
            profiling::scope!("views");

            this.image_view_cache.lock().clear();
        }

        unsafe {
            this.device.destroy_image(this.image, None);
        }

        {
            profiling::scope!("deallocate");

            this.device
                .allocator
                .as_ref()
                .unwrap()
                .lock()
                .free(allocation)
                .unwrap_or_else(|_| warn!("Unable to free image allocation"));
        }
    }

    /// Consumes a Vulkan image created by some other library.
    ///
    /// The image is not destroyed automatically on drop, unlike images created through the
    /// [`Image::create`] function.
    #[profiling::function]
    pub fn from_raw(device: &Arc<Device>, image: vk::Image, info: impl Into<ImageInfo>) -> Self {
        let device = Arc::clone(device);
        let info = info.into();

        Self {
            allocation: None,
            device,
            image,
            image_view_cache: Mutex::new(Default::default()),
            info,
            name: None,
            prev_access: AtomicU8::new(access_type_into_u8(AccessType::Nothing)),
        }
    }

    #[profiling::function]
    pub(crate) fn view(this: &Self, info: ImageViewInfo) -> Result<vk::ImageView, DriverError> {
        let mut image_view_cache = this.image_view_cache.lock();

        Ok(match image_view_cache.entry(info) {
            Entry::Occupied(entry) => entry.get().image_view,
            Entry::Vacant(entry) => {
                entry
                    .insert(ImageView::create(&this.device, info, this.image)?)
                    .image_view
            }
        })
    }
}

impl Debug for Image {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{} ({:?})", name, self.image)
        } else {
            write!(f, "{:?}", self.image)
        }
    }
}

impl Deref for Image {
    type Target = vk::Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl Drop for Image {
    // This function is not profiled because drop_allocation is
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        // When our allocation is some we allocated ourself; otherwise somebody
        // else owns this image and we should not destroy it. Usually it's the swapchain...
        if let Some(allocation) = self.allocation.take() {
            Self::drop_allocation(self, allocation);
        }
    }
}

/// Information used to create an [`Image`] instance.
#[derive(Builder, Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[builder(
    build_fn(private, name = "fallible_build", error = "ImageInfoBuilderError"),
    derive(Copy, Clone, Debug),
    pattern = "owned"
)]
#[non_exhaustive]
pub struct ImageInfo {
    /// The number of layers in the image.
    #[builder(default = "1", setter(strip_option))]
    pub array_elements: u32,

    /// Image extent of the Z axis, when describing a three dimensional image.
    #[builder(setter(strip_option))]
    pub depth: u32,

    /// A bitmask of describing additional parameters of the image.
    #[builder(default, setter(strip_option))]
    pub flags: vk::ImageCreateFlags,

    /// The format and type of the texel blocks that will be contained in the image.
    #[builder(setter(strip_option))]
    pub fmt: vk::Format,

    /// Image extent of the Y axis, when describing a two or three dimensional image.
    #[builder(setter(strip_option))]
    pub height: u32,

    /// The number of levels of detail available for minified sampling of the image.
    #[builder(default = "1", setter(strip_option))]
    pub mip_level_count: u32,

    /// Specifies the number of [samples per texel].
    ///
    /// [samples per texel]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#primsrast-multisampling
    #[builder(default = "SampleCount::X1", setter(strip_option))]
    pub sample_count: SampleCount,

    /// Specifies the tiling arrangement of the texel blocks in memory.
    ///
    /// The default value is [`vk::ImageTiling::OPTIMAL`].
    #[builder(default = "vk::ImageTiling::OPTIMAL", setter(strip_option))]
    pub tiling: vk::ImageTiling,

    /// The basic dimensionality of the image.
    ///
    /// Layers in array textures do not count as a dimension for the purposes of the image type.
    #[builder(setter(strip_option))]
    pub ty: ImageType,

    /// A bitmask of describing the intended usage of the image.
    #[builder(default, setter(strip_option))]
    pub usage: vk::ImageUsageFlags,

    /// Image extent of the X axis.
    #[builder(setter(strip_option))]
    pub width: u32,
}

impl ImageInfo {
    #[allow(clippy::new_ret_no_self)]
    const fn new(
        fmt: vk::Format,
        ty: ImageType,
        width: u32,
        height: u32,
        depth: u32,
        array_elements: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        ImageInfoBuilder {
            ty: Some(ty),
            fmt: Some(fmt),
            width: Some(width),
            height: Some(height),
            depth: Some(depth),
            usage: Some(usage),
            flags: None,
            tiling: None,
            mip_level_count: None,
            array_elements: Some(array_elements),
            sample_count: None,
        }
    }

    /// Specifies a one-dimensional image.
    pub const fn new_1d(fmt: vk::Format, len: u32, usage: vk::ImageUsageFlags) -> ImageInfoBuilder {
        Self::new(fmt, ImageType::Texture1D, len, 1, 1, 1, usage)
    }

    /// Specifies a two-dimensional image.
    pub const fn new_2d(
        fmt: vk::Format,
        width: u32,
        height: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::new(fmt, ImageType::Texture2D, width, height, 1, 1, usage)
    }

    /// Specifies a two-dimensional image array.
    pub const fn new_2d_array(
        fmt: vk::Format,
        width: u32,
        height: u32,
        array_elements: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::new(
            fmt,
            ImageType::TextureArray2D,
            width,
            height,
            1,
            array_elements,
            usage,
        )
    }

    /// Specifies a three-dimensional image.
    pub const fn new_3d(
        fmt: vk::Format,
        width: u32,
        height: u32,
        depth: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::new(fmt, ImageType::Texture3D, width, height, depth, 1, usage)
    }

    /// Specifies a cube image.
    pub fn new_cube(fmt: vk::Format, width: u32, usage: vk::ImageUsageFlags) -> ImageInfoBuilder {
        Self::new(fmt, ImageType::Cube, width, width, 1, 1, usage)
    }

    /// Provides an `ImageViewInfo` for this format, type, aspect, array elements, and mip levels.
    pub fn default_view_info(self) -> ImageViewInfo {
        self.into()
    }

    fn image_create_info<'a>(self) -> vk::ImageCreateInfoBuilder<'a> {
        let (ty, extent, array_layers) = match self.ty {
            ImageType::Texture1D => (
                vk::ImageType::TYPE_1D,
                vk::Extent3D {
                    width: self.width,
                    height: 1,
                    depth: 1,
                },
                1,
            ),
            ImageType::TextureArray1D => (
                vk::ImageType::TYPE_1D,
                vk::Extent3D {
                    width: self.width,
                    height: 1,
                    depth: 1,
                },
                self.array_elements,
            ),
            ImageType::Texture2D => (
                vk::ImageType::TYPE_2D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
                if self.flags.contains(vk::ImageCreateFlags::CUBE_COMPATIBLE) {
                    self.array_elements
                } else {
                    1
                },
            ),
            ImageType::TextureArray2D => (
                vk::ImageType::TYPE_2D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
                self.array_elements,
            ),
            ImageType::Texture3D => (
                vk::ImageType::TYPE_3D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: self.depth,
                },
                1,
            ),
            ImageType::Cube => (
                vk::ImageType::TYPE_2D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
                6,
            ),
            ImageType::CubeArray => (
                vk::ImageType::TYPE_2D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
                6 * self.array_elements,
            ),
        };

        vk::ImageCreateInfo::builder()
            .flags(self.flags)
            .image_type(ty)
            .format(self.fmt)
            .extent(extent)
            .mip_levels(self.mip_level_count)
            .array_layers(array_layers)
            .samples(self.sample_count.into_vk())
            .tiling(self.tiling)
            .usage(self.usage)
            .sharing_mode(vk::SharingMode::CONCURRENT)
            .initial_layout(vk::ImageLayout::UNDEFINED)
    }
}

impl From<ImageInfoBuilder> for ImageInfo {
    fn from(info: ImageInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl ImageInfoBuilder {
    /// Builds a new `ImageInfo`.
    pub fn build(self) -> ImageInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
struct ImageInfoBuilderError;

impl From<UninitializedFieldError> for ImageInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Describes a subset of an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageSubresource {
    /// The number of layers for which this subset applies.
    ///
    /// The default value of `None` equates to `vk::REMAINING_ARRAY_LAYERS`.
    pub array_layer_count: Option<u32>,

    /// The portion of the image for which this subset applies.
    pub aspect_mask: vk::ImageAspectFlags,

    /// The first array layer for which this subset applies.
    pub base_array_layer: u32,

    /// The first mip level for which this subset applies.
    pub base_mip_level: u32,

    /// The number of mip levels for which this subset applies.
    ///
    /// The default value of `None` equates to `vk::REMAINING_MIP_LEVELS`.
    pub mip_level_count: Option<u32>,
}

impl ImageSubresource {
    pub(crate) fn into_vk(self) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange {
            aspect_mask: self.aspect_mask,
            base_mip_level: self.base_mip_level,
            base_array_layer: self.base_array_layer,
            layer_count: self.array_layer_count.unwrap_or(vk::REMAINING_ARRAY_LAYERS),
            level_count: self.mip_level_count.unwrap_or(vk::REMAINING_MIP_LEVELS),
        }
    }
}

impl From<ImageViewInfo> for ImageSubresource {
    fn from(info: ImageViewInfo) -> Self {
        Self {
            aspect_mask: info.aspect_mask,
            base_mip_level: info.base_mip_level,
            base_array_layer: info.base_array_layer,
            array_layer_count: Some(info.array_layer_count.unwrap_or(vk::REMAINING_ARRAY_LAYERS)),
            mip_level_count: Some(info.mip_level_count.unwrap_or(vk::REMAINING_MIP_LEVELS)),
        }
    }
}

// TODO: Remove this and use vk::ImageType instead
/// Describes the number of dimensions and array elements of an image.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ImageType {
    /// One dimensional (linear) image.
    Texture1D = 0,
    /// One dimensional (linear) image with multiple array elements.
    TextureArray1D = 1,
    /// Two dimensional (planar) image.
    Texture2D = 2,
    /// Two dimensional (planar) image with multiple array elements.
    TextureArray2D = 3,
    /// Three dimensional (volume) image.
    Texture3D = 4,
    /// Six two-dimensional images.
    Cube = 5,
    /// Six two-dimensional images with multiple array elements.
    CubeArray = 6,
}

impl ImageType {
    pub(crate) fn into_vk(self) -> vk::ImageViewType {
        match self {
            Self::Cube => vk::ImageViewType::CUBE,
            Self::CubeArray => vk::ImageViewType::CUBE_ARRAY,
            Self::Texture1D => vk::ImageViewType::TYPE_1D,
            Self::Texture2D => vk::ImageViewType::TYPE_2D,
            Self::Texture3D => vk::ImageViewType::TYPE_3D,
            Self::TextureArray1D => vk::ImageViewType::TYPE_1D_ARRAY,
            Self::TextureArray2D => vk::ImageViewType::TYPE_2D_ARRAY,
        }
    }
}

impl From<ImageType> for vk::ImageType {
    fn from(value: ImageType) -> Self {
        match value {
            ImageType::Texture1D | ImageType::TextureArray1D => vk::ImageType::TYPE_1D,
            ImageType::Texture2D
            | ImageType::TextureArray2D
            | ImageType::Cube
            | ImageType::CubeArray => vk::ImageType::TYPE_2D,
            ImageType::Texture3D => vk::ImageType::TYPE_3D,
        }
    }
}

struct ImageView {
    device: Arc<Device>,
    image_view: vk::ImageView,
}

impl ImageView {
    #[profiling::function]
    fn create(
        device: &Arc<Device>,
        info: impl Into<ImageViewInfo>,
        image: vk::Image,
    ) -> Result<Self, DriverError> {
        let info = info.into();
        let device = Arc::clone(device);
        let create_info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
            p_next: null(),
            flags: vk::ImageViewCreateFlags::empty(),
            view_type: info.ty.into_vk(),
            format: info.fmt,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            },
            image,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: info.aspect_mask,
                base_array_layer: info.base_array_layer,
                base_mip_level: info.base_mip_level,
                level_count: info.mip_level_count.unwrap_or(vk::REMAINING_MIP_LEVELS),
                layer_count: info.array_layer_count.unwrap_or(vk::REMAINING_ARRAY_LAYERS),
            },
        };

        let image_view =
            unsafe { device.create_image_view(&create_info, None) }.map_err(|err| {
                warn!("{err}");

                DriverError::Unsupported
            })?;

        Ok(Self { device, image_view })
    }
}

impl Drop for ImageView {
    #[profiling::function]
    fn drop(&mut self) {
        if panicking() {
            return;
        }

        unsafe {
            self.device.destroy_image_view(self.image_view, None);
        }
    }
}

/// Information used to reinterpret an existing [`Image`] instance.
#[derive(Builder, Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[builder(
    build_fn(private, name = "fallible_build", error = "ImageViewInfoBuilderError"),
    derive(Clone, Copy, Debug),
    pattern = "owned"
)]
#[non_exhaustive]
pub struct ImageViewInfo {
    /// The number of layers that will be contained in the view.
    ///
    /// The default value of `None` equates to `vk::REMAINING_ARRAY_LAYERS`.
    #[builder(default)]
    pub array_layer_count: Option<u32>,

    /// The portion of the image that will be contained in the view.
    pub aspect_mask: vk::ImageAspectFlags,

    /// The first array layer that will be contained in the view.
    #[builder(default)]
    pub base_array_layer: u32,

    /// The first mip level that will be contained in the view.
    #[builder(default)]
    pub base_mip_level: u32,

    /// The format and type of the texel blocks that will be contained in the view.
    pub fmt: vk::Format,

    /// The number of mip levels that will be contained in the view.
    ///
    /// The default value of `None` equates to `vk::REMAINING_MIP_LEVELS`.
    #[builder(default)]
    pub mip_level_count: Option<u32>,

    /// The basic dimensionality of the view.
    pub ty: ImageType,
}

impl ImageViewInfo {
    /// Specifies a default view with the given `fmt` and `ty` values.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(format: vk::Format, ty: ImageType) -> ImageViewInfoBuilder {
        ImageViewInfoBuilder::new(format, ty).aspect_mask(format_aspect_mask(format))
    }

    /// Takes this instance and returns it with a newly specified `ImageType`.
    pub fn with_ty(mut self, ty: ImageType) -> Self {
        self.ty = ty;

        self
    }
}

impl From<ImageInfo> for ImageViewInfo {
    fn from(info: ImageInfo) -> Self {
        Self {
            array_layer_count: Some(info.array_elements),
            aspect_mask: format_aspect_mask(info.fmt),
            base_array_layer: 0,
            base_mip_level: 0,
            fmt: info.fmt,
            mip_level_count: Some(info.mip_level_count),
            ty: info.ty,
        }
    }
}

impl From<ImageViewInfoBuilder> for ImageViewInfo {
    fn from(info: ImageViewInfoBuilder) -> Self {
        info.build()
    }
}

// HACK: https://github.com/colin-kiegel/rust-derive-builder/issues/56
impl ImageViewInfoBuilder {
    /// Specifies a default view with the given `fmt` and `ty` values.
    pub fn new(fmt: vk::Format, ty: ImageType) -> Self {
        Self::default().fmt(fmt).ty(ty)
    }

    /// Builds a new 'ImageViewInfo'.
    pub fn build(self) -> ImageViewInfo {
        self.fallible_build()
            .expect("All required fields set at initialization")
    }
}

#[derive(Debug)]
struct ImageViewInfoBuilderError;

impl From<UninitializedFieldError> for ImageViewInfoBuilderError {
    fn from(_: UninitializedFieldError) -> Self {
        Self
    }
}

/// Specifies sample counts supported for an image used for storage operation.
///
/// Values must not exceed the device limits specified by [Device.physical_device.props.limits].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SampleCount {
    /// Single image sample. This is the usual mode.
    X1,

    /// Multiple image samples.
    X2,

    /// Multiple image samples.
    X4,

    /// Multiple image samples.
    X8,

    /// Multiple image samples.
    X16,

    /// Multiple image samples.
    X32,

    /// Multiple image samples.
    X64,
}

impl SampleCount {
    pub(super) fn into_vk(self) -> vk::SampleCountFlags {
        match self {
            Self::X1 => vk::SampleCountFlags::TYPE_1,
            Self::X2 => vk::SampleCountFlags::TYPE_2,
            Self::X4 => vk::SampleCountFlags::TYPE_4,
            Self::X8 => vk::SampleCountFlags::TYPE_8,
            Self::X16 => vk::SampleCountFlags::TYPE_16,
            Self::X32 => vk::SampleCountFlags::TYPE_32,
            Self::X64 => vk::SampleCountFlags::TYPE_64,
        }
    }
}

impl Default for SampleCount {
    fn default() -> Self {
        Self::X1
    }
}
