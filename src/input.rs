//! User input.

use rendy::init::winit::event::{Event, WindowEvent};

#[derive(Clone, Debug)]
pub enum UserInput {
    None,
    TerminationRequested,
    Resized((u32, u32)),
    CursorMoved((i32, i32)),
}

impl UserInput {
    pub fn from_event(event: Event<()>) -> Self {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => Self::TerminationRequested,
            Event::WindowEvent {
                event: WindowEvent::Resized(physical_size),
                ..
            } => Self::Resized((physical_size.width, physical_size.height)),
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => Self::CursorMoved((position.x, position.y)),
            _ => Self::None,
        }
    }
}
