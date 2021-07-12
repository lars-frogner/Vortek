//! Creation and management of rendering windows.

use rendy::init::winit::{
    dpi::{LogicalSize, Size},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub const DEFAULT_WINDOW_NAME: &str = "Vortek";
pub const DEFAULT_WINDOW_SIZE: LogicalSize<f64> = LogicalSize::new(800.0, 600.0);

/// Creates a new event loop.
pub fn create_event_loop() -> EventLoop<()> {
    EventLoop::new()
}

/// Creates a new builder for a window with the given title and inner dimensions.
pub fn create_window_builder<T: Into<String> + Clone, S: Into<Size>>(
    title: T,
    dimensions: S,
) -> WindowBuilder {
    WindowBuilder::new()
        .with_title(title)
        .with_inner_size(dimensions)
}
