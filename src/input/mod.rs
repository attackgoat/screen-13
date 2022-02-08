//! Window-based inputs captured during event loop processing.

mod key_buf;
mod key_map;
mod mouse_buf;
mod typing;

use winit::event::Event;

pub use self::{
    key_buf::KeyBuf,
    key_map::KeyMap,
    mouse_buf::{MouseBuf, MouseButton},
    typing::Typing,
};

// Handles keyboard and mouse `Event`s while updating the provided buffers.
pub fn update_input<'a>(
    keyboard: &'a mut KeyBuf,
    mouse: &'a mut MouseBuf,
    events: impl IntoIterator<Item = &'a Event<'a, ()>>,
) {
    update_input_opt(Some(keyboard), Some(mouse), events);
}

// Handles keyboard `Event`s while updating the provided buffer.
pub fn update_keyboard<'a>(
    keyboard: &'a mut KeyBuf,
    events: impl IntoIterator<Item = &'a Event<'a, ()>>,
) {
    update_input_opt(Some(keyboard), None, events);
}

// Handles mouse `Event`s while updating the provided buffer.
pub fn update_mouse<'a>(
    mouse: &'a mut MouseBuf,
    events: impl IntoIterator<Item = &'a Event<'a, ()>>,
) {
    update_input_opt(None, Some(mouse), events);
}

fn update_input_opt<'a>(
    mut keyboard: Option<&'a mut KeyBuf>,
    mut mouse: Option<&'a mut MouseBuf>,
    events: impl IntoIterator<Item = &'a Event<'a, ()>>,
) {
    if let Some(keyboard) = keyboard.as_mut() {
        keyboard.update();
    }

    if let Some(mouse) = mouse.as_mut() {
        mouse.update();
    }

    for event in events.into_iter() {
        if let Some(keyboard) = keyboard.as_mut() {
            keyboard.handle_event(event);
        }

        if let Some(mouse) = mouse.as_mut() {
            mouse.handle_event(event);
        }
    }
}
