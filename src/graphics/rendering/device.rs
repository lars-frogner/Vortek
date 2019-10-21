//! Device management.

use super::RenderingError;
use crate::error::{VortekError, VortekResult};
use gfx_hal::{
    adapter::{Adapter, PhysicalDevice},
    queue::{QueueFamily, QueueGroup, Queues},
    window::Surface,
    Backend, Features, Gpu, Graphics,
};

/// Structure for managing device state.
pub struct DeviceState<B: Backend> {
    device: B::Device,
    physical_device: B::PhysicalDevice,
    queue_group: QueueGroup<B, Graphics>,
}

impl<B: Backend> DeviceState<B> {
    /// Creates a new device state from the given adapter.
    pub fn new(adapter: Adapter<B>, surface: &B::Surface) -> VortekResult<Self> {
        let queue_family = Self::select_queue_family(&adapter.queue_families, surface)?;

        let Gpu { device, mut queues } =
            unsafe { Self::create_logical_device(&adapter.physical_device, queue_family)? };

        let queue_group = Self::take_queue_group(&mut queues, queue_family)?;

        Ok(Self {
            device,
            physical_device: adapter.physical_device,
            queue_group,
        })
    }

    /// Returns a reference to the device held by the device state.
    pub fn device(&self) -> &B::Device {
        &self.device
    }

    /// Returns a reference to the physical device held by the device state.
    pub fn physical_device(&self) -> &B::PhysicalDevice {
        &self.physical_device
    }

    /// Returns a reference to the queue group held by the device state.
    pub fn queue_group(&self) -> &QueueGroup<B, Graphics> {
        &self.queue_group
    }

    /// Returns a mutable reference to the queue group held by the device state.
    pub fn queue_group_mut(&mut self) -> &mut QueueGroup<B, Graphics> {
        &mut self.queue_group
    }

    fn select_queue_family<'a, 'b>(
        queue_families: &'a [<B as Backend>::QueueFamily],
        surface: &'b B::Surface,
    ) -> VortekResult<&'a <B as Backend>::QueueFamily> {
        queue_families
            .iter()
            .find(|family| surface.supports_queue_family(family) && family.supports_graphics())
            .ok_or_else(|| {
                VortekError::RenderingError(RenderingError::from_str(
                    "Could not find supported queue family with graphics.",
                ))
            })
    }

    unsafe fn create_logical_device(
        physical_device: &<B as Backend>::PhysicalDevice,
        queue_family: &<B as Backend>::QueueFamily,
    ) -> VortekResult<Gpu<B>> {
        physical_device
            .open(&[(queue_family, &[1.0; 1])], Features::empty())
            .map_err(|err| {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not open physical device: ",
                    err,
                ))
            })
    }

    fn take_queue_group(
        queues: &mut Queues<B>,
        queue_family: &<B as Backend>::QueueFamily,
    ) -> VortekResult<QueueGroup<B, Graphics>> {
        let queue_group = queues.take::<Graphics>(queue_family.id()).ok_or_else(|| {
            VortekError::RenderingError(RenderingError::from_str(
                "Could not take ownership of queue group.",
            ))
        })?;

        if queue_group.queues.is_empty() {
            Err(VortekError::RenderingError(RenderingError::from_str(
                "Queue group did not have any command queues available.",
            )))
        } else {
            Ok(queue_group)
        }
    }
}
