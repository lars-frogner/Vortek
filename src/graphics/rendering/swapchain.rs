//! Swapchain management.

use super::{
    super::window::WindowState, backend::BackendState, device::DeviceState, RenderingError,
};
use crate::error::{VortekError, VortekResult};
use gfx_hal::{
    device::Device,
    format::{ChannelType, Format},
    image::{Extent, Usage},
    window::{
        CompositeAlphaMode, Extent2D, PresentMode, Surface, SurfaceCapabilities, SwapchainConfig,
    },
    Backend,
};
use log::debug;
use std::{cell::RefCell, cmp, ops::Drop, rc::Rc};

/// Structure for managing swapchain state.
pub struct SwapchainState<B: Backend> {
    swapchain: Option<B::Swapchain>,
    backbuffer: Option<Vec<B::Image>>,
    extent: Extent,
    format: Format,
    device_state: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> SwapchainState<B> {
    /// Creates a new swapchain state from the given backend and device states.
    pub fn new(
        device_state: Rc<RefCell<DeviceState<B>>>,
        backend_state: &mut BackendState<B>,
    ) -> VortekResult<Self> {
        let capabilities = backend_state
            .surface()
            .capabilities(device_state.borrow().physical_device());
        let supported_formats = backend_state
            .surface()
            .supported_formats(device_state.borrow().physical_device());

        debug!("Surface capabilities: {:?}", capabilities);
        debug!("Supported formats: {:?}", supported_formats);

        let present_mode = Self::select_present_mode(&capabilities)?;
        let composite_alpha_mode = Self::select_composite_alpha_mode(&capabilities)?;
        let format = Self::select_format(supported_formats.as_ref())?;
        let extent = Self::determine_extent(backend_state.window_state(), &capabilities)?;
        let image_count = Self::compute_image_count(&capabilities, present_mode);
        let image_layers = 1;
        let image_usage = Self::select_image_usage(&capabilities)?;

        let swapchain_config = SwapchainConfig {
            present_mode,
            composite_alpha_mode,
            format,
            extent,
            image_count,
            image_layers,
            image_usage,
        };
        debug!("{:?}", swapchain_config);

        assert!(backend_state
            .surface()
            .supports_queue_family(device_state.borrow().queue_family()));

        let (swapchain, backbuffer) = unsafe {
            device_state
                .borrow()
                .device()
                .create_swapchain(backend_state.surface_mut(), swapchain_config, None)
                .map_err(|err| {
                    VortekError::RenderingError(RenderingError::from_error(
                        "Could not create swapchain: ",
                        err,
                    ))
                })?
        };

        Ok(Self {
            swapchain: Some(swapchain),
            backbuffer: Some(backbuffer),
            extent: extent.to_extent(),
            format,
            device_state,
        })
    }

    /// Returns a reference to the swapchain held by the swapchain state.
    pub fn swapchain(&self) -> &B::Swapchain {
        self.swapchain
            .as_ref()
            .expect("No swapchain in swapchain state.")
    }

    /// Returns a mutable reference to the swapchain held by the swapchain state.
    pub fn swapchain_mut(&mut self) -> &mut B::Swapchain {
        self.swapchain
            .as_mut()
            .expect("No swapchain in swapchain state.")
    }

    /// Returns a reference to the extent held by the swapchain state.
    pub fn extent(&self) -> &Extent {
        &self.extent
    }

    /// Returns the format held by the swapchain state.
    pub fn format(&self) -> Format {
        self.format
    }

    /// Moves the backbuffer out of the swapchain state.
    pub fn take_backbuffer(&mut self) -> Vec<B::Image> {
        self.backbuffer
            .take()
            .expect("No backbuffer in swapchain state.")
    }

    /// Selects the preferred present mode from the given surface capabilities.
    fn select_present_mode(capabilities: &SurfaceCapabilities) -> VortekResult<PresentMode> {
        [
            PresentMode::MAILBOX,
            PresentMode::FIFO,
            PresentMode::RELAXED,
            PresentMode::IMMEDIATE,
        ]
        .iter()
        .cloned()
        .find(|&present_mode| capabilities.present_modes.contains(present_mode))
        .ok_or_else(|| {
            VortekError::RenderingError(RenderingError::from_str("No present modes specified."))
        })
    }

    /// Selects the preferred composite alpha mode from the given surface capabilities.
    fn select_composite_alpha_mode(
        capabilities: &SurfaceCapabilities,
    ) -> VortekResult<CompositeAlphaMode> {
        [
            CompositeAlphaMode::OPAQUE,
            CompositeAlphaMode::INHERIT,
            CompositeAlphaMode::PREMULTIPLIED,
            CompositeAlphaMode::POSTMULTIPLIED,
        ]
        .iter()
        .cloned()
        .find(|&composite_alpha_mode| {
            capabilities
                .composite_alpha_modes
                .contains(composite_alpha_mode)
        })
        .ok_or_else(|| {
            VortekError::RenderingError(RenderingError::from_str(
                "No composite alpha modes specified.",
            ))
        })
    }

    /// Tries to select an SRGB format from the given list of supported formats,
    /// or falls back to the first format in the list.
    fn select_format(supported_formats: Option<&Vec<Format>>) -> VortekResult<Format> {
        supported_formats.map_or(Ok(Format::Rgba8Srgb), |formats| {
            match formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .cloned()
            {
                Some(srgb_format) => Ok(srgb_format),
                None => formats.get(0).cloned().ok_or_else(|| {
                    VortekError::RenderingError(RenderingError::from_str(
                        "Supported format list was empty.",
                    ))
                }),
            }
        })
    }

    /// Determines the swapchain extent to use by clamping the window extent to
    /// lie between the supported extents.
    fn determine_extent(
        window_state: &WindowState,
        capabilities: &SurfaceCapabilities,
    ) -> VortekResult<Extent2D> {
        let (window_width, window_height) = window_state.inner_physical_size().into();

        Ok(capabilities.current_extent.unwrap_or_else(|| {
            let (min_width, max_width) = (
                capabilities.extents.start().width,
                capabilities.extents.end().width,
            );
            let (min_height, max_height) = (
                capabilities.extents.start().height,
                capabilities.extents.end().height,
            );

            let width = cmp::min(max_width, cmp::max(window_width, min_width));
            let height = cmp::min(max_height, cmp::max(window_height, min_height));

            Extent2D { width, height }
        }))
    }

    /// Computes the number of images to use in the swapchain based on the present mode
    /// and supported number of images.
    fn compute_image_count(capabilities: &SurfaceCapabilities, present_mode: PresentMode) -> u32 {
        cmp::min(
            *capabilities.image_count.end(),
            cmp::max(
                *capabilities.image_count.start(),
                if present_mode == PresentMode::MAILBOX {
                    3
                } else {
                    2
                },
            ),
        )
    }

    /// Specifies that the images should be used as color attachments,
    /// or returns an error if this is not possible.
    fn select_image_usage(capabilities: &SurfaceCapabilities) -> VortekResult<Usage> {
        if capabilities.usage.contains(Usage::COLOR_ATTACHMENT) {
            Ok(Usage::COLOR_ATTACHMENT)
        } else {
            Err(VortekError::RenderingError(RenderingError::from_str(
                "Surface does not support color.",
            )))
        }
    }
}

impl<B: Backend> Drop for SwapchainState<B> {
    fn drop(&mut self) {
        unsafe {
            self.device_state.borrow().device().destroy_swapchain(
                self.swapchain
                    .take()
                    .expect("No swapchain in swapchain state."),
            );
        }
    }
}
