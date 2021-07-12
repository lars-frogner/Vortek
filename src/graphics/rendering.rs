use crate::{application::ApplicationState, error::VortekResult};
use rendy::{command::Families, core::hal::Backend, factory::Factory, graph::Graph};
use std::{borrow::Cow, fmt};

mod graph;

pub struct Renderer<B: Backend> {
    graph: Option<Graph<B, ()>>,
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

    fn from_string(message: String) -> Self {
        Self {
            message: Cow::from(message),
        }
    }

    fn from_str(message: &'static str) -> Self {
        Self {
            message: Cow::from(message),
        }
    }
}

impl<B: Backend> Renderer<B> {
    pub fn new(factory: &mut Factory<B>, families: &mut Families<B>) -> VortekResult<Self> {
        let graph = Some(graph::build_graph(factory, families)?);
        Ok(Self { graph })
    }

    pub fn render_frame(
        &mut self,
        factory: &mut Factory<B>,
        families: &mut Families<B>,
        _app_state: &ApplicationState,
    ) -> VortekResult<()> {
        factory.maintain(families);
        if let Some(ref mut graph) = self.graph {
            graph.run(factory, families, &());
        }
        Ok(())
    }

    pub fn dispose(&mut self, factory: &mut Factory<B>) {
        if self.graph.is_some() {
            self.graph.take().unwrap().dispose(factory, &());
        }
    }
}

impl fmt::Display for RenderingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
