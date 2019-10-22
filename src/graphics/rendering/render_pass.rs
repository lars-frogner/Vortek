//! Render pass management.

use super::{device::DeviceState, swapchain::SwapchainState, RenderingError};
use crate::error::{VortekError, VortekResult};
use gfx_hal::{
    device::Device,
    format::Format,
    image::{Access, Layout},
    pass::{
        Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDependency,
        SubpassDesc, SubpassRef,
    },
    pso::PipelineStage,
    Backend,
};
use std::{cell::RefCell, ops::Drop, rc::Rc};

/// Structure for managing render pass state.
pub struct RenderPassState<B: Backend> {
    render_pass: Option<B::RenderPass>,
    device_state: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> RenderPassState<B> {
    /// Creates a new render pass state from the given swapchain and device states.
    pub unsafe fn new(
        device_state: Rc<RefCell<DeviceState<B>>>,
        swapchain_state: &SwapchainState<B>,
    ) -> VortekResult<Self> {
        let render_pass = {
            let attachement = Self::create_attachement(swapchain_state.format());
            let subpass_description = Self::create_subpass_description();
            let subpass_dependency = Self::create_subpass_dependency();

            device_state
                .borrow()
                .device()
                .create_render_pass(
                    &[attachement],
                    &[subpass_description],
                    &[subpass_dependency],
                )
                .map_err(|err| {
                    VortekError::RenderingError(RenderingError::from_error(
                        "Could not create render pass: ",
                        err,
                    ))
                })?
        };

        Ok(Self {
            render_pass: Some(render_pass),
            device_state,
        })
    }

    /// Returns a reference to the render pass held by the render pass state.
    pub fn render_pass(&self) -> &B::RenderPass {
        self.render_pass
            .as_ref()
            .expect("No render pass in render pass state.")
    }

    /// Creates a simple image attachement description for the given format,
    /// which clears the attachement at the beginning of the subpass and
    /// preserves the data written to the attachement during the subpass.
    fn create_attachement(format: Format) -> Attachment {
        Attachment {
            format: Some(format),
            samples: 1,
            ops: AttachmentOps {
                load: AttachmentLoadOp::Clear,
                store: AttachmentStoreOp::Store,
            },
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present,
        }
    }

    /// Creates a simple subpass description which uses a color buffer with
    /// the optimal layout.
    fn create_subpass_description() -> SubpassDesc<'static> {
        SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        }
    }

    /// Creates a subpass dependency description.
    fn create_subpass_dependency() -> SubpassDependency {
        SubpassDependency {
            passes: SubpassRef::External..SubpassRef::Pass(0),
            stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            accesses: Access::empty()
                ..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE),
        }
    }
}

impl<B: Backend> Drop for RenderPassState<B> {
    fn drop(&mut self) {
        unsafe {
            self.device_state.borrow().device().destroy_render_pass(
                self.render_pass
                    .take()
                    .expect("No render pass in render pass state."),
            );
        }
    }
}
