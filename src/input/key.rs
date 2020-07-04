use winit::event::KeyboardInput;

use super::KeyCode;

const DEFAULT_EVENT_CAPACITY: usize = 16;

pub struct KeyBuf {
    char_buf: String,
    pressed_keys: Vec<KeyCode>,
    released_keys: Vec<KeyCode>,
}

impl KeyBuf {
    pub fn clear(&mut self) {
        self.char_buf.clear();
        self.pressed_keys.clear();
        self.released_keys.clear();
    }

    pub fn char_buf(&self) -> &str {
        &self.char_buf
    }

    pub fn handle(&mut self, _event: &KeyboardInput) {
        /*match event {
            Event::KeyboardInput(state, _, Some(key_code)) => self.handle_key(*state, *key_code),
            Event::ReceivedCharacter(chr) => self.handle_char(*chr),
            _ => unimplemented!(),
        }*/
    }

    fn handle_char(&mut self, chr: char) {
        if !chr.is_control() {
            self.char_buf.push(chr)
        }
    }

    /*fn handle_key(&mut self, state: ElementState, key_code: KeyCode) {
        match state {
            ElementState::Pressed => self.pressed_keys.push(key_code),
            ElementState::Released => {
                self.pressed_keys.retain(|&k| k != key_code);
                self.released_keys.push(key_code);
            }
        }
    }*/

    pub fn is_key_down(&self, key_code: KeyCode) -> bool {
        self.pressed_keys.contains(&key_code)
    }

    pub fn was_key_down(&self, key_code: KeyCode) -> bool {
        self.released_keys.contains(&key_code)
    }
}

impl Default for KeyBuf {
    fn default() -> Self {
        Self {
            char_buf: String::with_capacity(DEFAULT_EVENT_CAPACITY),
            pressed_keys: Vec::with_capacity(DEFAULT_EVENT_CAPACITY),
            released_keys: Vec::with_capacity(DEFAULT_EVENT_CAPACITY),
        }
    }
}
