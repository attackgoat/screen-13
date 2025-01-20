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
    std::{
        collections::{hash_map::Entry, HashMap},
        fmt::{Debug, Formatter},
        mem::take,
        ops::Deref,
        sync::{
            atomic::{AtomicU8, Ordering},
            Arc,
        },
        thread::panicking,
    },
    vk_sync::AccessType,
};

#[cfg(feature = "parking_lot")]
use parking_lot::Mutex;

#[cfg(not(feature = "parking_lot"))]
use std::sync::Mutex;

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
/// # let info = ImageInfo::image_1d(1, vk::Format::R8_UINT, vk::ImageUsageFlags::STORAGE);
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
    pub(super) device: Arc<Device>,
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
    /// let info = ImageInfo::image_2d(32, 32, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED);
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

            #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
            let mut allocator = device.allocator.lock();

            #[cfg(not(feature = "parking_lot"))]
            let mut allocator = allocator.unwrap();

            allocator
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
    /// # let info = ImageInfo::image_1d(1, vk::Format::R8_UINT, vk::ImageUsageFlags::STORAGE);
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
        #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
        let mut image_view_cache = this.image_view_cache.lock();

        #[cfg(not(feature = "parking_lot"))]
        let mut image_view_cache = image_view_cache.unwrap();

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

            #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
            let mut image_view_cache = this.image_view_cache.lock();

            #[cfg(not(feature = "parking_lot"))]
            let mut image_view_cache = image_view_cache.unwrap();

            image_view_cache.clear();
        }

        unsafe {
            this.device.destroy_image(this.image, None);
        }

        {
            profiling::scope!("deallocate");

            #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
            let mut allocator = this.device.allocator.lock();

            #[cfg(not(feature = "parking_lot"))]
            let mut allocator = allocator.unwrap();

            allocator.free(allocation)
        }
        .unwrap_or_else(|_| warn!("Unable to free image allocation"));
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
        #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
        let mut image_view_cache = this.image_view_cache.lock();

        #[cfg(not(feature = "parking_lot"))]
        let mut image_view_cache = image_view_cache.unwrap();

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
    #[builder(default = "SampleCount::Type1", setter(strip_option))]
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
    /// Specifies a cube image.
    #[inline(always)]
    pub const fn cube(size: u32, fmt: vk::Format, usage: vk::ImageUsageFlags) -> ImageInfo {
        Self::new(ImageType::Cube, size, size, 1, 1, fmt, usage)
    }

    /// Specifies a one-dimensional image.
    #[inline(always)]
    pub const fn image_1d(size: u32, fmt: vk::Format, usage: vk::ImageUsageFlags) -> ImageInfo {
        Self::new(ImageType::Texture1D, size, 1, 1, 1, fmt, usage)
    }

    /// Specifies a two-dimensional image.
    #[inline(always)]
    pub const fn image_2d(
        width: u32,
        height: u32,
        fmt: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfo {
        Self::new(ImageType::Texture2D, width, height, 1, 1, fmt, usage)
    }

    /// Specifies a two-dimensional image array.
    #[inline(always)]
    pub const fn image_2d_array(
        width: u32,
        height: u32,
        array_elements: u32,
        fmt: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfo {
        Self::new(
            ImageType::TextureArray2D,
            width,
            height,
            1,
            array_elements,
            fmt,
            usage,
        )
    }

    /// Specifies a three-dimensional image.
    #[inline(always)]
    pub const fn image_3d(
        width: u32,
        height: u32,
        depth: u32,
        fmt: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfo {
        Self::new(ImageType::Texture3D, width, height, depth, 1, fmt, usage)
    }

    #[inline(always)]
    const fn new(
        ty: ImageType,
        width: u32,
        height: u32,
        depth: u32,
        array_elements: u32,
        fmt: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> Self {
        Self {
            ty,
            width,
            height,
            depth,
            array_elements,
            fmt,
            usage,
            flags: vk::ImageCreateFlags::empty(),
            tiling: vk::ImageTiling::OPTIMAL,
            mip_level_count: 1,
            sample_count: SampleCount::Type1,
        }
    }

    /// Specifies a one-dimensional image.
    #[deprecated = "Use ImageInfo::image_1d()"]
    #[doc(hidden)]
    pub fn new_1d(fmt: vk::Format, size: u32, usage: vk::ImageUsageFlags) -> ImageInfoBuilder {
        Self::image_1d(size, fmt, usage).to_builder()
    }

    /// Specifies a two-dimensional image.
    #[deprecated = "Use ImageInfo::image_2d()"]
    #[doc(hidden)]
    pub fn new_2d(
        fmt: vk::Format,
        width: u32,
        height: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::image_2d(width, height, fmt, usage).to_builder()
    }

    /// Specifies a two-dimensional image array.
    #[deprecated = "Use ImageInfo::image_2d_array()"]
    #[doc(hidden)]
    pub fn new_2d_array(
        fmt: vk::Format,
        width: u32,
        height: u32,
        array_elements: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::image_2d_array(width, height, array_elements, fmt, usage).to_builder()
    }

    /// Specifies a three-dimensional image.
    #[deprecated = "Use ImageInfo::image_3d()"]
    #[doc(hidden)]
    pub fn new_3d(
        fmt: vk::Format,
        width: u32,
        height: u32,
        depth: u32,
        usage: vk::ImageUsageFlags,
    ) -> ImageInfoBuilder {
        Self::image_3d(width, height, depth, fmt, usage).to_builder()
    }

    /// Specifies a cube image.
    #[deprecated = "Use ImageInfo::cube()"]
    #[doc(hidden)]
    pub fn new_cube(fmt: vk::Format, size: u32, usage: vk::ImageUsageFlags) -> ImageInfoBuilder {
        Self::cube(size, fmt, usage).to_builder()
    }

    /// Provides an `ImageViewInfo` for this format, type, aspect, array elements, and mip levels.
    pub fn default_view_info(self) -> ImageViewInfo {
        self.into()
    }

    fn image_create_info<'a>(self) -> vk::ImageCreateInfo<'a> {
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

        vk::ImageCreateInfo::default()
            .flags(self.flags)
            .image_type(ty)
            .format(self.fmt)
            .extent(extent)
            .mip_levels(self.mip_level_count)
            .array_layers(array_layers)
            .samples(self.sample_count.into())
            .tiling(self.tiling)
            .usage(self.usage)
            .sharing_mode(vk::SharingMode::CONCURRENT)
            .initial_layout(vk::ImageLayout::UNDEFINED)
    }

    /// Converts an `ImageInfo` into an `ImageInfoBuilder`.
    #[inline(always)]
    pub fn to_builder(self) -> ImageInfoBuilder {
        ImageInfoBuilder {
            array_elements: Some(self.array_elements),
            depth: Some(self.depth),
            flags: Some(self.flags),
            fmt: Some(self.fmt),
            height: Some(self.height),
            mip_level_count: Some(self.mip_level_count),
            sample_count: Some(self.sample_count),
            tiling: Some(self.tiling),
            ty: Some(self.ty),
            usage: Some(self.usage),
            width: Some(self.width),
        }
    }
}

impl From<ImageInfoBuilder> for ImageInfo {
    fn from(info: ImageInfoBuilder) -> Self {
        info.build()
    }
}

impl ImageInfoBuilder {
    /// Builds a new `ImageInfo`.
    ///
    /// # Panics
    ///
    /// If any of the following functions have not been called this function will panic:
    ///
    /// * `ty`
    /// * `fmt`
    /// * `width`
    /// * `height`
    /// * `depth`
    #[inline(always)]
    pub fn build(self) -> ImageInfo {
        match self.fallible_build() {
            Err(ImageInfoBuilderError(err)) => panic!("{err}"),
            Ok(info) => info,
        }
    }
}

#[derive(Debug)]
struct ImageInfoBuilderError(UninitializedFieldError);

impl From<UninitializedFieldError> for ImageInfoBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(err)
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
        let create_info = vk::ImageViewCreateInfo::default()
            .view_type(info.ty.into_vk())
            .format(info.fmt)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            })
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: info.aspect_mask,
                base_array_layer: info.base_array_layer,
                base_mip_level: info.base_mip_level,
                level_count: info.mip_level_count.unwrap_or(vk::REMAINING_MIP_LEVELS),
                layer_count: info.array_layer_count.unwrap_or(vk::REMAINING_ARRAY_LAYERS),
            });

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
    ///
    /// # Note
    ///
    /// Automatically sets [`aspect_mask`](Self::aspect_mask) to a suggested value.
    #[inline(always)]
    pub const fn new(fmt: vk::Format, ty: ImageType) -> ImageViewInfo {
        Self {
            array_layer_count: None,
            aspect_mask: format_aspect_mask(fmt),
            base_array_layer: 0,
            base_mip_level: 0,
            fmt,
            mip_level_count: None,
            ty,
        }
    }

    /// Converts a `ImageViewInfo` into a `ImageViewInfoBuilder`.
    #[inline(always)]
    pub fn to_builder(self) -> ImageViewInfoBuilder {
        ImageViewInfoBuilder {
            array_layer_count: Some(self.array_layer_count),
            aspect_mask: Some(self.aspect_mask),
            base_array_layer: Some(self.base_array_layer),
            base_mip_level: Some(self.base_mip_level),
            fmt: Some(self.fmt),
            mip_level_count: Some(self.mip_level_count),
            ty: Some(self.ty),
        }
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

impl ImageViewInfoBuilder {
    /// Specifies a default view with the given `fmt` and `ty` values.
    #[deprecated = "Use ImageViewInfo::new()"]
    #[doc(hidden)]
    pub fn new(fmt: vk::Format, ty: ImageType) -> Self {
        Self::default().fmt(fmt).ty(ty)
    }

    /// Builds a new 'ImageViewInfo'.
    ///
    /// # Panics
    ///
    /// If any of the following values have not been set this function will panic:
    ///
    /// * `ty`
    /// * `fmt`
    /// * `aspect_mask`
    #[inline(always)]
    pub fn build(self) -> ImageViewInfo {
        match self.fallible_build() {
            Err(ImageViewInfoBuilderError(err)) => panic!("{err}"),
            Ok(info) => info,
        }
    }
}

#[derive(Debug)]
struct ImageViewInfoBuilderError(UninitializedFieldError);

impl From<UninitializedFieldError> for ImageViewInfoBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(err)
    }
}

/// Specifies sample counts supported for an image used for storage operation.
///
/// Values must not exceed the device limits specified by [Device.physical_device.props.limits].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SampleCount {
    /// Single image sample. This is the usual mode.
    Type1,

    /// Multiple image samples.
    Type2,

    /// Multiple image samples.
    Type4,

    /// Multiple image samples.
    Type8,

    /// Multiple image samples.
    Type16,

    /// Multiple image samples.
    Type32,

    /// Multiple image samples.
    Type64,
}

impl SampleCount {
    /// Returns `true` when the value represents a single sample mode.
    pub fn is_single(self) -> bool {
        matches!(self, Self::Type1)
    }

    /// Returns `true` when the value represents a multiple sample mode.
    pub fn is_multiple(self) -> bool {
        matches!(
            self,
            Self::Type2 | Self::Type4 | Self::Type8 | Self::Type16 | Self::Type32 | Self::Type64
        )
    }
}

impl From<SampleCount> for vk::SampleCountFlags {
    fn from(sample_count: SampleCount) -> Self {
        match sample_count {
            SampleCount::Type1 => Self::TYPE_1,
            SampleCount::Type2 => Self::TYPE_2,
            SampleCount::Type4 => Self::TYPE_4,
            SampleCount::Type8 => Self::TYPE_8,
            SampleCount::Type16 => Self::TYPE_16,
            SampleCount::Type32 => Self::TYPE_32,
            SampleCount::Type64 => Self::TYPE_64,
        }
    }
}

impl Default for SampleCount {
    fn default() -> Self {
        Self::Type1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn image_info_cube() {
        let info = ImageInfo::cube(42, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_cube_builder() {
        let info = ImageInfo::cube(42, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = ImageInfoBuilder::default()
            .ty(ImageType::Cube)
            .fmt(vk::Format::R32_SFLOAT)
            .width(42)
            .height(42)
            .depth(1)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_1d() {
        let info = ImageInfo::image_1d(42, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_1d_builder() {
        let info = ImageInfo::image_1d(42, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = ImageInfoBuilder::default()
            .ty(ImageType::Texture1D)
            .fmt(vk::Format::R32_SFLOAT)
            .width(42)
            .height(1)
            .depth(1)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_2d() {
        let info =
            ImageInfo::image_2d(42, 84, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_2d_builder() {
        let info =
            ImageInfo::image_2d(42, 84, vk::Format::R32_SFLOAT, vk::ImageUsageFlags::empty());
        let builder = ImageInfoBuilder::default()
            .ty(ImageType::Texture2D)
            .fmt(vk::Format::R32_SFLOAT)
            .width(42)
            .height(84)
            .depth(1)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_2d_array() {
        let info = ImageInfo::image_2d_array(
            42,
            84,
            100,
            vk::Format::default(),
            vk::ImageUsageFlags::empty(),
        );
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_2d_array_builder() {
        let info = ImageInfo::image_2d_array(
            42,
            84,
            100,
            vk::Format::R32_SFLOAT,
            vk::ImageUsageFlags::empty(),
        );
        let builder = ImageInfoBuilder::default()
            .ty(ImageType::TextureArray2D)
            .fmt(vk::Format::R32_SFLOAT)
            .width(42)
            .height(84)
            .depth(1)
            .array_elements(100)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_3d() {
        let info = ImageInfo::image_3d(
            42,
            84,
            100,
            vk::Format::R32_SFLOAT,
            vk::ImageUsageFlags::empty(),
        );
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_info_image_3d_builder() {
        let info = ImageInfo::image_3d(
            42,
            84,
            100,
            vk::Format::R32_SFLOAT,
            vk::ImageUsageFlags::empty(),
        );
        let builder = ImageInfoBuilder::default()
            .ty(ImageType::Texture3D)
            .fmt(vk::Format::R32_SFLOAT)
            .width(42)
            .height(84)
            .depth(100)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: depth")]
    pub fn image_info_builder_uninit_depth() {
        ImageInfoBuilder::default().build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: fmt")]
    pub fn image_info_builder_uninit_fmt() {
        ImageInfoBuilder::default().depth(1).build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: height")]
    pub fn image_info_builder_uninit_height() {
        ImageInfoBuilder::default()
            .depth(1)
            .fmt(vk::Format::default())
            .build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: ty")]
    pub fn image_info_builder_uninit_ty() {
        ImageInfoBuilder::default()
            .depth(1)
            .fmt(vk::Format::default())
            .height(2)
            .build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: width")]
    pub fn image_info_builder_uninit_width() {
        ImageInfoBuilder::default()
            .depth(1)
            .fmt(vk::Format::default())
            .height(2)
            .ty(ImageType::Texture2D)
            .build();
    }

    #[test]
    pub fn image_view_info() {
        let info = ImageViewInfo::new(vk::Format::default(), ImageType::Texture1D);
        let builder = info.to_builder().build();

        assert_eq!(info, builder);
    }

    #[test]
    pub fn image_view_info_builder() {
        let info = ImageViewInfo::new(vk::Format::default(), ImageType::Texture1D);
        let builder = ImageViewInfoBuilder::default()
            .fmt(vk::Format::default())
            .ty(ImageType::Texture1D)
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .build();

        assert_eq!(info, builder);
    }

    #[test]
    #[should_panic(expected = "Field not initialized: aspect_mask")]
    pub fn image_view_info_builder_uninit_aspect_mask() {
        ImageViewInfoBuilder::default().build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: fmt")]
    pub fn image_view_info_builder_unint_fmt() {
        ImageViewInfoBuilder::default()
            .aspect_mask(vk::ImageAspectFlags::empty())
            .build();
    }

    #[test]
    #[should_panic(expected = "Field not initialized: ty")]
    pub fn image_view_info_builder_unint_ty() {
        ImageViewInfoBuilder::default()
            .aspect_mask(vk::ImageAspectFlags::empty())
            .fmt(vk::Format::default())
            .build();
    }
}
