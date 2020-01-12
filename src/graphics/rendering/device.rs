//! Device management.

use super::RenderingError;
use crate::error::{VortekError, VortekResult};
use gfx_hal::{
    adapter::{Adapter, Gpu, PhysicalDevice},
    queue::{QueueFamily, QueueGroup},
    window::Surface,
    Backend, Features,
};
use log::debug;

/// Structure for managing device state.
pub struct DeviceState<B: Backend> {
    device: B::Device,
    physical_device: B::PhysicalDevice,
    queue_family: B::QueueFamily,
    queue_group: QueueGroup<B>,
}

impl<B: Backend> DeviceState<B> {
    /// Creates a new device state from the given adapter.
    pub fn new(adapter: Adapter<B>, surface: &B::Surface) -> VortekResult<Self> {
        let Adapter {
            info,
            physical_device,
            queue_families,
        } = adapter;
        debug!("Adapter: {:?}", info);

        let queue_family = Self::take_queue_family(queue_families, surface)?;

        let Gpu {
            device,
            queue_groups,
        } = unsafe { Self::create_logical_device(&physical_device, &queue_family)? };

        let queue_group = Self::take_queue_group(queue_groups, &queue_family)?;

        Ok(Self {
            device,
            physical_device,
            queue_family,
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

    /// Returns a reference to the queue family held by the device state.
    pub fn queue_family(&self) -> &B::QueueFamily {
        &self.queue_family
    }

    /// Returns a reference to the queue group held by the device state.
    pub fn queue_group(&self) -> &QueueGroup<B> {
        &self.queue_group
    }

    /// Returns a mutable reference to the queue group held by the device state.
    pub fn queue_group_mut(&mut self) -> &mut QueueGroup<B> {
        &mut self.queue_group
    }

    /// Takes and returns the first available queue family that supports graphics
    /// and is supported by the surface.
    fn take_queue_family(
        queue_families: Vec<<B as Backend>::QueueFamily>,
        surface: &B::Surface,
    ) -> VortekResult<<B as Backend>::QueueFamily> {
        queue_families
            .into_iter()
            .find(|family| {
                family.queue_type().supports_graphics() && surface.supports_queue_family(family)
            })
            .ok_or_else(|| {
                VortekError::RenderingError(RenderingError::from_str(
                    "Could not find supported queue family with graphics.",
                ))
            })
    }

    /// Creates a new logical device from the given physical device and queue
    /// family, with only core features supported.
    ///
    /// # Safety
    /// The physical device and queue family must be compatible.
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

    /// Takes and returns the first available queue group of the given family
    /// from the given list of queue groups associated with a logical device.
    fn take_queue_group(
        queue_groups: Vec<QueueGroup<B>>,
        queue_family: &<B as Backend>::QueueFamily,
    ) -> VortekResult<QueueGroup<B>> {
        let queue_group = queue_groups
            .into_iter()
            .find(|queue_group| queue_group.family == queue_family.id())
            .ok_or_else(|| {
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
