use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    let (file, receipt) = arguments().unwrap_or_else(|error| {
        eprintln!("file-drag-source: error: {error}");
        std::process::exit(64);
    });
    let application = gtk::Application::builder()
        .application_id("dev.zentty.FileDragSource")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    application.connect_activate(move |application| {
        let receipt = Rc::new(receipt.clone());
        let label = gtk::Label::new(Some("Drag this file into Zentty"));
        label.set_size_request(220, 100);

        let motion = gtk::EventControllerMotion::new();
        let motion_receipt = Rc::clone(&receipt);
        motion.connect_enter(move |_, _, _| append_receipt(&motion_receipt, "pointer-target"));
        label.add_controller(motion);

        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::COPY);
        let uri_list = format!("{}\r\n", gtk::gio::File::for_path(&file).uri());
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_bytes(
                "text/uri-list",
                &gtk::glib::Bytes::from(uri_list.as_bytes()),
            ))
        });
        let begin_receipt = Rc::clone(&receipt);
        source.connect_drag_begin(move |_, drag| {
            append_receipt(
                &begin_receipt,
                &format!("drag-begin:{}", drag.formats().to_str()),
            );
        });
        let end_receipt = Rc::clone(&receipt);
        source.connect_drag_end(move |_, drag, _| {
            append_receipt(
                &end_receipt,
                &format!("drag-end:{:?}", drag.selected_action()),
            );
        });
        label.add_controller(source);

        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .title("Zentty File Drag Source")
            .child(&label)
            .default_width(220)
            .default_height(100)
            .build();
        window.present();
        if let Err(error) = std::fs::write(receipt.as_ref(), b"ready\n") {
            eprintln!("file-drag-source: error: could not publish readiness: {error}");
            application.quit();
        }
    });
    application.run_with_args(&["file-drag-source"])
}

fn append_receipt(path: &PathBuf, event: &str) {
    use std::io::Write as _;
    let result = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .and_then(|mut receipt| writeln!(receipt, "{event}"));
    if let Err(error) = result {
        eprintln!("file-drag-source: error: could not publish {event}: {error}");
    }
}

fn arguments() -> Result<(PathBuf, PathBuf), String> {
    let mut arguments = std::env::args_os().skip(1);
    let file = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "usage: file_drag_source FILE RECEIPT".to_owned())?;
    let receipt = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "usage: file_drag_source FILE RECEIPT".to_owned())?;
    if arguments.next().is_some() || !file.is_absolute() || !file.is_file() {
        return Err("FILE must be one existing absolute regular file".to_owned());
    }
    if !receipt.is_absolute() || receipt.exists() {
        return Err("RECEIPT must be one absent absolute path".to_owned());
    }
    Ok((file, receipt))
}
