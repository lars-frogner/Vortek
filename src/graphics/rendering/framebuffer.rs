//! Framebuffer management.

use super::{
    device::DeviceState, render_pass::RenderPassState, swapchain::SwapchainState, RenderingError,
};
use crate::error::{VortekError, VortekResult};
use gfx_hal::{
    device::{Device, OomOrDeviceLost},
    format::{Aspects, Format, Swizzle},
    image::{Extent, SubresourceRange, ViewKind},
    pool::{CommandPool, CommandPoolCreateFlags},
    queue::{QueueFamily, QueueFamilyId},
    window::SwapImageIndex,
    Backend,
};
use std::{cell::RefCell, ops::Drop, rc::Rc};

/// Structure for managing framebuffer state.
pub struct FramebufferState<B: Backend> {
    framebuffers: Option<Vec<B::Framebuffer>>,
    frame_images: Option<Vec<(B::Image, B::ImageView)>>,
    command_pools: Option<Vec<B::CommandPool>>,
    command_buffer_lists: Vec<Vec<B::CommandBuffer>>,
    in_flight_fences: Option<Vec<B::Fence>>,
    acquire_semaphores: Option<Vec<B::Semaphore>>,
    present_semaphores: Option<Vec<B::Semaphore>>,
    number_of_frames: usize,
    next_semaphore_index: usize,
    device_state: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> FramebufferState<B> {
    /// Creates a new framebuffer state from the given device, render pass and swapchain states.
    ///
    /// # Safety
    /// A potential source of unsafety is the creation of image views
    /// with an incompatible device and swapchain state, but the safety
    /// requirements of `Device::create_image_view` are not documented.
    pub unsafe fn new(
        device_state: Rc<RefCell<DeviceState<B>>>,
        swapchain_state: &mut SwapchainState<B>,
        render_pass_state: &RenderPassState<B>,
    ) -> VortekResult<Self> {
        let images = swapchain_state.take_backbuffer();
        let number_of_frames = images.len();

        let image_views = Self::create_image_views(
            device_state.borrow().device(),
            swapchain_state.format(),
            &images,
        )?;

        let framebuffers = Self::create_framebuffers(
            device_state.borrow().device(),
            render_pass_state.render_pass(),
            swapchain_state.extent(),
            &image_views,
        )?;

        let in_flight_fences =
            Self::create_fences(device_state.borrow().device(), number_of_frames)?;
        let acquire_semaphores =
            Self::create_semaphores(device_state.borrow().device(), number_of_frames)?;
        let present_semaphores =
            Self::create_semaphores(device_state.borrow().device(), number_of_frames)?;

        let (command_pools, command_buffer_lists) = Self::create_command_pools_and_buffers(
            device_state.borrow().device(),
            device_state.borrow().queue_family().id(),
            number_of_frames,
        )?;

        Ok(FramebufferState {
            framebuffers: Some(framebuffers),
            frame_images: Some(images.into_iter().zip(image_views.into_iter()).collect()),
            command_pools: Some(command_pools),
            command_buffer_lists,
            in_flight_fences: Some(in_flight_fences),
            acquire_semaphores: Some(acquire_semaphores),
            present_semaphores: Some(present_semaphores),
            number_of_frames,
            next_semaphore_index: 0,
            device_state,
        })
    }

    /// Returns mutable references to the framebuffer, command pool, command buffers,
    /// fence, acquire semaphore and present semaphore for the given swap chain and
    /// semaphore indices.
    #[allow(clippy::type_complexity)]
    pub fn frame_data_mut(
        &mut self,
        swap_image_index: SwapImageIndex,
        semaphore_index: usize,
    ) -> (
        (
            &mut B::Framebuffer,
            (&mut B::CommandPool, &mut Vec<B::CommandBuffer>),
            &mut B::Fence,
        ),
        (&mut B::Semaphore, &mut B::Semaphore),
    ) {
        let swap_image_index = swap_image_index as usize;
        (
            (
                &mut self
                    .framebuffers
                    .as_mut()
                    .expect("No framebuffers in framebuffer state.")[swap_image_index],
                (
                    &mut self
                        .command_pools
                        .as_mut()
                        .expect("No command pools in framebuffer state.")[swap_image_index],
                    &mut self.command_buffer_lists[swap_image_index],
                ),
                &mut self
                    .in_flight_fences
                    .as_mut()
                    .expect("No in-flight fences in framebuffer state.")[swap_image_index],
            ),
            (
                &mut self
                    .acquire_semaphores
                    .as_mut()
                    .expect("No acquire semaphores in framebuffer state.")[semaphore_index],
                &mut self
                    .present_semaphores
                    .as_mut()
                    .expect("No present semaphores in framebuffer state.")[semaphore_index],
            ),
        )
    }

    /// Returns a reference to the framebuffer for the given swap image index.
    pub fn framebuffer(&self, swap_image_index: SwapImageIndex) -> &B::Framebuffer {
        &self
            .framebuffers
            .as_ref()
            .expect("No framebuffers in framebuffer state.")[swap_image_index as usize]
    }

    /// Returns a mutable reference to the framebuffer for the given swap image index.
    pub fn framebuffer_mut(&mut self, swap_image_index: SwapImageIndex) -> &mut B::Framebuffer {
        &mut self
            .framebuffers
            .as_mut()
            .expect("No framebuffers in framebuffer state.")[swap_image_index as usize]
    }

    /// Returns references to the command pool and buffers for the given swap image index.
    #[allow(clippy::type_complexity)]
    pub fn command_buffer_data(
        &self,
        swap_image_index: SwapImageIndex,
    ) -> (&B::CommandPool, &[B::CommandBuffer]) {
        (
            &self
                .command_pools
                .as_ref()
                .expect("No command pools in framebuffer state.")[swap_image_index as usize],
            &self.command_buffer_lists[swap_image_index as usize],
        )
    }

    /// Returns mutable references to the command pool and buffers for the given swap image index.
    #[allow(clippy::type_complexity)]
    pub fn command_buffer_data_mut(
        &mut self,
        swap_image_index: SwapImageIndex,
    ) -> (&mut B::CommandPool, &mut Vec<B::CommandBuffer>) {
        (
            &mut self
                .command_pools
                .as_mut()
                .expect("No command pools in framebuffer state.")[swap_image_index as usize],
            &mut self.command_buffer_lists[swap_image_index as usize],
        )
    }

    /// Returns a reference to the in-flight fence for the given swap image index.
    pub fn in_flight_fence(&self, swap_image_index: SwapImageIndex) -> &B::Fence {
        &self
            .in_flight_fences
            .as_ref()
            .expect("No in-flight fences in framebuffer state.")[swap_image_index as usize]
    }

    /// Returns a mutable reference to the in-flight fence for the given swap image index.
    pub fn in_flight_fence_mut(&mut self, swap_image_index: SwapImageIndex) -> &mut B::Fence {
        &mut self
            .in_flight_fences
            .as_mut()
            .expect("No in-flight fences in framebuffer state.")[swap_image_index as usize]
    }

    /// Returns a reference to the acquire semaphore for the given semaphore index.
    pub fn acquire_semaphore(&self, semaphore_index: usize) -> &B::Semaphore {
        &self
            .acquire_semaphores
            .as_ref()
            .expect("No acquire semaphores in framebuffer state.")[semaphore_index]
    }

    /// Returns a mutable reference to the acquire semaphore for the given semaphore index.
    pub fn acquire_semaphore_mut(&mut self, semaphore_index: usize) -> &mut B::Semaphore {
        &mut self
            .acquire_semaphores
            .as_mut()
            .expect("No acquire semaphores in framebuffer state.")[semaphore_index]
    }

    /// Returns a reference to the present semaphore for the given semaphore index.
    pub fn present_semaphore(&self, semaphore_index: usize) -> &B::Semaphore {
        &self
            .present_semaphores
            .as_ref()
            .expect("No present semaphores in framebuffer state.")[semaphore_index]
    }

    /// Returns a mutable reference to the present semaphore for the given semaphore index.
    pub fn present_semaphore_mut(&mut self, semaphore_index: usize) -> &mut B::Semaphore {
        &mut self
            .present_semaphores
            .as_mut()
            .expect("No present semaphores in framebuffer state.")[semaphore_index]
    }

    /// Advances the semaphore index and returns the current index.
    pub fn advance_semaphore_index(&mut self) -> usize {
        let current_semaphore_index = self.next_semaphore_index;
        self.next_semaphore_index = (self.next_semaphore_index + 1) % self.number_of_frames;
        current_semaphore_index
    }

    /// Creates a simple color image view for each given image of the swapchain backbuffer.
    unsafe fn create_image_views(
        device: &B::Device,
        format: Format,
        images: &[B::Image],
    ) -> VortekResult<Vec<B::ImageView>> {
        let color_range = SubresourceRange {
            aspects: Aspects::COLOR,
            levels: 0..1,
            layers: 0..1,
        };
        images
            .iter()
            .map(|image| {
                device
                    .create_image_view(
                        image,
                        ViewKind::D2,
                        format,
                        Swizzle::NO,
                        color_range.clone(),
                    )
                    .map_err(|err| {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not create image view: ",
                            err,
                        ))
                    })
            })
            .collect::<VortekResult<Vec<_>>>()
    }

    /// Creates a framebuffer with the given extent and render pass from each given image view.
    fn create_framebuffers(
        device: &B::Device,
        render_pass: &B::RenderPass,
        extent: &Extent,
        image_views: &[B::ImageView],
    ) -> VortekResult<Vec<B::Framebuffer>> {
        let extent = Extent {
            width: extent.width as _,
            height: extent.height as _,
            depth: 1,
        };
        assert!(
            extent.width > 0 && extent.height > 0,
            "Image extent is zero."
        );

        image_views
            .iter()
            .map(|image_view| unsafe {
                device
                    .create_framebuffer(render_pass, Some(image_view), extent)
                    .map_err(|err| {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not create framebuffer: ",
                            err,
                        ))
                    })
            })
            .collect::<Result<Vec<_>, VortekError>>()
    }

    /// Creates the given number of new fences.
    fn create_fences(device: &B::Device, number: usize) -> VortekResult<Vec<B::Fence>> {
        let mut fences = Vec::with_capacity(number);
        for _ in 0..number {
            fences.push(device.create_fence(true).map_err(|err| {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not create fence: ",
                    err,
                ))
            })?);
        }
        Ok(fences)
    }

    /// Creates the given number of new semaphores.
    fn create_semaphores(device: &B::Device, number: usize) -> VortekResult<Vec<B::Semaphore>> {
        let mut semaphores = Vec::with_capacity(number);
        for _ in 0..number {
            semaphores.push(device.create_semaphore().map_err(|err| {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not create semaphore: ",
                    err,
                ))
            })?);
        }
        Ok(semaphores)
    }

    /// Creates the given number of command pools and empty command buffer lists
    /// for the given command queue family.
    ///
    /// # Safety
    /// The queue family has to be supported by the logical device.
    #[allow(clippy::type_complexity)]
    unsafe fn create_command_pools_and_buffers(
        device: &B::Device,
        queue_family_id: QueueFamilyId,
        number: usize,
    ) -> VortekResult<(Vec<B::CommandPool>, Vec<Vec<B::CommandBuffer>>)> {
        let mut command_pools = Vec::with_capacity(number);
        let mut command_buffer_lists = Vec::with_capacity(number);
        for _ in 0..number {
            command_pools.push(
                device
                    .create_command_pool(queue_family_id, CommandPoolCreateFlags::RESET_INDIVIDUAL)
                    .map_err(|err| {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not create command pool: ",
                            err,
                        ))
                    })?,
            );

            command_buffer_lists.push(Vec::new());
        }
        Ok((command_pools, command_buffer_lists))
    }
}

impl<B: Backend> Drop for FramebufferState<B> {
    fn drop(&mut self) {
        let borrowed_device_state = self.device_state.borrow();
        let device = borrowed_device_state.device();
        unsafe {
            for fence in self
                .in_flight_fences
                .take()
                .expect("No in-flight fences in framebuffer state.")
            {
                device
                    .wait_for_fence(&fence, std::u64::MAX)
                    .unwrap_or_else(|oom_or_device_lost| match oom_or_device_lost {
                        OomOrDeviceLost::OutOfMemory(out_of_memory_err) => panic!(
                            "Could not wait for in-flight fence (out of memory): {}",
                            out_of_memory_err
                        ),
                        OomOrDeviceLost::DeviceLost(device_lost_err) => panic!(
                            "Could not wait for in-flight fence (device lost): {}",
                            device_lost_err
                        ),
                    });
                device.destroy_fence(fence);
            }

            for (mut command_pool, command_buffer_list) in self
                .command_pools
                .take()
                .expect("No command pools in framebuffer state.")
                .into_iter()
                .zip(self.command_buffer_lists.drain(..))
            {
                command_pool.free(command_buffer_list);
                device.destroy_command_pool(command_pool);
            }

            for acquire_semaphore in self
                .acquire_semaphores
                .take()
                .expect("No acquire semaphores in framebuffer state.")
            {
                device.destroy_semaphore(acquire_semaphore);
            }

            for present_semaphore in self
                .present_semaphores
                .take()
                .expect("No present semaphores in framebuffer state.")
            {
                device.destroy_semaphore(present_semaphore);
            }

            for framebuffer in self
                .framebuffers
                .take()
                .expect("No framebuffers in framebuffer state.")
            {
                device.destroy_framebuffer(framebuffer);
            }

            for (_, image_view) in self
                .frame_images
                .take()
                .expect("No image views in framebuffer state.")
            {
                device.destroy_image_view(image_view);
            }
        }
    }
}
