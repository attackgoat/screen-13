//! Contains pre-made implementations of Screen 13 traits.
//! 
//! This module is intended to help you get started quickly by providing high quality solutions
//! to common development scenarios.

mod fade;
mod solid;

pub use self::{fade::Fade, solid::Solid};
