use {super::KeyBuf, winit::event::VirtualKeyCode};

#[deprecated]
#[derive(Default, Eq, PartialEq)]
pub struct Typing {
    buf: String,
    pos: usize,
}

#[allow(deprecated)]
impl Typing {
    /// TODO
    #[allow(unused)]
    pub fn as_split_str(&self) -> (&str, &str) {
        self.buf.split_at(self.pos)
    }

    /// TODO
    #[allow(unused)]
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// TODO
    #[allow(unused)]
    pub fn handle_input(&mut self, input: &KeyBuf) {
        // Handle adding new character input
        let chars: String = input.chars().collect();
        if !chars.is_empty() {
            if self.pos == self.buf.len() {
                self.buf.push_str(&chars);
            } else {
                self.buf.insert_str(self.pos, &chars);
            }

            self.pos += chars.len();
        }

        // Handle back/forward delete and cursor movement
        if input.is_pressed(&VirtualKeyCode::Back) && 0 < self.pos {
            if self.pos == self.buf.len() {
                if let Some(c) = self.buf.pop() {
                    self.pos -= c.len_utf8();
                }
            } else {
                let (mut lhs, rhs) = self.to_split_string();
                if let Some(c) = lhs.pop() {
                    self.pos -= c.len_utf8();
                }

                self.buf.clear();
                self.buf.push_str(&lhs);
                self.buf.push_str(&rhs);
            }
        } else if input.is_pressed(&VirtualKeyCode::Delete) && self.pos < self.buf.len() {
            let (_, rhs) = self.to_split_string();
            let mut rhs = rhs.chars();
            rhs.next();
            self.buf.truncate(self.pos);
            self.buf.push_str(rhs.as_str());
        } else if input.is_pressed(&VirtualKeyCode::Left) && 0 < self.pos {
            let (lhs, _) = self.buf.split_at(self.pos);
            if let Some(c) = lhs.chars().last() {
                self.pos -= c.len_utf8();
            }
        } else if input.is_pressed(&VirtualKeyCode::Right) && self.pos < self.buf.len() {
            let (_, rhs) = self.buf.split_at(self.pos);
            if let Some(c) = rhs.chars().next() {
                self.pos += c.len_utf8();
            }
        } else if input.is_pressed(&VirtualKeyCode::Home) {
            self.pos = 0;
        } else if input.is_pressed(&VirtualKeyCode::End) {
            self.pos = self.buf.len();
        }
    }

    /// TODO
    #[allow(unused)]
    pub fn to_split_string(&self) -> (String, String) {
        let (lhs, rhs) = self.as_split_str();
        (lhs.to_owned(), rhs.to_owned())
    }
}
