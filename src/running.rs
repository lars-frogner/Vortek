//! Running the application.

use crate::{
    application::ApplicationState,
    color::Color,
    graphics::{rendering::Renderer, window},
    input::UserInput,
};
use log::error;
use rendy::{
    self,
    factory::{BasicDevicesConfigure, BasicHeapsConfigure, Config, OneGraphicsQueue},
    init::{
        self,
        winit::event_loop::{ControlFlow, EventLoop},
        AnyWindowedRendy,
    },
};
use simple_logger;
use std::process;
use wgpu::{
    self, Adapter, BackendBit, DeviceDescriptor, Extensions, Limits, PowerPreference,
    RequestAdapterOptions, Surface,
};

pub fn run() {
    init_logging();
    let (windowed_rendy, event_loop) = init_graphics();

    rendy::with_any_windowed_rendy!((windowed_rendy) (mut factory, mut families, _surface, window) => {

        let mut app_state =
        ApplicationState::new(window.inner_size().into(), Color::black());

        let mut renderer = Some(Renderer::new(&mut factory, &mut families).unwrap_or_else(|err| {
            error!("{}", err);
            process::exit(1);
        }));

        event_loop.run(move |event, _, control_flow| {
            // Pause event loop if no events are available to process
            *control_flow = ControlFlow::Wait;

            let input = UserInput::from_event(event);

            if let UserInput::TerminationRequested = input {
                if renderer.is_some() {
                    renderer.take().unwrap().dispose(&mut factory);
                }
                *control_flow = ControlFlow::Exit;
            } else {
                app_state.update_from_input(&input);

                if let Some(ref mut renderer) = renderer {
                    renderer.render_frame(&mut factory, &mut families, &app_state).unwrap_or_else(|err| {
                        error!("{}", err);
                        process::exit(1);
                    });
                }
            }
        });
    });
}

fn init_logging() {
    simple_logger::init().unwrap_or_else(|err| {
        eprintln!("Logger initialization failed: {}", err);
        process::exit(1);
    });
}

fn init_graphics() -> (AnyWindowedRendy, EventLoop<()>) {
    let window_builder =
        window::create_window_builder(window::DEFAULT_WINDOW_NAME, window::DEFAULT_WINDOW_SIZE);

    let event_loop = window::create_event_loop();

    #[cfg(not(feature = "gl"))]
    let (window, surface) = {
        let window = window_builder
            .build(&event_loop)
            .unwrap_or_else(|err| error!("Could not build window: {}", err));
        let surface = Surface::create(&window);
        (window, surface)
    };

    #[cfg(feature = "gl")]
    let (instance, window, surface) = {
        context_builder = wgpu::glutin::ContextBuilder::new().with_vsync(true);
        let windowed_context = context_builder
            .build_windowed(window_builder, &event_loop)
            .unwrap_or_else(|err| error!("Could not build windowed OpenGL context: {}", err));
        let (context, window) = unsafe {
            windowed_context
                .make_current()
                .unwrap_or_else(|err| error!("Could not set OpenGL context as current: {}", err))
                .split()
        };
        let instance = wgpu::Instance::new(context);
        let surface = instance.get_surface();

        (instance, window, surface)
    };

    let adapter = Adapter::request(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        backends: BackendBit::PRIMARY,
    })
    .unwrap_or_else(|err| {
        error!("Could not find supported graphics device and/or backend");
        process::exit(1);
    });

    let (device, queue) = adapter.request_device(&DeviceDescriptor {
        extensions: Extensions {
            anisotropic_filtering: false,
        },
        limits: Limits::default(),
    });

    let config = Config {
        devices: BasicDevicesConfigure,
        heaps: BasicHeapsConfigure,
        queues: OneGraphicsQueue,
    };

    dbg!(init::available_backends());
    dbg!(init::BASIC_PRIORITY
        .iter()
        .filter_map(|b| std::convert::TryInto::try_into(*b).ok())
        .collect::<Vec<rendy::core::EnabledBackend>>());

    let windowed_rendy = AnyWindowedRendy::init_auto(&config, window_builder, &event_loop)
        .unwrap_or_else(|err| {
            error!("Rendy initialization failed: {}", err);
            process::exit(1);
        });

    (windowed_rendy, event_loop)
}
