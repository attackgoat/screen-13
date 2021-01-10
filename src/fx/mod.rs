//! Contains pre-made implementations of _Screen 13_ traits.
//!
//! This module is intended to help you get started quickly by providing high quality solutions
//! to common development scenarios.

mod fade;
mod solid;

pub use self::{fade::Fade, solid::Solid};
