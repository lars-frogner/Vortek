//! Creation and management of rendering windows.

use crate::error::{VortekError, VortekResult};
use std::{borrow::Cow, fmt};
use winit::{
    dpi::{LogicalSize, PhysicalSize, Size},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub const DEFAULT_WINDOW_NAME: &str = "Vortek";
pub const DEFAULT_WINDOW_SIZE: LogicalSize<f64> = LogicalSize::new(800.0, 600.0);

/// Manages window state.
pub struct WindowState {
    window: Window,
    window_title: String,
}

/// Error structure for window operations.
#[derive(Clone, Debug)]
pub struct WindowError {
    message: Cow<'static, str>,
}

/// Creates a new window state object and an associated event loop.
pub fn create_window_and_event_loop<T: Into<String> + Clone, S: Into<Size>>(
    title: T,
    dimensions: S,
) -> VortekResult<(WindowState, EventLoop<()>)> {
    let event_loop = EventLoop::new();
    WindowState::new(&event_loop, title, dimensions).map(|window_state| (window_state, event_loop))
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

    #[allow(dead_code)]
    fn from_str(message: &'static str) -> Self {
        Self {
            message: Cow::from(message),
        }
    }
}

impl WindowState {
    /// Creates a new window state object holding a window associated with
    /// the given event loop and with the given title and dimensions.
    fn new<T: Into<String> + Clone, S: Into<Size>>(
        event_loop: &EventLoop<()>,
        title: T,
        dimensions: S,
    ) -> VortekResult<Self> {
        let window_result = Self::create_window(event_loop, title.clone(), dimensions);
        window_result.map(|window| Self {
            window,
            window_title: title.into(),
        })
    }

    /// Returns a reference to the window.
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Returns a reference to the window title string.
    pub fn window_title(&self) -> &str {
        &self.window_title
    }

    /// Returns the logical size of the window's client area.
    pub fn inner_logical_size(&self) -> LogicalSize<f64> {
        self.inner_physical_size()
            .to_logical(self.window.scale_factor())
    }

    /// Returns the physical size of the window's client area.
    pub fn inner_physical_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn create_window<T: Into<String>, S: Into<Size>>(
        event_loop: &EventLoop<()>,
        title: T,
        size: S,
    ) -> VortekResult<Window> {
        WindowBuilder::new()
            .with_title(title)
            .with_inner_size(size)
            .build(event_loop)
            .map_err(|err| {
                VortekError::WindowError(WindowError::from_error("Could not create window: ", err))
            })
    }
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
