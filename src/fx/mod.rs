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
