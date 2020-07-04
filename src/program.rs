/// Program is the required information to start a game
/// TODO: Add icons and such
pub struct Program<'a> {
    pub name: &'static str,
    pub window_title: &'a str,
}

impl<'a> Program<'a> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            window_title: name,
        }
    }

    pub fn with_window_title(&mut self, window_title: &'a str) {
        self.window_title = window_title;
    }
}
