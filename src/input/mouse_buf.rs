pub use winit::event::MouseButton;

use winit::event::{ElementState, Event, MouseScrollDelta, TouchPhase, WindowEvent};

const fn mouse_button_idx(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(idx) => idx,
    }
}

#[allow(dead_code)]
const fn idx_mouse_button(button: u16) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        idx => MouseButton::Other(idx),
    }
}

/// A container for Window-based mouse, tablet and touch input events.
#[derive(Clone, Debug, Default)]
pub struct MouseBuf {
    /// Amount of mouse movement detected since the last update.
    pub delta: (f32, f32),

    held: u16,
    position: Option<(f32, f32)>,
    pressed: u16,
    released: u16,

    /// Amount of wheel scroll detected since the last update.
    pub wheel: (f32, f32),

    /// The x-axis mouse position in pixels.
    pub x: f32,

    /// The y-axis mouse position in pixels.
    pub y: f32,
}

impl MouseBuf {
    /// Returns `true` if any buttons have been pressed for multiple frames.
    pub fn any_held(&self) -> bool {
        self.held != 0
    }

    /// Returns `true` if any buttons have been pressed since the last frame.
    pub fn any_pressed(&self) -> bool {
        self.pressed != 0
    }

    /// Returns `true` if any buttons have been released since the last frame.
    pub fn any_released(&self) -> bool {
        self.released != 0
    }

    const fn bit(button: MouseButton) -> u16 {
        1 << mouse_button_idx(button)
    }

    /// Call this before handling events.
    pub fn update(&mut self) {
        self.delta = (0.0, 0.0);
        self.pressed = 0;
        self.released = 0;
        self.wheel = (0.0, 0.0);
    }

    /// Handles a single event.
    pub fn handle_event(&mut self, event: &Event<()>) -> bool {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CursorMoved { position, .. } => {
                    // TODO: Reckon with "it should not be used to implement non-cursor-like interactions such as 3D camera control"
                    let prev_position = self.position;
                    self.position = Some((position.x as _, position.y as _));
                    let position = self.position.unwrap_or_default();
                    let prev_position = prev_position.unwrap_or(position);
                    self.delta.0 += position.0 - prev_position.0;
                    self.delta.1 += position.1 - prev_position.1;
                    self.x = position.0;
                    self.y = position.1;

                    true
                }
                WindowEvent::MouseInput { button, state, .. } => {
                    match state {
                        ElementState::Pressed => {
                            self.pressed |= Self::bit(*button);
                            self.held |= Self::bit(*button);
                        }
                        ElementState::Released => {
                            self.held &= !Self::bit(*button);
                            self.released &= !Self::bit(*button);
                        }
                    }

                    true
                }
                WindowEvent::MouseWheel { delta, phase, .. } if *phase == TouchPhase::Moved => {
                    let (x, y) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                        MouseScrollDelta::PixelDelta(p) => (p.x as _, p.y as _),
                    };
                    self.wheel.0 += x;
                    self.wheel.1 += y;

                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Returns `true` if the given button has been pressed for multiple frames.
    pub fn is_held(&self, button: MouseButton) -> bool {
        self.held & Self::bit(button) != 0
    }

    /// Returns `true` if the given button has been pressed since the last frame.
    pub fn is_pressed(&self, button: MouseButton) -> bool {
        self.pressed & Self::bit(button) != 0
    }

    /// Returns `true` if the given button has been released since the last frame.
    pub fn is_released(&self, button: MouseButton) -> bool {
        self.released & Self::bit(button) != 0
    }

    /// Centered around zero, so negative values are the bottom left of the screen.
    ///
    /// Units are unspecified and vary by device.
    pub fn position(&self) -> (f32, f32) {
        self.position.unwrap_or_default()
    }
}
