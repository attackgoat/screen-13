//! Window-based inputs captured during event loop processing.

mod key_buf;
mod mouse_buf;
mod typing;

pub use self::{key_buf::KeyBuf, mouse_buf::MouseBuf, typing::Typing};

/// A container of Window input buffers.
#[derive(Default)]
pub struct Input {
    /// Gets current keyboard inputs.
    pub key: KeyBuf,

    /// Gets current mouse/tablet/touch inputs.
    pub mouse: MouseBuf,
}

// TODO: Should we add 'normal' keys as something like `Other(char)`?
/// Keys that can be detected as pressed or released.
#[derive(Debug, PartialEq)]
pub enum Key {
    /// Back
    Back,

    /// Left Arrow
    Left,

    /// Delete
    Delete,

    /// Right Arrow
    Right,

    /// Up Arrow
    Up,

    /// Down Arrow
    Down,

    /// Home
    Home,

    /// End
    End,
}
