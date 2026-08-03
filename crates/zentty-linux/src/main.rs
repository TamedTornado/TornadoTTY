#![forbid(unsafe_code)]

use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;
use zentty_ghostty::{AsyncBackend, GhosttyRuntime, SurfaceConfig};

#[derive(Debug, Default)]
struct Options {
    command: Option<String>,
    quit_after_last_terminal_exit: bool,
}

fn parse_options() -> Result<Options, String> {
    let mut arguments = std::env::args().skip(1);
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--command" => {
                options.command = Some(
                    arguments
                        .next()
                        .ok_or_else(|| "--command requires a value".to_owned())?,
                );
            }
            "--quit-after-last-terminal-exit" => {
                options.quit_after_last_terminal_exit = true;
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(options)
}

fn run() -> Result<(), String> {
    let options = parse_options()?;

    // Ghostty owns process-global initialization that must precede GTK.
    let runtime = GhosttyRuntime::new(AsyncBackend::Default).map_err(|error| error.to_string())?;
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    let main_loop = glib::MainLoop::new(None, false);
    let window = gtk::Window::new();
    window.set_title(Some(zentty_core::PRODUCT_NAME));
    window.set_default_size(1000, 700);

    let surface = runtime
        .create_surface(&SurfaceConfig {
            command: options.command,
            title: zentty_core::PRODUCT_NAME.to_owned(),
        })
        .map_err(|error| error.to_string())?;

    surface.on_initialized(|| eprintln!("zentty-linux: terminal-ready"));
    surface.on_title_changed(|title| eprintln!("zentty-linux: title={title}"));
    let child_loop = main_loop.clone();
    surface.on_child_exited(move || {
        eprintln!("zentty-linux: child-exited");
        if options.quit_after_last_terminal_exit {
            child_loop.quit();
        }
    });

    window.set_child(Some(surface.widget()));
    let close_loop = main_loop.clone();
    window.connect_close_request(move |_| {
        close_loop.quit();
        glib::Propagation::Proceed
    });

    let ticking_runtime = runtime.clone();
    let tick_loop = main_loop.clone();
    let observed_window = window.clone();
    let last_window_size = Rc::new(Cell::new((0, 0)));
    let ticking_window_size = Rc::clone(&last_window_size);
    let tick_source = glib::timeout_add_local(Duration::from_millis(10), move || {
        let window_size = (observed_window.width(), observed_window.height());
        if window_size != ticking_window_size.get() && window_size.0 > 0 && window_size.1 > 0 {
            eprintln!(
                "zentty-linux: window-size={}x{}",
                window_size.0, window_size.1
            );
            ticking_window_size.set(window_size);
        }
        if let Err(error) = ticking_runtime.tick() {
            eprintln!("zentty-linux: {error}");
            tick_loop.quit();
            // Keep ownership of the source until the composition root removes
            // it after `MainLoop::run`; this preserves one teardown path for
            // both ordinary exit and tick failure.
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Continue
        }
    });

    window.present();
    surface.grab_focus();
    main_loop.run();

    tick_source.remove();
    window.set_child(gtk::Widget::NONE);
    window.close();
    drop(surface);
    drop(window);
    while glib::MainContext::default().pending() {
        glib::MainContext::default().iteration(false);
    }
    drop(runtime);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zentty-linux: {error}");
            ExitCode::FAILURE
        }
    }
}
