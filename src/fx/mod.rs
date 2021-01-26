//! Contains pre-made implementations of _Screen 13_ traits.
//!
//! This module is intended to help you get started quickly by providing high quality solutions
//! to common development scenarios.
//!
//! **_NOTE:_** `Fade` requires the `blend-modes` feature to be enabled.

#[cfg(feature = "blend-modes")]
mod fade;

mod solid;

#[cfg(feature = "blend-modes")]
pub use self::fade::Fade;

pub use self::solid::Solid;

use crate::gpu::Render;

#[cfg(not(feature = "multi-monitor"))]
type RenderReturn<P> = Render<P>;

#[cfg(feature = "multi-monitor")]
type RenderReturn<P> = Vec<Option<Render<P>>>;
