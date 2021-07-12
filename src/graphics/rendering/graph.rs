//! Render graph creation.

use super::RenderingError;
use crate::error::{VortekError, VortekResult};
use rendy::{
    command::Families,
    core::hal::Backend,
    factory::Factory,
    graph::{Graph, GraphBuildError, GraphBuilder, NodeBuildError},
    wsi::SwapchainError,
};

pub fn build_graph<B: Backend>(
    factory: &mut Factory<B>,
    families: &mut Families<B>,
) -> VortekResult<Graph<B, ()>> {
    let graph_builder = GraphBuilder::<B, ()>::new();
    graph_builder
        .build(factory, families, &())
        .map_err(|err| match err {
            GraphBuildError::Buffer(err) => {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not create buffer during render graph construction: ",
                    err,
                ))
            }
            GraphBuildError::Image(err) => VortekError::RenderingError(RenderingError::from_error(
                "Could not create image during render graph construction: ",
                err,
            )),
            GraphBuildError::Semaphore(err) => {
                VortekError::RenderingError(RenderingError::from_error(
                    "Could not create semaphore during render graph construction: ",
                    err,
                ))
            }
            GraphBuildError::Node(err) => match err {
                NodeBuildError::Upload(err) => VortekError::RenderingError(
                    RenderingError::from_error("Could not build render graph node: ", err),
                ),
                NodeBuildError::QueueFamily(_) => {
                    VortekError::RenderingError(RenderingError::from_str(
                        "Could not build render graph node: Mismatched queue family",
                    ))
                }
                NodeBuildError::View(err) => VortekError::RenderingError(
                    RenderingError::from_error("Could not build render graph node: ", err),
                ),
                NodeBuildError::Pipeline(err) => VortekError::RenderingError(
                    RenderingError::from_error("Could not build render graph node: ", err),
                ),
                NodeBuildError::Swapchain(err) => match err {
                    SwapchainError::Create(err) => {
                        VortekError::RenderingError(RenderingError::from_error(
                            "Could not create swapchain during render graph node construction: ",
                            err,
                        ))
                    },
                    SwapchainError::BadPresentMode(present_mode) => VortekError::RenderingError(RenderingError::from_string(format!(
                            "Could not create swapchain during render graph node construction: Present mode not supported: {:?}",
                            present_mode,
                        ))),
                    SwapchainError::BadImageCount(count) => VortekError::RenderingError(RenderingError::from_string(format!(
                            "Could not create swapchain during render graph node construction: Image count not supported: {}",
                            count,
                        )))
                },
                NodeBuildError::OutOfMemory(err) => VortekError::RenderingError(
                    RenderingError::from_error("Could not build render graph node: ", err),
                ),
            },
        })
}
