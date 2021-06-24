use {super::Key, winit::event::KeyboardInput};

const DEFAULT_EVENT_CAPACITY: usize = 16;

/// A container for Window-based keyboard input events.
#[derive(Debug)]
pub struct KeyBuf {
    char_buf: String,
    pressed_keys: Vec<Key>,
    released_keys: Vec<Key>,
}

impl KeyBuf {
    /// Returns `true` if any key is physically pressed down right now.
    pub fn any_down(&self) -> bool {
        !self.pressed_keys.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.char_buf.clear();
        self.pressed_keys.clear();
        self.released_keys.clear();
    }

    pub(crate) fn char_buf(&self) -> &str {
        &self.char_buf
    }

    pub(crate) fn handle(&mut self, _event: &KeyboardInput) {
        /*match event {
            Event::KeyboardInput(state, _, Some(key_code)) => self.handle_key(*state, *key_code),
            Event::ReceivedCharacter(char) => self.handle_char(*char),
            _ => unimplemented!(),
        }*/
    }

    fn handle_char(&mut self, char: char) {
        if !char.is_control() {
            self.char_buf.push(char)
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

    /// Returns `true` if the given key is physically pressed down right now.
    pub fn is_down(&self, key_code: Key) -> bool {
        self.pressed_keys.contains(&key_code)
    }

    /// Returns `true` if the given key was physically released just now.
    pub fn was_down(&self, key_code: Key) -> bool {
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
