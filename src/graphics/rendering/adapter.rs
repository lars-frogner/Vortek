//! Adapter management.

use super::RenderingError;
use crate::error::{VortekError, VortekResult};
use gfx_hal::{adapter::Adapter, queue::QueueFamily, window::Surface, Backend};

/// Structure for managing adapter state.
pub struct AdapterState<B: Backend> {
    adapter: Option<Adapter<B>>,
}

impl<B: Backend> AdapterState<B> {
    /// Creates a new adapter state representing the first adaptor supported by the
    /// given surface.
    pub fn new(adapters: Vec<Adapter<B>>, surface: &B::Surface) -> VortekResult<Self> {
        let adapter = Self::select_adapter(adapters, surface)?;
        Ok(Self {
            adapter: Some(adapter),
        })
    }

    /// Moves the adapter out of the adapter state.
    pub fn take_adapter(&mut self) -> Adapter<B> {
        self.adapter.take().expect("No adapter in adapter state.")
    }

    /// Selects the first available adapter with a queue family that supports graphics
    /// and is supported by the surface.
    fn select_adapter(adapters: Vec<Adapter<B>>, surface: &B::Surface) -> VortekResult<Adapter<B>> {
        adapters
            .into_iter()
            .find(|adapter| {
                adapter.queue_families.iter().any(|queue_family| {
                    queue_family.queue_type().supports_graphics()
                        && surface.supports_queue_family(queue_family)
                })
            })
            .ok_or_else(|| {
                VortekError::RenderingError(RenderingError::from_str(
                    "Could not find a supported graphical adapter.",
                ))
            })
    }
}
