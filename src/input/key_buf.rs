use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};

/// A container for Window-based keyboard input events.
///
/// NOTE: Keys pressed and released during a single update (is that possible, really?) will show
/// as both released and pressed.
#[derive(Clone, Debug, Default)]
pub struct KeyBuf {
    chars: Vec<char>,
    held: Vec<VirtualKeyCode>,
    pressed: Vec<VirtualKeyCode>,
    released: Vec<VirtualKeyCode>,
}

impl KeyBuf {
    pub fn any_held(&self) -> bool {
        !self.held.is_empty()
    }

    pub fn any_pressed(&self) -> bool {
        !self.pressed.is_empty()
    }

    pub fn any_released(&self) -> bool {
        !self.released.is_empty()
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.chars.iter().copied()
    }

    /// Call this before handling events
    pub fn update(&mut self) {
        self.chars.clear();
        self.pressed.clear();
        self.released.clear();
    }

    pub fn handle_event(&mut self, event: &Event<'_, ()>) -> bool {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } if input.virtual_keycode.is_some() => {
                    let key = input.virtual_keycode.unwrap();
                    match input.state {
                        ElementState::Pressed => {
                            self.pressed
                                .insert(self.pressed.binary_search(&key).err().unwrap(), key);
                            self.held
                                .insert(self.held.binary_search(&key).ok().unwrap(), key);
                        }
                        ElementState::Released => {
                            self.held
                                .remove(self.held.binary_search(&key).ok().unwrap());
                            self.released
                                .insert(self.released.binary_search(&key).err().unwrap(), key);
                        }
                    }

                    true
                }
                WindowEvent::ReceivedCharacter(char) => {
                    self.chars.push(*char);

                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_held(&self, key: &VirtualKeyCode) -> bool {
        self.held.binary_search(key).is_ok()
    }

    pub fn is_pressed(&self, key: &VirtualKeyCode) -> bool {
        self.pressed.binary_search(key).is_ok()
    }

    pub fn is_released(&self, key: &VirtualKeyCode) -> bool {
        self.released.binary_search(key).is_ok()
    }

    pub fn held(&self) -> impl Iterator<Item = VirtualKeyCode> + '_ {
        self.held.iter().copied()
    }

    pub fn pressed(&self) -> impl Iterator<Item = VirtualKeyCode> + '_ {
        self.pressed.iter().copied()
    }

    pub fn released(&self) -> impl Iterator<Item = VirtualKeyCode> + '_ {
        self.released.iter().copied()
    }
}
