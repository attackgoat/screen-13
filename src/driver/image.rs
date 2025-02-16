//! Image resource types

use {
    super::{device::Device, format_aspect_mask, DriverError},
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
        mem::{replace, take},
        ops::{Deref, DerefMut},
        sync::Arc,
        thread::panicking,
    },
    vk_sync::AccessType,
};

#[cfg(feature = "parking_lot")]
use parking_lot::Mutex;

#[cfg(not(feature = "parking_lot"))]
use std::sync::Mutex;

#[cfg(debug_assertions)]
fn assert_aspect_mask_supported(aspect_mask: vk::ImageAspectFlags) {
    use vk::ImageAspectFlags as A;

    const COLOR: A = A::COLOR;
    const DEPTH: A = A::DEPTH;
    const DEPTH_STENCIL: A = A::from_raw(A::DEPTH.as_raw() | A::STENCIL.as_raw());
    const STENCIL: A = A::STENCIL;

    assert!(matches!(
        aspect_mask,
        COLOR | DEPTH | DEPTH_STENCIL | STENCIL
    ));
}

pub(crate) fn image_subresource_range_contains(
    lhs: vk::ImageSubresourceRange,
    rhs: vk::ImageSubresourceRange,
) -> bool {
    lhs.aspect_mask.contains(rhs.aspect_mask)
        && lhs.base_array_layer <= rhs.base_array_layer
        && lhs.base_array_layer + lhs.layer_count >= rhs.base_array_layer + rhs.layer_count
        && lhs.base_mip_level <= rhs.base_mip_level
        && lhs.base_mip_level + lhs.level_count >= rhs.base_mip_level + rhs.level_count
}

pub(crate) fn image_subresource_range_intersects(
    lhs: vk::ImageSubresourceRange,
    rhs: vk::ImageSubresourceRange,
) -> bool {
    lhs.aspect_mask.intersects(rhs.aspect_mask)
        && lhs.base_array_layer < rhs.base_array_layer + rhs.layer_count
        && lhs.base_array_layer + lhs.layer_count > rhs.base_array_layer
        && lhs.base_mip_level < rhs.base_mip_level + rhs.level_count
        && lhs.base_mip_level + lhs.level_count > rhs.base_mip_level
}

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
/// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
/// # let info = ImageInfo::image_1d(1, vk::Format::R8_UINT, vk::ImageUsageFlags::STORAGE);
/// # let my_image = Image::create(&device, info)?;
/// # let my_subresource_range = vk::ImageSubresourceRange::default();
/// let prev = Image::access(&my_image, AccessType::AnyShaderWrite, my_subresource_range);
/// # Ok(()) }
/// ```
///
/// [image]: https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkImage.html
/// [deref]: core::ops::Deref
/// [fully qualified syntax]: https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#fully-qualified-syntax-for-disambiguation-calling-methods-with-the-same-name
pub struct Image {
    accesses: Mutex<ImageAccess>,
    allocation: Option<Allocation>, // None when we don't own the image (Swapchain images)
    pub(super) device: Arc<Device>,
    image: vk::Image,
    #[allow(clippy::type_complexity)]
    image_view_cache: Mutex<HashMap<ImageViewInfo, ImageView>>,

    /// Information used to create this object.
    pub info: ImageInfo,

    /// A name for debugging purposes.
    pub name: Option<String>,
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

        let accesses = Mutex::new(ImageAccess::new(info));

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
            accesses,
            allocation: Some(allocation),
            device,
            image,
            image_view_cache: Mutex::new(Default::default()),
            info,
            name: None,
        })
    }

    /// Keeps track of some next `access` which affects a `range` this image.
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
    /// # let device = Arc::new(Device::create_headless(DeviceInfo::default())?);
    /// # let info = ImageInfo::image_1d(1, vk::Format::R8_UINT, vk::ImageUsageFlags::STORAGE);
    /// # let my_image = Image::create(&device, info)?;
    /// # let my_subresource_range = vk::ImageSubresourceRange::default();
    /// // Initially we want to "Read Other"
    /// let next = AccessType::AnyShaderReadOther;
    /// let mut prev = Image::access(&my_image, next, my_subresource_range);
    /// assert_eq!(prev.next().unwrap().0, AccessType::Nothing);
    ///
    /// // External code may now "Read Other"; no barrier required
    ///
    /// // Subsequently we want to "Write"
    /// let next = AccessType::FragmentShaderWrite;
    /// let mut prev = Image::access(&my_image, next, my_subresource_range);
    /// assert_eq!(prev.next().unwrap().0, AccessType::AnyShaderReadOther);
    ///
    /// // A barrier on "Read Other" before "Write" is required!
    /// # Ok(()) }
    /// ```
    ///
    /// [_Ash_]: https://crates.io/crates/ash
    /// [_Erupt_]: https://crates.io/crates/erupt
    #[profiling::function]
    pub fn access(
        this: &Self,
        access: AccessType,
        mut access_range: vk::ImageSubresourceRange,
    ) -> impl Iterator<Item = (AccessType, vk::ImageSubresourceRange)> + '_ {
        #[cfg(debug_assertions)]
        {
            assert_aspect_mask_supported(access_range.aspect_mask);

            assert!(format_aspect_mask(this.info.fmt).contains(access_range.aspect_mask));
        }

        if access_range.layer_count == vk::REMAINING_ARRAY_LAYERS {
            debug_assert!(access_range.base_array_layer < this.info.array_layer_count);

            access_range.layer_count = this.info.array_layer_count - access_range.base_array_layer
        }

        debug_assert!(
            access_range.base_array_layer + access_range.layer_count <= this.info.array_layer_count
        );

        if access_range.level_count == vk::REMAINING_MIP_LEVELS {
            debug_assert!(access_range.base_mip_level < this.info.mip_level_count);

            access_range.level_count = this.info.mip_level_count - access_range.base_mip_level
        }

        debug_assert!(
            access_range.base_mip_level + access_range.level_count <= this.info.mip_level_count
        );

        let accesses = this.accesses.lock();

        #[cfg(not(feature = "parking_lot"))]
        let accesses = accesses.unwrap();

        ImageAccessIter::new(accesses, access, access_range)
    }

    #[profiling::function]
    pub(super) fn clone_swapchain(this: &Self) -> Self {
        // Moves the image view cache from the current instance to the clone!
        #[cfg_attr(not(feature = "parking_lot"), allow(unused_mut))]
        let mut image_view_cache = this.image_view_cache.lock();

        #[cfg(not(feature = "parking_lot"))]
        let mut image_view_cache = image_view_cache.unwrap();

        let image_view_cache = take(&mut *image_view_cache);

        // Does NOT copy over the image accesses!
        let Self { image, info, .. } = *this;
        let mut accesses = ImageAccess::new(info);

        // Force previous access to general to wait for presentation
        for access in &mut accesses.accesses {
            *access = AccessType::General;
        }

        let accesses = Mutex::new(accesses);

        Self {
            accesses,
            allocation: None,
            device: Arc::clone(&this.device),
            image,
            image_view_cache: Mutex::new(image_view_cache),
            info,
            name: this.name.clone(),
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
        let mut accesses = ImageAccess::new(info);

        // For now default all image access to general, but maybe make this configurable later.
        // This helps make sure the first presentation of a swapchain image doesn't throw a
        // validation error, but it could also be very useful for raw vulkan images from other
        // sources.
        for access in &mut accesses.accesses {
            *access = AccessType::General;
        }

        Self {
            accesses: Mutex::new(accesses),
            allocation: None,
            device,
            image,
            image_view_cache: Mutex::new(Default::default()),
            info,
            name: None,
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

#[derive(Debug)]
struct ImageAccess {
    accesses: Box<[AccessType]>,

    #[cfg(debug_assertions)]
    array_layer_count: u32,

    aspect_count: u8,
    mip_level_count: u32,
}

impl ImageAccess {
    fn new(info: ImageInfo) -> Self {
        let aspect_mask = format_aspect_mask(info.fmt);

        #[cfg(debug_assertions)]
        assert_aspect_mask_supported(aspect_mask);

        let aspect_count = aspect_mask.as_raw().count_ones() as u8;
        let array_layer_count = info.array_layer_count;
        let mip_level_count = info.mip_level_count;

        Self {
            accesses: vec![
                AccessType::Nothing;
                (aspect_count as u32 * array_layer_count * mip_level_count) as _
            ]
            .into_boxed_slice(),

            #[cfg(debug_assertions)]
            array_layer_count,

            aspect_count,
            mip_level_count,
        }
    }

    fn idx(&self, aspect: u8, array_layer: u32, mip_level: u32) -> usize {
        // For a 3 Layer, 2 Mip, Depth/Stencil image:
        // 0     1     2     3     4     5     6     7     8     9     10    11
        // DL0M0 SL0M0 DL0M1 SL0M1 DL1M0 SL1M0 DL1M1 SL1M1 DL2M0 SL2M0 DL2M1 SL2M1
        let idx = (array_layer * self.aspect_count as u32 * self.mip_level_count
            + mip_level * self.aspect_count as u32
            + aspect as u32) as _;

        debug_assert!(idx < self.accesses.len());

        idx
    }
}

struct ImageAccessIter<T> {
    access: AccessType,
    access_range: ImageAccessRange,
    array_layer: u32,
    aspect: u8,
    image: T,
    mip_level: u32,
}

impl<T> ImageAccessIter<T> {
    fn new(image: T, access: AccessType, access_range: vk::ImageSubresourceRange) -> Self
    where
        T: DerefMut<Target = ImageAccess>,
    {
        #[cfg(debug_assertions)]
        assert_aspect_mask_supported(access_range.aspect_mask);

        #[cfg(debug_assertions)]
        assert!(access_range.base_array_layer < image.array_layer_count);

        debug_assert!(access_range.base_mip_level < image.mip_level_count);
        debug_assert_ne!(access_range.layer_count, 0);
        debug_assert_ne!(access_range.level_count, 0);

        let aspect_count = access_range.aspect_mask.as_raw().count_ones() as _;

        debug_assert!(aspect_count <= image.aspect_count);

        let base_aspect = access_range.aspect_mask.as_raw().trailing_zeros() as _;

        Self {
            access,
            array_layer: 0,
            aspect: 0,
            image,
            mip_level: 0,
            access_range: ImageAccessRange {
                aspect_count,
                base_array_layer: access_range.base_array_layer,
                base_aspect,
                base_mip_level: access_range.base_mip_level,
                layer_count: access_range.layer_count,
                level_count: access_range.level_count,
            },
        }
    }
}

impl<T> Iterator for ImageAccessIter<T>
where
    T: DerefMut<Target = ImageAccess>,
{
    type Item = (AccessType, vk::ImageSubresourceRange);

    fn next(&mut self) -> Option<Self::Item> {
        if self.aspect == self.access_range.aspect_count {
            return None;
        }

        let mut res = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::from_raw(
                (1 << (self.access_range.base_aspect + self.aspect)) as _,
            ),
            base_array_layer: self.access_range.base_array_layer + self.array_layer,
            base_mip_level: self.access_range.base_mip_level + self.mip_level,
            layer_count: 1,
            level_count: 1,
        };

        let base_aspect = (self.image.aspect_count << self.access_range.base_aspect == 8) as u8;
        let prev_access = replace(
            {
                let idx = self.image.idx(
                    base_aspect + self.aspect,
                    res.base_array_layer,
                    res.base_mip_level,
                );

                unsafe { self.image.accesses.get_unchecked_mut(idx) }
            },
            self.access,
        );

        loop {
            self.mip_level += 1;
            self.mip_level %= self.access_range.level_count;
            if self.mip_level == 0 {
                break;
            }

            let idx = self.image.idx(
                base_aspect + self.aspect,
                self.access_range.base_array_layer + self.array_layer,
                self.access_range.base_mip_level + self.mip_level,
            );
            let access = unsafe { self.image.accesses.get_unchecked_mut(idx) };
            if *access != prev_access {
                return Some((prev_access, res));
            }

            *access = self.access;
            res.level_count += 1;
        }

        loop {
            self.array_layer += 1;
            self.array_layer %= self.access_range.layer_count;
            if self.array_layer == 0 {
                break;
            }

            if res.base_mip_level != self.access_range.base_mip_level {
                return Some((prev_access, res));
            }

            let array_layer = self.access_range.base_array_layer + self.array_layer;
            let end_mip_level = self.access_range.base_mip_level + self.access_range.level_count;

            for mip_level in self.access_range.base_mip_level..end_mip_level {
                let idx = self
                    .image
                    .idx(base_aspect + self.aspect, array_layer, mip_level);
                let access = unsafe { *self.image.accesses.get_unchecked(idx) };
                if access != prev_access {
                    return Some((prev_access, res));
                }
            }

            for mip_level in self.access_range.base_mip_level..end_mip_level {
                let idx = self
                    .image
                    .idx(base_aspect + self.aspect, array_layer, mip_level);
                let access = unsafe { self.image.accesses.get_unchecked_mut(idx) };
                *access = self.access;
            }

            res.layer_count += 1;
        }

        loop {
            self.aspect += 1;
            if self.aspect == self.access_range.aspect_count {
                return Some((prev_access, res));
            }

            let end_array_layer =
                self.access_range.base_array_layer + self.access_range.layer_count;
            let end_mip_level = self.access_range.base_mip_level + self.access_range.level_count;

            for array_layer in self.access_range.base_array_layer..end_array_layer {
                for mip_level in self.access_range.base_mip_level..end_mip_level {
                    let idx = self
                        .image
                        .idx(base_aspect + self.aspect, array_layer, mip_level);
                    let access = unsafe { *self.image.accesses.get_unchecked(idx) };
                    if access != prev_access {
                        return Some((prev_access, res));
                    }
                }
            }

            for array_layer in self.access_range.base_array_layer..end_array_layer {
                for mip_level in self.access_range.base_mip_level..end_mip_level {
                    let idx = self
                        .image
                        .idx(base_aspect + self.aspect, array_layer, mip_level);
                    let access = unsafe { self.image.accesses.get_unchecked_mut(idx) };
                    *access = self.access;
                }
            }

            res.aspect_mask |= vk::ImageAspectFlags::from_raw(
                (1 << (self.access_range.base_aspect + self.aspect)) as _,
            )
        }
    }
}

#[derive(Copy, Clone)]
struct ImageAccessRange {
    aspect_count: u8,
    base_array_layer: u32,
    base_aspect: u8,
    base_mip_level: u32,
    layer_count: u32,
    level_count: u32,
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
    pub array_layer_count: u32,

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
        array_layer_count: u32,
        fmt: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> Self {
        Self {
            ty,
            width,
            height,
            depth,
            array_layer_count,
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
        let (ty, extent, array_layer_count) = match self.ty {
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
                self.array_layer_count,
            ),
            ImageType::Texture2D => (
                vk::ImageType::TYPE_2D,
                vk::Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
                if self.flags.contains(vk::ImageCreateFlags::CUBE_COMPATIBLE) {
                    self.array_layer_count
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
                self.array_layer_count,
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
                6 * self.array_layer_count,
            ),
        };

        vk::ImageCreateInfo::default()
            .flags(self.flags)
            .image_type(ty)
            .format(self.fmt)
            .extent(extent)
            .mip_levels(self.mip_level_count)
            .array_layers(array_layer_count)
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
            array_layer_count: Some(self.array_layer_count),
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

impl From<ImageViewInfo> for vk::ImageSubresourceRange {
    fn from(info: ImageViewInfo) -> Self {
        Self {
            aspect_mask: info.aspect_mask,
            base_mip_level: info.base_mip_level,
            base_array_layer: info.base_array_layer,
            layer_count: info.array_layer_count,
            level_count: info.mip_level_count,
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
                level_count: info.mip_level_count,
                layer_count: info.array_layer_count,
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
    /// The default value is `vk::REMAINING_ARRAY_LAYERS`.
    #[builder(default = "vk::REMAINING_ARRAY_LAYERS")]
    pub array_layer_count: u32,

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
    /// The default value is `vk::REMAINING_MIP_LEVELS`.
    #[builder(default = "vk::REMAINING_MIP_LEVELS")]
    pub mip_level_count: u32,

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
            array_layer_count: vk::REMAINING_ARRAY_LAYERS,
            aspect_mask: format_aspect_mask(fmt),
            base_array_layer: 0,
            base_mip_level: 0,
            fmt,
            mip_level_count: vk::REMAINING_MIP_LEVELS,
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
            array_layer_count: info.array_layer_count,
            aspect_mask: format_aspect_mask(info.fmt),
            base_array_layer: 0,
            base_mip_level: 0,
            fmt: info.fmt,
            mip_level_count: info.mip_level_count,
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
    use {super::*, std::ops::Range};

    // ImageSubresourceRange does not implement PartialEq
    fn assert_access_ranges_eq(
        lhs: (AccessType, vk::ImageSubresourceRange),
        rhs: (AccessType, vk::ImageSubresourceRange),
    ) {
        assert_eq!(
            (
                lhs.0,
                lhs.1.aspect_mask,
                lhs.1.base_array_layer,
                lhs.1.layer_count,
                lhs.1.base_mip_level,
                lhs.1.level_count
            ),
            (
                rhs.0,
                rhs.1.aspect_mask,
                rhs.1.base_array_layer,
                rhs.1.layer_count,
                rhs.1.base_mip_level,
                rhs.1.level_count
            )
        );
    }

    #[test]
    pub fn image_access_basic() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::R8G8B8A8_UNORM, 1, 1));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::COLOR, 0..1, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

    #[test]
    pub fn image_access_color() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::R8G8B8A8_UNORM, 3, 3));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::COLOR, 0..3, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::COLOR, 0..3, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::ComputeShaderWrite,
                image_subresource_range(A::COLOR, 0..3, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..1, 1..3),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 1..3, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostRead,
                image_subresource_range(A::COLOR, 0..3, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::ComputeShaderWrite,
                    image_subresource_range(A::COLOR, 0..3, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostWrite,
                image_subresource_range(A::COLOR, 1..2, 1..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 1..2, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::GeometryShaderReadOther,
                image_subresource_range(A::COLOR, 0..3, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 0..1, 0..3),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 1..2, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostWrite,
                    image_subresource_range(A::COLOR, 1..2, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 1..2, 2..3),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 2..3, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::VertexBuffer,
                image_subresource_range(A::COLOR, 0..3, 1..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 0..3, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::ColorAttachmentRead,
                image_subresource_range(A::COLOR, 0..3, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::VertexBuffer,
                    image_subresource_range(A::COLOR, 0..1, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 0..1, 2..3),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 1..2, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::VertexBuffer,
                    image_subresource_range(A::COLOR, 1..2, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 1..2, 2..3),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 2..3, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::VertexBuffer,
                    image_subresource_range(A::COLOR, 2..3, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::GeometryShaderReadOther,
                    image_subresource_range(A::COLOR, 2..3, 2..3),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

    #[test]
    pub fn image_access_layers() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::R8G8B8A8_UNORM, 3, 1));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::COLOR, 0..3, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::COLOR, 0..3, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 2..3, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 2..3, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostRead,
                image_subresource_range(A::COLOR, 0..2, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..2, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 1..2, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 1..2, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostWrite,
                image_subresource_range(A::COLOR, 0..3, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::COLOR, 0..3, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

    #[test]
    pub fn image_access_levels() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::R8G8B8A8_UNORM, 1, 3));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::COLOR, 0..1, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::COLOR, 0..1, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 2..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..1, 2..3),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostRead,
                image_subresource_range(A::COLOR, 0..1, 0..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::COLOR, 0..1, 0..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 0..1, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::COLOR, 0..1, 1..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::HostRead,
                    image_subresource_range(A::COLOR, 0..1, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostWrite,
                image_subresource_range(A::COLOR, 0..1, 0..3),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::COLOR, 0..1, 0..3),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

    #[test]
    pub fn image_access_depth_stencil() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::D24_UNORM_S8_UINT, 4, 3));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::DEPTH, 0..4, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::DEPTH, 0..4, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::STENCIL, 0..4, 1..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 0..4, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::DEPTH | A::STENCIL, 0..4, 0..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::DEPTH, 0..1, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::DEPTH, 0..1, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::DEPTH, 1..2, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::DEPTH, 1..2, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::DEPTH, 2..3, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::DEPTH, 2..3, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::DEPTH, 3..4, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::DEPTH, 3..4, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 0..1, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 0..1, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 1..2, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 1..2, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 2..3, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 2..3, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 3..4, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 3..4, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AccelerationStructureBuildWrite,
                image_subresource_range(A::DEPTH | A::STENCIL, 0..4, 0..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::DEPTH | A::STENCIL, 0..4, 0..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AccelerationStructureBuildRead,
                image_subresource_range(A::DEPTH, 1..3, 0..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AccelerationStructureBuildWrite,
                    image_subresource_range(A::DEPTH, 1..3, 0..2),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

    #[test]
    pub fn image_access_stencil() {
        use vk::ImageAspectFlags as A;

        let mut image = ImageAccess::new(image_subresource(vk::Format::S8_UINT, 2, 2));

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderWrite,
                image_subresource_range(A::STENCIL, 0..2, 0..1),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 0..2, 0..1),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::AnyShaderReadOther,
                image_subresource_range(A::STENCIL, 0..2, 1..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::Nothing,
                    image_subresource_range(A::STENCIL, 0..2, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }

        {
            let mut accesses = ImageAccessIter::new(
                &mut image,
                AccessType::HostRead,
                image_subresource_range(A::STENCIL, 0..2, 0..2),
            );

            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 0..1, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::STENCIL, 0..1, 1..2),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderWrite,
                    image_subresource_range(A::STENCIL, 1..2, 0..1),
                ),
            );
            assert_access_ranges_eq(
                accesses.next().unwrap(),
                (
                    AccessType::AnyShaderReadOther,
                    image_subresource_range(A::STENCIL, 1..2, 1..2),
                ),
            );
            assert!(accesses.next().is_none());
        }
    }

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
            .array_layer_count(100)
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

    fn image_subresource(
        fmt: vk::Format,
        array_layer_count: u32,
        mip_level_count: u32,
    ) -> ImageInfo {
        ImageInfo::image_2d(1, 1, fmt, vk::ImageUsageFlags::empty())
            .to_builder()
            .array_layer_count(array_layer_count)
            .mip_level_count(mip_level_count)
            .build()
    }

    fn image_subresource_range(
        aspect_mask: vk::ImageAspectFlags,
        array_layers: Range<u32>,
        mip_levels: Range<u32>,
    ) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange {
            aspect_mask,
            base_array_layer: array_layers.start,
            base_mip_level: mip_levels.start,
            layer_count: array_layers.len() as _,
            level_count: mip_levels.len() as _,
        }
    }

    #[test]
    pub fn image_subresource_range_contains() {
        use {
            super::image_subresource_range_contains as f, image_subresource_range as i,
            vk::ImageAspectFlags as A,
        };

        assert!(f(i(A::COLOR, 0..1, 0..1), i(A::COLOR, 0..1, 0..1)));
        assert!(f(i(A::COLOR, 0..2, 0..1), i(A::COLOR, 0..1, 0..1)));
        assert!(f(i(A::COLOR, 0..1, 0..2), i(A::COLOR, 0..1, 0..1)));
        assert!(f(i(A::COLOR, 0..2, 0..2), i(A::COLOR, 0..1, 0..1)));
        assert!(!f(i(A::COLOR, 0..1, 1..3), i(A::COLOR, 0..1, 0..1)));
        assert!(!f(i(A::COLOR, 1..3, 0..1), i(A::COLOR, 0..1, 0..1)));
        assert!(!f(i(A::COLOR, 0..1, 1..3), i(A::COLOR, 0..1, 0..2)));
        assert!(!f(i(A::COLOR, 1..3, 0..1), i(A::COLOR, 0..2, 0..1)));
    }

    #[test]
    pub fn image_subresource_range_intersects() {
        use {
            super::image_subresource_range_intersects as f, image_subresource_range as i,
            vk::ImageAspectFlags as A,
        };

        assert!(f(i(A::COLOR, 0..1, 0..1), i(A::COLOR, 0..1, 0..1)));
        assert!(!f(i(A::COLOR, 0..1, 0..1), i(A::DEPTH, 0..1, 0..1)));

        assert!(!f(i(A::COLOR, 0..1, 0..1), i(A::COLOR, 1..2, 0..1)));
        assert!(!f(i(A::COLOR, 0..1, 0..1), i(A::COLOR, 0..1, 1..2)));
        assert!(!f(i(A::COLOR, 0..1, 0..1), i(A::DEPTH, 1..2, 0..1)));
        assert!(!f(i(A::COLOR, 0..1, 0..1), i(A::DEPTH, 0..1, 1..2)));
        assert!(!f(i(A::COLOR, 1..2, 1..2), i(A::COLOR, 0..1, 0..1)));

        assert!(f(
            i(A::DEPTH | A::STENCIL, 2..3, 3..5),
            i(A::DEPTH, 2..3, 2..4)
        ));
        assert!(f(
            i(A::DEPTH | A::STENCIL, 2..3, 3..5),
            i(A::DEPTH, 2..3, 4..6)
        ));
        assert!(!f(
            i(A::DEPTH | A::STENCIL, 2..3, 3..5),
            i(A::DEPTH, 2..3, 2..3)
        ));
        assert!(!f(
            i(A::DEPTH | A::STENCIL, 2..3, 3..5),
            i(A::DEPTH, 2..3, 5..6)
        ));
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
