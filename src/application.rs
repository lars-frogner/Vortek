//! Application.

use crate::{color::Color, input::UserInput};

pub struct ApplicationState {
    background_color: Color,
}

impl ApplicationState {
    pub fn update_from_input(&mut self, input: &UserInput) {}

    pub fn background_color(&self) -> &Color {
        &self.background_color
    }
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            background_color: Color::black(),
        }
    }
}
