//! Interfacing with the hardware abstraction layer.

pub mod graph;

use super::window::WindowState;
use crate::{
    color::Color,
    error::{VortekError, VortekResult},
};
use backend::{BackendState, BackendType};
use device::DeviceState;
use framebuffer::FramebufferState;
use log::{info, warn};
use render_pass::RenderPassState;
use std::{borrow::Cow, cell::RefCell, fmt, iter, ops::Drop, rc::Rc};
use swapchain::SwapchainState;

use gfx_hal::{
    command::{ClearColor, ClearValue, CommandBuffer, CommandBufferFlags, Level, SubpassContents},
    device::{Device, OomOrDeviceLost},
    image::Extent,
    pool::CommandPool,
    pso::{PipelineStage, Rect, Viewport},
    queue::{CommandQueue, Submission},
    window::Swapchain,
    Backend,
};

pub type RendererStateType = RendererState<BackendType>;

pub struct RendererState<B: Backend> {
    backend_state: BackendState<B>,
    device_state: Rc<RefCell<DeviceState<B>>>,
    swapchain_state: Option<SwapchainState<B>>,
    render_pass_state: RenderPassState<B>,
    framebuffer_state: FramebufferState<B>,
    viewport: Viewport,
    recreate_swapchain: bool,
}

#[derive(Clone, Debug)]
pub struct RenderingError {
    message: Cow<'static, str>,
}

impl RenderingError {
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

impl<B: Backend> RendererState<B> {
    /// Creates a new renderer state from the given backend state.
    pub fn new(mut backend_state: BackendState<B>) -> VortekResult<Self> {
        let device_state = Rc::new(RefCell::new(DeviceState::new(
            backend_state.adapter_state_mut().take_adapter(),
            backend_state.surface(),
        )?));

        let mut swapchain_state =
            SwapchainState::new(Rc::clone(&device_state), &mut backend_state)?;

        let render_pass_state = RenderPassState::new(Rc::clone(&device_state), &swapchain_state)?;

        let framebuffer_state = unsafe {
            FramebufferState::new(
                Rc::clone(&device_state),
                &mut swapchain_state,
                &render_pass_state,
            )?
        };

        let viewport = Self::create_viewport(swapchain_state.extent());

        Ok(Self {
            backend_state,
            device_state,
            swapchain_state: Some(swapchain_state),
            render_pass_state,
            framebuffer_state,
            viewport,
            recreate_swapchain: false,
        })
    }

    /// Returns a mutable reference to the window state held by the renderer state.
    pub fn window_state_mut(&mut self) -> &mut WindowState {
        self.backend_state.window_state_mut()
    }

    pub fn draw_clear_frame(&mut self, color: &Color) -> VortekResult<()> {
        if self.recreate_swapchain {
            self.recreate_swapchain()?;
            self.recreate_swapchain = false;
        }

        let semaphore_index = self.framebuffer_state.advance_semaphore_index();

        let swap_image_index = unsafe {
            let acquire_semaphore = self.framebuffer_state.acquire_semaphore(semaphore_index);

            match self
                .swapchain_state
                .as_mut()
                .unwrap()
                .swapchain_mut()
                .acquire_image(std::u64::MAX, Some(acquire_semaphore), None)
            {
                Ok((swap_image_index, _)) => swap_image_index,
                Err(_) => {
                    // Resizing the window will make the current swapchain obsolete,
                    // so we have to recreate it when this happens.
                    warn!("Could not acquire image.");
                    self.recreate_swapchain = true;
                    return Ok(());
                }
            }
        };

        let (
            (framebuffer, (command_pool, command_buffer_list), in_flight_fence),
            (acquire_semaphore, present_semaphore),
        ) = self
            .framebuffer_state
            .frame_data_mut(swap_image_index, semaphore_index);

        unsafe {
            self.device_state
                .borrow()
                .device()
                .wait_for_fence(in_flight_fence, std::u64::MAX)
                .map_err(|oom_or_device_lost| match oom_or_device_lost {
                    OomOrDeviceLost::OutOfMemory(out_of_memory_err) => {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not wait for in-flight fence (out of memory): {}",
                            out_of_memory_err,
                        ))
                    }
                    OomOrDeviceLost::DeviceLost(device_lost_err) => {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not wait for in-flight fence (device lost): {}",
                            device_lost_err,
                        ))
                    }
                })?;

            self.device_state
                .borrow()
                .device()
                .reset_fence(in_flight_fence)
                .map_err(|err| {
                    VortekError::RenderingError(RenderingError::from_error(
                        "Could not reset in-flight fence: ",
                        err,
                    ))
                })?;

            command_pool.reset(false);

            let mut command_buffer = command_buffer_list
                .pop()
                .unwrap_or_else(|| command_pool.allocate_one(Level::Primary));

            let clear_values = [ClearValue {
                color: ClearColor {
                    float32: color.to_slice(),
                },
            }];

            command_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
            command_buffer.begin_render_pass(
                self.render_pass_state.render_pass(),
                framebuffer,
                self.viewport.rect,
                clear_values.iter(),
                SubpassContents::Inline,
            );
            command_buffer.end_render_pass();
            command_buffer.finish();

            let submission = Submission {
                command_buffers: iter::once(&command_buffer),
                wait_semaphores: iter::once((
                    &*acquire_semaphore,
                    PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                )),
                signal_semaphores: iter::once(&*present_semaphore),
            };

            self.device_state.borrow_mut().queue_group_mut().queues[0]
                .submit(submission, Some(in_flight_fence));

            command_buffer_list.push(command_buffer);

            if self
                .swapchain_state
                .as_ref()
                .unwrap()
                .swapchain()
                .present(
                    &mut self.device_state.borrow_mut().queue_group_mut().queues[0],
                    swap_image_index,
                    iter::once(&*present_semaphore),
                )
                .is_err()
            {
                // Resizing the window will make the current swapchain obsolete,
                // so we have to recreate it when this happens.
                warn!("Could not present image.");
                self.recreate_swapchain = true;
                return Ok(());
            }
        }
        Ok(())
    }

    // pub fn draw_triangle_frame(&mut self, triangle_coords: [f32; 6]) -> VortekResult<()> {
    //     if self.recreate_swapchain {
    //         self.recreate_swapchain()?;
    //         self.recreate_swapchain = false;
    //     }

    //     unsafe {
    //         let mut data_target = self
    //             .device_state
    //             .borrow()
    //             .device()
    //             .acquire_mapping_writer(&self.memory, 0..self.requirements.size)
    //             .map_err(|err| {
    //                 VortekError::RenderingError(RenderingError::from_error(
    //                     "Could not acquire mapping writer: ",
    //                     err,
    //                 ))
    //             })?;
    //         data_target[..6].copy_from_slice(&triangle_coords);
    //         self.device_state
    //             .borrow()
    //             .device()
    //             .release_mapping_writer(data_target)
    //             .map_err(|err| {
    //                 VortekError::RenderingError(RenderingError::from_error(
    //                     "Could not release mapping writer: ",
    //                     err,
    //                 ))
    //             })?;
    //     }

    //     let semaphore_index = self.framebuffer_state.advance_semaphore_index();

    //     let swap_image_index = unsafe {
    //         let acquire_semaphore = self.framebuffer_state.acquire_semaphore(semaphore_index);

    //         match self
    //             .swapchain_state
    //             .as_mut()
    //             .unwrap()
    //             .swapchain_mut()
    //             .acquire_image(std::u64::MAX, Some(acquire_semaphore), None)
    //         {
    //             Ok((swap_image_index, _)) => swap_image_index,
    //             Err(_) => {
    //                 // Resizing the window will make the current swapchain obsolete,
    //                 // so we have to recreate it when this happens.
    //                 warn!("Could not acquire image.");
    //                 self.recreate_swapchain = true;
    //                 return Ok(());
    //             }
    //         }
    //     };

    //     let (
    //         (framebuffer, (command_pool, command_buffer_list), in_flight_fence),
    //         (acquire_semaphore, present_semaphore),
    //     ) = self
    //         .framebuffer_state
    //         .frame_data_mut(swap_image_index, semaphore_index);

    //     unsafe {
    //         self.device_state
    //             .borrow()
    //             .device()
    //             .wait_for_fence(in_flight_fence, std::u64::MAX)
    //             .map_err(|err| {
    //                 VortekError::RenderingError(RenderingError::from_error(
    //                     "Could not wait for in-flight fence: ",
    //                     err,
    //                 ))
    //             })?;

    //         self.device_state
    //             .borrow()
    //             .device()
    //             .reset_fence(in_flight_fence)
    //             .map_err(|err| {
    //                 VortekError::RenderingError(RenderingError::from_error(
    //                     "Could not reset in-flight fence: ",
    //                     err,
    //                 ))
    //             })?;

    //         command_pool.reset(false);

    //         let mut command_buffer = command_buffer_list
    //             .pop()
    //             .unwrap_or_else(|| command_pool.acquire_command_buffer());

    //         const TRIANGLE_CLEAR_VALUES: [ClearValue; 1] =
    //             [ClearValue::Color(ClearColor::Sfloat([0.1, 0.2, 0.3, 1.0]))];

    //         command_buffer.begin();
    //         {
    //             let mut encoder = command_buffer.begin_render_pass_inline(
    //                 self.render_pass_state.render_pass(),
    //                 framebuffer,
    //                 self.viewport.rect,
    //                 TRIANGLE_CLEAR_VALUES.iter(),
    //             );
    //             encoder.bind_graphics_pipeline(&self.graphics_pipeline);
    //             encoder.bind_vertex_buffers(0, iter::once((&self.buffer, 0)));
    //             encoder.draw(0..3, 0..1);
    //         }
    //         command_buffer.finish();

    //         let submission = Submission {
    //             command_buffers: iter::once(&command_buffer),
    //             wait_semaphores: iter::once((
    //                 &*acquire_semaphore,
    //                 PipelineStage::COLOR_ATTACHMENT_OUTPUT,
    //             )),
    //             signal_semaphores: iter::once(&*present_semaphore),
    //         };

    //         self.device_state.borrow_mut().queue_group_mut().queues[0]
    //             .submit(submission, Some(in_flight_fence));

    //         command_buffer_list.push(command_buffer);

    //         if self
    //             .swapchain_state
    //             .as_ref()
    //             .unwrap()
    //             .swapchain()
    //             .present(
    //                 &mut self.device_state.borrow_mut().queue_group_mut().queues[0],
    //                 swap_image_index,
    //                 iter::once(&*present_semaphore),
    //             )
    //             .is_err()
    //         {
    //             // Resizing the window will make the current swapchain obsolete,
    //             // so we have to recreate it when this happens.
    //             warn!("Could not present image.");
    //             self.recreate_swapchain = true;
    //             return Ok(());
    //         }
    //     }
    //     Ok(())
    // }

    fn recreate_swapchain(&mut self) -> VortekResult<()> {
        info!("Recreating swapchain.");

        self.device_state
            .borrow()
            .device()
            .wait_idle()
            .map_err(|err| {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not wait for device to become idle: ",
                    err,
                ))
            })?;

        // Drop existing swapchain
        self.swapchain_state
            .take()
            .expect("No swapchain state in renderer state.");

        self.swapchain_state = Some(SwapchainState::new(
            Rc::clone(&self.device_state),
            &mut self.backend_state,
        )?);

        self.render_pass_state = RenderPassState::new(
            Rc::clone(&self.device_state),
            self.swapchain_state.as_ref().unwrap(),
        )?;

        self.framebuffer_state = unsafe {
            FramebufferState::new(
                Rc::clone(&self.device_state),
                self.swapchain_state.as_mut().unwrap(),
                &self.render_pass_state,
            )?
        };

        self.viewport = Self::create_viewport(self.swapchain_state.as_ref().unwrap().extent());

        Ok(())
    }

    fn create_viewport(extent: &Extent) -> Viewport {
        Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: extent.width as i16,
                h: extent.height as i16,
            },
            depth: 0.0..1.0,
        }
    }
}

impl<B: Backend> Drop for RendererState<B> {
    fn drop(&mut self) {
        self.swapchain_state.take();
    }
}

impl fmt::Display for RenderingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
