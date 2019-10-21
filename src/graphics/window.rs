//! Creation and management of rendering windows.

use crate::error::{VortekError, VortekResult};
use std::{borrow::Cow, fmt};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    EventsLoop, Window, WindowBuilder,
};

const DEFAULT_WINDOW_NAME: &str = "Vortek";

/// Manages window state.
pub struct WindowState {
    event_loop: EventsLoop,
    window: Window,
    window_title: String,
}

/// Error structure for window operations.
#[derive(Clone, Debug)]
pub struct WindowError {
    message: Cow<'static, str>,
}

impl WindowError {
    /// Returns the error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    fn from_error<E: fmt::Display>(front_message: &'static str, error: E) -> Self {
        Self {
            message: Cow::from(format!("{}{}", front_message, error)),
        }
    }

    fn from_str(message: &'static str) -> Self {
        Self {
            message: Cow::from(message),
        }
    }
}

impl WindowState {
    /// Creates a new window state object holding a window with the given title and dimensions.
    pub fn new<T: Into<String> + Clone>(title: T, dimensions: LogicalSize) -> VortekResult<Self> {
        let event_loop = Self::crate_event_loop();
        let window_result = Self::create_window(&event_loop, title.clone(), dimensions);
        window_result.map(|window| Self {
            event_loop,
            window,
            window_title: title.into(),
        })
    }

    /// Returns a reference to the event loop.
    pub fn event_loop(&self) -> &EventsLoop {
        &self.event_loop
    }

    /// Returns a mutable reference to the event loop.
    pub fn event_loop_mut(&mut self) -> &mut EventsLoop {
        &mut self.event_loop
    }

    /// Returns a reference to the window.
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Returns a reference to the window title string.
    pub fn window_title(&self) -> &str {
        &self.window_title
    }

    /// Computes the physical size of the window.
    pub fn compute_physical_size(&self) -> VortekResult<PhysicalSize> {
        self.window
            .get_inner_size()
            .map(|logical_size| logical_size.to_physical(self.window.get_hidpi_factor()))
            .ok_or_else(|| {
                VortekError::WindowError(WindowError::from_str(
                    "Could not compute physical window size since window no longer exists.",
                ))
            })
    }

    fn crate_event_loop() -> EventsLoop {
        EventsLoop::new()
    }

    fn create_window<T: Into<String>>(
        event_loop: &EventsLoop,
        title: T,
        size: LogicalSize,
    ) -> VortekResult<Window> {
        WindowBuilder::new()
            .with_title(title)
            .with_dimensions(size)
            .build(event_loop)
            .map_err(|err| VortekError::WindowError(WindowError::from_error("", err)))
    }
}

impl Default for WindowState {
    /// Creates a new window state object with a default title and dimensions 800x600.
    fn default() -> Self {
        let title = DEFAULT_WINDOW_NAME;
        let dimensions = LogicalSize {
            width: 800.0,
            height: 600.0,
        };
        Self::new(title, dimensions)
            .unwrap_or_else(|err| panic!("Could not create window: {}", err))
    }
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
