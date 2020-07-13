mod key_buf;
mod mouse_buf;
mod typing;

pub use self::{key_buf::KeyBuf, mouse_buf::MouseBuf, typing::Typing};

#[derive(Default)]
pub struct Input {
    pub keys: KeyBuf,
    pub mouse: MouseBuf,
}

// TODO: Should we add 'normal' keys as something like `Other(char)`?
#[derive(PartialEq)]
pub enum Key {
    Back,
    Left,
    Delete,
    Right,
    Up,
    Down,
    Home,
    End,
}
