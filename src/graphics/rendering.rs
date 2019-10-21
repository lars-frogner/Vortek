//! Interfacing with the hardware abstraction layer.

pub mod adapter;
pub mod backend;
pub mod device;
pub mod framebuffer;
pub mod render_pass;
pub mod swapchain;

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
    command::{ClearColor, ClearValue},
    device::Device,
    image::Extent,
    pso::{PipelineStage, Rect, Viewport},
    queue::Submission,
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
    pub unsafe fn new(mut backend_state: BackendState<B>) -> VortekResult<Self> {
        let device_state = Rc::new(RefCell::new(DeviceState::new(
            backend_state.adapter_state_mut().take_adapter(),
            backend_state.surface(),
        )?));

        let mut swapchain_state =
            SwapchainState::new(Rc::clone(&device_state), &mut backend_state)?;

        let render_pass_state = RenderPassState::new(Rc::clone(&device_state), &swapchain_state)?;

        let framebuffer_state = FramebufferState::new(
            Rc::clone(&device_state),
            &mut swapchain_state,
            &render_pass_state,
        )?;

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
                .map_err(|err| {
                    VortekError::RenderingError(RenderingError::from_error(
                        "Could not wait for in-flight fence: ",
                        err,
                    ))
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
                .unwrap_or_else(|| command_pool.acquire_command_buffer());

            let clear_values = [ClearValue::Color(ClearColor::Sfloat(color.to_slice()))];

            command_buffer.begin();
            command_buffer.begin_render_pass_inline(
                self.render_pass_state.render_pass(),
                framebuffer,
                self.viewport.rect,
                clear_values.iter(),
            );
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
                warn!("Could not present image.");
                self.recreate_swapchain = true;
                return Ok(());
            }
        }
        Ok(())
    }

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

        self.swapchain_state = Some(unsafe {
            SwapchainState::new(Rc::clone(&self.device_state), &mut self.backend_state)?
        });

        self.render_pass_state = unsafe {
            RenderPassState::new(
                Rc::clone(&self.device_state),
                self.swapchain_state.as_ref().unwrap(),
            )?
        };

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

//         let instance = backend::Instance::create(window_state.window_title(), 1);
//         let mut surface = instance.create_surface(window_state.window());

//         let adapter = instance
//             .enumerate_adapters()
//             .into_iter()
//             .find(|adapter| {
//                 adapter.queue_families.iter().any(|queue_family| {
//                     queue_family.supports_graphics() && surface.supports_queue_family(queue_family)
//                 })
//             })
//             .ok_or(VortekError::RenderingError(RenderingError::from_str(
//                 "Could not find a supported graphical adapter.",
//             )))?;

//         let (device, queue_group) = {
//             let queue_family = adapter
//                 .queue_families
//                 .iter()
//                 .find(|queue_family| {
//                     queue_family.supports_graphics() && surface.supports_queue_family(queue_family)
//                 })
//                 .ok_or(VortekError::RenderingError(RenderingError::from_str(
//                     "Could not find a queue family with graphics.",
//                 )))?;

//             let Gpu { device, mut queues } = unsafe {
//                 adapter
//                     .physical_device
//                     .open(&[(&queue_family, &[1.0; 1])], Features::empty())
//                     .map_err(|err| {
//                         VortekError::RenderingError(RenderingError::from_error(
//                             "Could not open physical device: ",
//                             err,
//                         ))
//                     })?
//             };

//             let queue_group =
//                 queues
//                     .take::<Graphics>(queue_family.id())
//                     .ok_or(VortekError::RenderingError(RenderingError::from_str(
//                         "Could not take ownership of queue group.",
//                     )))?;

//             if queue_group.queues.len() == 0 {
//                 Err(VortekError::RenderingError(RenderingError::from_str(
//                     "Queue group did not have any command queues available.",
//                 )))?;
//             }

//             (device, queue_group)
//         };

//         let (swapchain, extent, backbuffer, format, number_of_frames_in_flight) = {
//             let (capabilities, preferred_formats, present_modes, composite_alphas) =
//                 surface.compatibility(&adapter.physical_device);
//             info!("{:?}", capabilities);
//             info!("Preferred formats: {:?}", preferred_formats);
//             info!("Present modes: {:?}", present_modes);
//             info!("Composite alphas: {:?}", composite_alphas);

//             let present_mode = [
//                 PresentMode::Mailbox,
//                 PresentMode::Fifo,
//                 PresentMode::Relaxed,
//                 PresentMode::Immediate,
//             ]
//             .iter()
//             .cloned()
//             .find(|present_mode| present_modes.contains(present_mode))
//             .ok_or(VortekError::RenderingError(RenderingError::from_str(
//                 "No present modes specified.",
//             )))?;

//             let composite_alpha = [
//                 CompositeAlpha::OPAQUE,
//                 CompositeAlpha::INHERIT,
//                 CompositeAlpha::PREMULTIPLIED,
//                 CompositeAlpha::POSTMULTIPLIED,
//             ]
//             .iter()
//             .cloned()
//             .find(|composite_alpha| composite_alphas.contains(composite_alpha))
//             .ok_or(VortekError::RenderingError(RenderingError::from_str(
//                 "No composite alpha modes specified.",
//             )))?;

//             let format =
//                 match preferred_formats {
//                     None => Format::Rgba8Srgb,
//                     Some(formats) => match formats
//                         .iter()
//                         .find(|format| format.base_format().1 == ChannelType::Srgb)
//                         .cloned()
//                     {
//                         Some(srgb_format) => srgb_format,
//                         None => formats.get(0).cloned().ok_or(VortekError::RenderingError(
//                             RenderingError::from_str("Preferred format list was empty."),
//                         ))?,
//                     },
//                 };

//             let extent = capabilities.extents.end;

//             let image_count =
//                 (capabilities.image_count.end - 1).min(capabilities.image_count.start.max(
//                     if present_mode == PresentMode::Mailbox {
//                         3
//                     } else {
//                         2
//                     },
//                 ));

//             let image_layers = 1;

//             let image_usage = if capabilities.usage.contains(Usage::COLOR_ATTACHMENT) {
//                 Usage::COLOR_ATTACHMENT
//             } else {
//                 Err(VortekError::RenderingError(RenderingError::from_str(
//                     "Surface does not support color.",
//                 )))?
//             };

//             let swapchain_config = SwapchainConfig {
//                 present_mode,
//                 composite_alpha,
//                 format,
//                 extent,
//                 image_count,
//                 image_layers,
//                 image_usage,
//             };
//             info!("{:?}", swapchain_config);

//             let (swapchain, backbuffer) = unsafe {
//                 device
//                     .create_swapchain(&mut surface, swapchain_config, None)
//                     .map_err(|err| {
//                         VortekError::RenderingError(RenderingError::from_error(
//                             "Could not create swapchain: ",
//                             err,
//                         ))
//                     })?
//             };
//             (swapchain, extent, backbuffer, format, image_count as usize)
//         };

//         let (image_available_semaphores, rendering_finished_semaphores, in_flight_fences) = {
//             let mut image_available_semaphores: Vec<<backend::Backend as Backend>::Semaphore> =
//                 Vec::new();
//             let mut rendering_finished_semaphores: Vec<<backend::Backend as Backend>::Semaphore> =
//                 Vec::new();
//             let mut in_flight_fences: Vec<<backend::Backend as Backend>::Fence> = Vec::new();

//             for _ in 0..number_of_frames_in_flight {
//                 image_available_semaphores.push(device.create_semaphore().map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error("Could not create semaphore: ", err))
//                 })?);
//                 rendering_finished_semaphores.push(device.create_semaphore().map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error("Could not create semaphore: ", err))
//                 })?);
//                 in_flight_fences.push(device.create_fence(true).map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error("Could not create fence: ", err))
//                 })?);
//             }
//             (
//                 image_available_semaphores,
//                 rendering_finished_semaphores,
//                 in_flight_fences,
//             )
//         };

//         let render_pass = {
//             let color_attachment = Attachment {
//                 format: Some(format),
//                 samples: 1,
//                 ops: AttachmentOps {
//                     load: AttachmentLoadOp::Clear,
//                     store: AttachmentStoreOp::Store,
//                 },
//                 stencil_ops: AttachmentOps::DONT_CARE,
//                 layouts: Layout::Undefined..Layout::Present,
//             };

//             let subpass = SubpassDesc {
//                 colors: &[(0, Layout::ColorAttachmentOptimal)],
//                 depth_stencil: None,
//                 inputs: &[],
//                 resolves: &[],
//                 preserves: &[],
//             };

//             unsafe {
//                 device
//                     .create_render_pass(&[color_attachment], &[subpass], &[])
//                     .map_err(|err| {
//                         VortekError::RenderingError(RenderingError::from_error(
//                             "Could not create render pass: ",
//                             err,
//                         ))
//                     })?
//             }
//         };

//         let image_views: Vec<_> = backbuffer
//             .into_iter()
//             .map(|image| unsafe {
//                 device
//                     .create_image_view(
//                         &image,
//                         ViewKind::D2,
//                         format,
//                         Swizzle::NO,
//                         SubresourceRange {
//                             aspects: Aspects::COLOR,
//                             levels: 0..1,
//                             layers: 0..1,
//                         },
//                     )
//                     .map_err(|err| {
//                         VortekError::RenderingError(RenderingError::from_error(
//                             "Could not create image view: ",
//                             err,
//                         ))
//                     })
//             })
//             .collect::<Result<Vec<_>, VortekError>>()?;

//         let framebuffers: Vec<<backend::Backend as Backend>::Framebuffer> = image_views
//             .iter()
//             .map(|image_view| unsafe {
//                 device
//                     .create_framebuffer(
//                         &render_pass,
//                         vec![image_view],
//                         Extent {
//                             width: extent.width as u32,
//                             height: extent.height as u32,
//                             depth: 1,
//                         },
//                     )
//                     .map_err(|err| {
//                         VortekError::RenderingError(RenderingError::from_error(
//                             "Could not create framebuffer: ",
//                             err,
//                         ))
//                     })
//             })
//             .collect::<Result<Vec<_>, VortekError>>()?;

//         let mut command_pool = unsafe {
//             device
//                 .create_command_pool(
//                     queue_group.family(),
//                     CommandPoolCreateFlags::RESET_INDIVIDUAL,
//                 )
//                 .map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error(
//                         "Could not create command pool: ",
//                         err,
//                     ))
//                 })?
//         };

//         let command_buffers: Vec<_> = framebuffers
//             .iter()
//             .map(|_| command_pool.acquire_command_buffer())
//             .collect();

//         let current_frame_index = 0;

//         Ok(Self {
//             _instance: ManuallyDrop::new(instance),
//             _surface: surface,
//             _adapter: adapter,
//             device: ManuallyDrop::new(device),
//             swapchain: ManuallyDrop::new(swapchain),
//             queue_group,
//             render_area: extent.to_extent().rect(),
//             render_pass: ManuallyDrop::new(render_pass),
//             image_views,
//             framebuffers,
//             command_pool: ManuallyDrop::new(command_pool),
//             command_buffers,
//             image_available_semaphores,
//             rendering_finished_semaphores,
//             in_flight_fences,
//             number_of_frames_in_flight,
//             current_frame_index,
//         })
//     }

// pub fn draw_clear_frame(&mut self, color: &Color) -> VortekResult<()> {
//     unimplemented!()
// }
//         let image_available_semaphore = &self.image_available_semaphores[self.current_frame_index];
//         let rendering_finished_semaphore =
//             &self.rendering_finished_semaphores[self.current_frame_index];

//         self.current_frame_index = (self.current_frame_index + 1) % self.number_of_frames_in_flight;

//         let (image_index_u32, image_index_usize) = unsafe {
//             let (image_index, _) = self
//                 .swapchain
//                 .acquire_image(std::u64::MAX, Some(image_available_semaphore), None)
//                 .map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error(
//                         "Could not acquire image from swapchain: ",
//                         err,
//                     ))
//                 })?;
//             (image_index, image_index as usize)
//         };

//         let in_flight_fence = &self.in_flight_fences[image_index_usize];
//         unsafe {
//             self.device
//                 .wait_for_fence(in_flight_fence, std::u64::MAX)
//                 .map_err(|err| {
//                     VortekError::RenderingError(RenderingError::from_error("Could not wait on fence: ", err))
//                 })?;
//             self.device.reset_fence(in_flight_fence).map_err(|err| {
//                 VortekError::RenderingError(RenderingError::from_error("Could not reset fence: ", err))
//             })?;
//         }

//         unsafe {
//             let buffer = &mut self.command_buffers[image_index_usize];
//             let clear_values = [ClearValue::Color(ClearColor::Sfloat(color.to_slice()))];
//             buffer.begin(false);
//             buffer.begin_render_pass_inline(
//                 &self.render_pass,
//                 &self.framebuffers[image_index_usize],
//                 self.render_area,
//                 clear_values.iter(),
//             );
//             buffer.finish();
//         }

//         let command_buffers = &self.command_buffers[image_index_usize..=image_index_usize];
//         let wait_semaphores: ArrayVec<[_; 1]> = [(
//             image_available_semaphore,
//             PipelineStage::COLOR_ATTACHMENT_OUTPUT,
//         )]
//         .into();
//         let signal_semaphores: ArrayVec<[_; 1]> = [rendering_finished_semaphore].into();
//         let present_wait_semaphores: ArrayVec<[_; 1]> = [rendering_finished_semaphore].into();

//         let submission = Submission {
//             command_buffers,
//             wait_semaphores,
//             signal_semaphores,
//         };

//         let command_queue = &mut self.queue_group.queues[0];

//         unsafe {
//             command_queue.submit(submission, Some(in_flight_fence));
//             self.swapchain
//                 .present(&mut command_queue, image_index_u32, present_wait_semaphores)
//                 .map_err(|_| {
//                     VortekError::RenderingError(RenderingError::from_str(
//                         "Could not present into the swapchain.",
//                     ))
//                 })?;
//         }
//         Ok(())
//     }
//}

// impl ops::Drop for RendererState {
//     fn drop(&mut self) {
//         let _ = self.device.wait_idle();
//     }
// }
