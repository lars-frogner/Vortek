//! Backend management.

use super::{super::window::WindowState, adapter::AdapterState};
use crate::error::VortekResult;
use gfx_hal::{Backend, Instance};

#[cfg(feature = "dx12")]
use gfx_backend_dx12 as backend;
#[cfg(feature = "metal")]
use gfx_backend_metal as backend;
#[cfg(feature = "vulkan")]
use gfx_backend_vulkan as backend;

pub type BackendType = backend::Backend;

/// Structure for managing backend state.
pub struct BackendState<B: Backend> {
    window_state: WindowState,
    surface: B::Surface,
    adapter_state: AdapterState<B>,
}

impl<B: Backend> BackendState<B> {
    /// Returns a reference to the window state held by the backend state.
    pub fn window_state(&self) -> &WindowState {
        &self.window_state
    }

    /// Returns a mutable reference to the window state held by the backend state.
    pub fn window_state_mut(&mut self) -> &mut WindowState {
        &mut self.window_state
    }

    /// Returns a reference to the surface held by the backend state.
    pub fn surface(&self) -> &B::Surface {
        &self.surface
    }

    /// Returns a mutable reference to the surface held by the backend state.
    pub fn surface_mut(&mut self) -> &mut B::Surface {
        &mut self.surface
    }

    /// Returns a reference to the adapter state held by the backend state.
    pub fn adapter_state(&self) -> &AdapterState<B> {
        &self.adapter_state
    }

    /// Returns a mutable reference to the adapter state held by the backend state.
    pub fn adapter_state_mut(&mut self) -> &mut AdapterState<B> {
        &mut self.adapter_state
    }
}

/// Creates a new backend state from the given window state.
pub fn create_backend_state(
    window_state: WindowState,
) -> VortekResult<(
    BackendState<<backend::Instance as Instance>::Backend>,
    backend::Instance,
)> {
    let instance = backend::Instance::create(window_state.window_title(), 1);
    let surface = instance.create_surface(window_state.window());
    let adapter_state = AdapterState::new(instance.enumerate_adapters(), &surface)?;
    Ok((
        BackendState {
            window_state,
            surface,
            adapter_state,
        },
        instance,
    ))
}
