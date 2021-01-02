use crate::math::Coord;

/// A container for Window-based mouse, tablet and touch input events.
#[derive(Default)]
pub struct MouseBuf {}

impl MouseBuf {
    /// Returns the mouse position relative to the center of the screen in pixel coordinates. When
    /// at rest the mouse position will be (0,0) and moving to the left or up will produce negative values.
    /// The mouse position is reset to (0,0) before each engine update.
    pub fn pos(&self) -> Coord {
        todo!()
    }
}
