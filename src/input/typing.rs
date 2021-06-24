use super::{Key, KeyBuf};

/// TODO
#[derive(Default, PartialEq)]
pub struct Typing {
    buf: String,
    pos: usize,
}

impl Typing {
    /// TODO
    pub fn as_split_str(&self) -> (&str, &str) {
        self.buf.split_at(self.pos)
    }

    /// TODO
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// TODO
    pub fn handle_input(&mut self, input: &KeyBuf) {
        // Handle adding new character input
        let chars = input.char_buf();
        if !chars.is_empty() {
            if self.pos == self.buf.len() {
                self.buf.push_str(chars);
            } else {
                self.buf.insert_str(self.pos, chars);
            }

            self.pos += chars.len();
        }

        // Handle back/forward delete and cursor movement
        if input.is_down(Key::Back) && 0 < self.pos {
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
        } else if input.is_down(Key::Delete) && self.pos < self.buf.len() {
            let (_, rhs) = self.to_split_string();
            let mut rhs = rhs.chars();
            rhs.next();
            self.buf.truncate(self.pos);
            self.buf.push_str(rhs.as_str());
        } else if input.is_down(Key::Left) && 0 < self.pos {
            let (lhs, _) = self.buf.split_at(self.pos);
            if let Some(c) = lhs.chars().last() {
                self.pos -= c.len_utf8();
            }
        } else if input.is_down(Key::Right) && self.pos < self.buf.len() {
            let (_, rhs) = self.buf.split_at(self.pos);
            if let Some(c) = rhs.chars().next() {
                self.pos += c.len_utf8();
            }
        } else if input.is_down(Key::Home) {
            self.pos = 0;
        } else if input.is_down(Key::End) {
            self.pos = self.buf.len();
        }
    }

    /// TODO
    pub fn to_split_string(&self) -> (String, String) {
        let (lhs, rhs) = self.as_split_str();
        (lhs.to_owned(), rhs.to_owned())
    }
}
