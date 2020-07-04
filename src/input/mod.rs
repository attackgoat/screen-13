mod key;
mod mouse;
mod typing;

pub use self::{key::KeyBuf, mouse::MouseBuf, typing::Typing};

#[derive(Default)]
pub struct Input {
    pub keys: KeyBuf,
    pub mouse: MouseBuf,
}

#[derive(PartialEq)]
pub enum KeyCode {
    Back,
    Left,
    Delete,
    Right,
    Up,
    Down,
    Home,
    End,
}
