//! Running the application.

use crate::{
    application::ApplicationState,
    error::VortekResult,
    graphics::{
        rendering::{backend, RendererState, RendererStateType},
        window::WindowState,
    },
    input::UserInput,
};
use log::error;
use simple_logger;
use std::process;

pub fn run() {
    simple_logger::init().unwrap_or_else(|err| {
        eprintln!("Logger initialization failed: {}", err);
        process::exit(1);
    });

    let window_state = WindowState::default();
    let (backend_state, _instance) =
        backend::create_backend_state(window_state).unwrap_or_else(|err| {
            error!("Could not initialize backend: {}", err);
            process::exit(1);
        });
    let mut renderer_state = RendererState::new(backend_state).unwrap_or_else(|err| {
        error!("Could not initialize renderer: {}", err);
        process::exit(1);
    });
    let mut app_state = ApplicationState::default();

    loop {
        let input = UserInput::poll_event_loop(renderer_state.window_state_mut().event_loop_mut());
        if let UserInput::TerminationRequested = input {
            break;
        }
        app_state.update_from_input(&input);

        if let Err(err) = render_frame(&mut renderer_state, &app_state) {
            error!("Rendering error: {}", err);
            process::exit(1);
        }
    }
}

fn render_frame(
    renderer_state: &mut RendererStateType,
    app_state: &ApplicationState,
) -> VortekResult<()> {
    renderer_state.draw_clear_frame(app_state.background_color())
}
