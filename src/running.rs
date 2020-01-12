//! Running the application.

use crate::{
    application::ApplicationState,
    color::Color,
    error::VortekResult,
    graphics::{
        rendering::{backend, RendererState, RendererStateType},
        window,
    },
    input::UserInput,
};
use log::error;
use simple_logger;
use std::process;
use winit::event_loop::ControlFlow;

pub fn run() {
    simple_logger::init().unwrap_or_else(|err| {
        eprintln!("Logger initialization failed: {}", err);
        process::exit(1);
    });

    let (window_state, event_loop) = window::create_window_and_event_loop(
        window::DEFAULT_WINDOW_NAME,
        window::DEFAULT_WINDOW_SIZE,
    )
    .unwrap_or_else(|err| {
        error!("{}", err);
        process::exit(1);
    });

    let mut app_state =
        ApplicationState::new(window_state.inner_physical_size().into(), Color::black());

    let (backend_state, _instance) =
        backend::create_backend_state(window_state).unwrap_or_else(|err| {
            error!("Could not initialize backend: {}", err);
            process::exit(1);
        });
    let mut renderer_state = RendererState::new(backend_state).unwrap_or_else(|err| {
        error!("Could not initialize renderer: {}", err);
        process::exit(1);
    });

    event_loop.run(move |event, _, control_flow| {
        // Pause event loop if no events are available to process
        *control_flow = ControlFlow::Wait;

        let input = UserInput::from_event(event);

        if let UserInput::TerminationRequested = input {
            *control_flow = ControlFlow::Exit;
        } else {
            app_state.update_from_input(&input);

            if let Err(err) = render_frame(&mut renderer_state, &app_state) {
                error!("Rendering error: {}", err);
                process::exit(1);
            }
        }
    });
}

fn render_frame(
    renderer_state: &mut RendererStateType,
    app_state: &ApplicationState,
) -> VortekResult<()> {
    renderer_state.draw_clear_frame(app_state.background_color())
}
