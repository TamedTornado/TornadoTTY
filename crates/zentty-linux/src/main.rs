#![forbid(unsafe_code)]

mod application_shell;

use application_shell::ApplicationShell;
use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;
use zentty_ghostty::{AsyncBackend, GhosttyRuntime};

#[derive(Debug)]
struct Options {
    command: Option<String>,
    quit_after_last_terminal_exit: bool,
    terminal_count: usize,
    lifecycle_cycles: usize,
    async_backend: AsyncBackend,
    exercise_workspace_actions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            command: None,
            quit_after_last_terminal_exit: false,
            terminal_count: 1,
            lifecycle_cycles: 1,
            async_backend: AsyncBackend::Default,
            exercise_workspace_actions: false,
        }
    }
}

fn parse_positive_count(name: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0 && *count <= 16)
        .ok_or_else(|| format!("{name} must be an integer from 1 through 16"))
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
            "--exercise-workspace-actions" => {
                options.exercise_workspace_actions = true;
            }
            "--terminal-count" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--terminal-count requires a value".to_owned())?;
                options.terminal_count = parse_positive_count("--terminal-count", &value)?;
            }
            "--lifecycle-cycles" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--lifecycle-cycles requires a value".to_owned())?;
                options.lifecycle_cycles = parse_positive_count("--lifecycle-cycles", &value)?;
            }
            "--async-backend" => {
                options.async_backend = match arguments
                    .next()
                    .ok_or_else(|| "--async-backend requires a value".to_owned())?
                    .as_str()
                {
                    "default" => AsyncBackend::Default,
                    "epoll" => AsyncBackend::Epoll,
                    "io_uring" => AsyncBackend::IoUring,
                    value => {
                        return Err(format!(
                            "--async-backend must be default, epoll, or io_uring; got {value}"
                        ));
                    }
                };
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(options)
}

fn run_lifecycle_cycle(
    runtime: &GhosttyRuntime,
    options: &Options,
    cycle: usize,
) -> Result<(), String> {
    let main_loop = glib::MainLoop::new(None, false);
    let shell = ApplicationShell::new(
        runtime,
        options.command.clone(),
        options.terminal_count,
        options.quit_after_last_terminal_exit,
        &main_loop,
    )?;
    let close_loop = main_loop.clone();
    shell.borrow().window().connect_close_request(move |_| {
        close_loop.quit();
        glib::Propagation::Proceed
    });

    let ticking_runtime = runtime.clone();
    let tick_loop = main_loop.clone();
    let observed_window = shell.borrow().window().clone();
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

    shell.borrow().present();
    if options.exercise_workspace_actions {
        ApplicationShell::schedule_workspace_actions(&shell);
    }
    main_loop.run();

    tick_source.remove();
    shell.borrow_mut().detach_and_close();
    while glib::MainContext::default().pending() {
        glib::MainContext::default().iteration(false);
    }
    if options.quit_after_last_terminal_exit && shell.borrow().live_children() != 0 {
        return Err(format!(
            "lifecycle cycle {cycle} ended with {} live children",
            shell.borrow().live_children()
        ));
    }
    eprintln!("zentty-linux: lifecycle-cycle={cycle} complete");
    Ok(())
}

fn run() -> Result<(), String> {
    let options = parse_options()?;

    // Ghostty owns process-global initialization that must precede GTK.
    let runtime = GhosttyRuntime::new(options.async_backend).map_err(|error| error.to_string())?;
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    for cycle in 1..=options.lifecycle_cycles {
        run_lifecycle_cycle(&runtime, &options, cycle)?;
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

#[cfg(test)]
mod tests {
    use super::parse_positive_count;

    #[test]
    fn positive_counts_enforce_product_bounds() {
        assert_eq!(parse_positive_count("count", "1"), Ok(1));
        assert_eq!(parse_positive_count("count", "16"), Ok(16));
        assert!(parse_positive_count("count", "0").is_err());
        assert!(parse_positive_count("count", "17").is_err());
        assert!(parse_positive_count("count", "not-a-number").is_err());
    }
}
