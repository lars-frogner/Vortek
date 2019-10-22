//! User input.

use winit::{Event, EventsLoop, WindowEvent};

#[derive(Clone, Debug)]
pub enum UserInput {
    None,
    TerminationRequested,
    Resized((f64, f64)),
}

impl UserInput {
    pub fn poll_event_loop(event_loop: &mut EventsLoop) -> Self {
        let mut input = Self::None;
        event_loop.poll_events(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                input = Self::TerminationRequested;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(logical_size),
                ..
            } => {
                input = UserInput::Resized((logical_size.width, logical_size.height));
            }
            _ => (),
        });
        input
    }
}
