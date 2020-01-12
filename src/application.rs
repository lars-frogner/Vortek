//! Application.

use crate::{color::Color, input::UserInput};

pub struct ApplicationState {
    physical_window_size: (u32, u32),
    current_background_color: Color,
}

impl ApplicationState {
    pub fn new(physical_window_size: (u32, u32), default_background_color: Color) -> Self {
        Self {
            physical_window_size,
            current_background_color: default_background_color,
        }
    }

    pub fn update_from_input(&mut self, input: &UserInput) {
        if let UserInput::CursorMoved((x, y)) = *input {
            let r = x as f32 / (self.physical_window_size.0 as f32);
            let g = y as f32 / (self.physical_window_size.1 as f32);
            let b = (r + g) * 0.3;
            let a = 1.0;
            self.current_background_color = Color::from_components(r, g, b, a);
        }
    }

    pub fn background_color(&self) -> &Color {
        &self.current_background_color
    }
}
