#![forbid(unsafe_code)]

use gtk::prelude::*;
use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("zentty-focus-steal-helper: error: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), String> {
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    let event_loop = gtk::glib::MainLoop::new(None, false);
    let quit_loop = event_loop.clone();
    let window = gtk::Window::builder()
        .title("Zentty controlled IME focus helper")
        .default_width(320)
        .default_height(180)
        .build();
    window.connect_map(|_| {
        println!("zentty-focus-steal-helper: READY mapped-real-gtk-window");
        let _ = std::io::stdout().flush();
    });
    window.connect_is_active_notify(|window| {
        if window.is_active() {
            println!("zentty-focus-steal-helper: ACTIVE compositor-focus-owned");
            let _ = std::io::stdout().flush();
        }
    });
    window.connect_destroy(move |_| quit_loop.quit());
    window.present();
    event_loop.run();
    Ok(())
}
