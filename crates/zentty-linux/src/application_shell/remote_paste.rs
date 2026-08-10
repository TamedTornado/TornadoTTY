use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::gdk;
use gtk::prelude::*;
use zentty_core::{
    MAX_REMOTE_FILE_BYTES, MAX_REMOTE_IMAGE_BYTES, RemoteUploadPath, SshDestination,
    escape_remote_path_for_shell,
};
use zentty_linux::remote_transfer::{
    RemoteTransferRequest, execute_remote_transfer, rollback_remote_transfers,
};

use super::ssh_identity::{self, PaneSshIdentity};
use super::{ApplicationShell, unix_time_ms};

pub(super) struct RemoteUploadActivity {
    identity: PaneSshIdentity,
    cancellation: Arc<AtomicBool>,
}

enum UploadInput {
    File(PathBuf),
    PngImage(Vec<u8>),
}

impl RemoteUploadActivity {
    pub(super) fn identity(&self) -> &PaneSshIdentity {
        &self.identity
    }

    pub(super) fn cancellation(&self) -> &AtomicBool {
        &self.cancellation
    }

    pub(super) fn cancel(&self) {
        self.cancellation.store(true, Ordering::Release);
    }
}

pub(super) fn install(
    shell: &Rc<RefCell<ApplicationShell>>,
    pane_id: &str,
    frame: &gtk::Widget,
    terminal: &gtk::Widget,
) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(shell);
    let paste_pane_id = pane_id.to_owned();
    let paste_widget = frame.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !matches!(key, gdk::Key::v | gdk::Key::V)
            || !modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            || !modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        {
            return gtk::glib::Propagation::Proceed;
        }
        let Some(shell) = weak.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        if !shell
            .borrow()
            .remote_panes
            .identities
            .contains_key(&paste_pane_id)
        {
            return gtk::glib::Propagation::Proceed;
        }
        let clipboard = paste_widget.display().clipboard();
        let formats = clipboard.formats();
        let has_uri_list = formats.contain_mime_type("text/uri-list");
        let has_files = has_uri_list || formats.contains_type(gdk::FileList::static_type());
        let has_image = formats.contains_type(gdk::Texture::static_type());
        if !has_files && !has_image {
            return gtk::glib::Propagation::Proceed;
        }
        let weak = Rc::downgrade(&shell);
        let pane_id = paste_pane_id.clone();
        gtk::glib::spawn_future_local(async move {
            if has_files {
                let inputs = if has_uri_list {
                    read_uri_list(&clipboard).await
                } else {
                    read_file_list(&clipboard)
                        .await
                        .map(|files| file_inputs(&files))
                };
                match inputs {
                    Ok(inputs) => begin_upload(&weak, &pane_id, inputs),
                    Err(error) => {
                        eprintln!("zentty-linux: remote-paste pane={pane_id} read-failed={error}");
                    }
                }
            } else {
                match clipboard.read_texture_future().await {
                    Ok(Some(texture)) => begin_upload(
                        &weak,
                        &pane_id,
                        vec![UploadInput::PngImage(
                            texture.save_to_png_bytes().as_ref().to_vec(),
                        )],
                    ),
                    Ok(None) => eprintln!(
                        "zentty-linux: remote-paste pane={pane_id} rejected=no-image-data"
                    ),
                    Err(error) => {
                        eprintln!("zentty-linux: remote-paste pane={pane_id} read-failed={error}");
                    }
                }
            }
        });
        gtk::glib::Propagation::Stop
    });
    frame.add_controller(keys);

    install_drop_targets(shell, pane_id, terminal);
}

fn install_drop_targets(
    shell: &Rc<RefCell<ApplicationShell>>,
    pane_id: &str,
    terminal: &gtk::Widget,
) {
    let uri_target = gtk::DropTargetAsync::new(
        Some(gdk::ContentFormats::new(&["text/uri-list"])),
        gdk::DragAction::COPY,
    );
    uri_target.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(shell);
    let accept_pane_id = pane_id.to_owned();
    uri_target.connect_accept(move |_, _| {
        weak.upgrade().is_some_and(|shell| {
            shell
                .borrow()
                .remote_panes
                .identities
                .contains_key(&accept_pane_id)
        })
    });
    let enter_pane_id = pane_id.to_owned();
    uri_target.connect_drag_enter(move |_, _, _, _| {
        eprintln!("zentty-linux: remote-paste pane={enter_pane_id} drop=uri-enter");
        gdk::DragAction::COPY
    });
    let weak = Rc::downgrade(shell);
    let uri_pane_id = pane_id.to_owned();
    uri_target.connect_drop(move |_, drop, _, _| {
        let drop = drop.clone();
        let weak = weak.clone();
        let pane_id = uri_pane_id.clone();
        gtk::glib::spawn_future_local(async move {
            let inputs = read_uri_drop(&drop).await;
            let accepted = match inputs {
                Ok(inputs) if weak.upgrade().is_some() => {
                    begin_upload(&weak, &pane_id, inputs);
                    true
                }
                Ok(_) => false,
                Err(error) => {
                    eprintln!(
                        "zentty-linux: remote-paste pane={pane_id} drop=read-failed detail={error}"
                    );
                    false
                }
            };
            drop.finish(if accepted {
                gdk::DragAction::COPY
            } else {
                gdk::DragAction::empty()
            });
        });
        true
    });
    terminal.add_controller(uri_target);
}

async fn read_file_list(clipboard: &gdk::Clipboard) -> Result<gdk::FileList, String> {
    let mut first_error = None;
    for attempt in 0..2 {
        match clipboard
            .read_value_future(gdk::FileList::static_type(), gtk::glib::Priority::DEFAULT)
            .await
        {
            Ok(value) => {
                return value
                    .get::<gdk::FileList>()
                    .map_err(|error| format!("clipboard value was not a file list: {error}"));
            }
            Err(error) if attempt == 0 => {
                first_error = Some(error.to_string());
                gtk::glib::timeout_future(Duration::from_millis(75)).await;
            }
            Err(error) => {
                return Err(format!(
                    "{}; retry failed: {error}",
                    first_error.as_deref().unwrap_or("initial read failed")
                ));
            }
        }
    }
    unreachable!("clipboard read loop always returns")
}

async fn read_uri_list(clipboard: &gdk::Clipboard) -> Result<Vec<UploadInput>, String> {
    let (stream, mime_type) = clipboard
        .read_future(&["text/uri-list"], gtk::glib::Priority::DEFAULT)
        .await
        .map_err(|error| format!("could not open URI-list clipboard: {error}"))?;
    if mime_type != "text/uri-list" {
        return Err(format!(
            "clipboard returned unexpected MIME type {mime_type}"
        ));
    }
    read_uri_stream(&stream).await
}

async fn read_uri_drop(drop: &gdk::Drop) -> Result<Vec<UploadInput>, String> {
    let (stream, mime_type) = drop
        .read_future(&["text/uri-list"], gtk::glib::Priority::DEFAULT)
        .await
        .map_err(|error| format!("could not open URI-list drop: {error}"))?;
    if mime_type != "text/uri-list" {
        return Err(format!("drop returned unexpected MIME type {mime_type}"));
    }
    read_uri_stream(&stream).await
}

async fn read_uri_stream(stream: &gtk::gio::InputStream) -> Result<Vec<UploadInput>, String> {
    const MAX_URI_LIST_BYTES: usize = 64 * 1024;
    let mut payload = Vec::new();
    loop {
        let bytes = stream
            .read_bytes_future(8192, gtk::glib::Priority::DEFAULT)
            .await
            .map_err(|error| format!("could not read URI-list clipboard: {error}"))?;
        if bytes.is_empty() {
            break;
        }
        if payload.len().saturating_add(bytes.len()) > MAX_URI_LIST_BYTES {
            return Err("URI-list clipboard exceeds 64 KiB".to_owned());
        }
        payload.extend_from_slice(bytes.as_ref());
    }
    parse_uri_list(&payload).map(|paths| paths.into_iter().map(UploadInput::File).collect())
}

fn parse_uri_list(payload: &[u8]) -> Result<Vec<PathBuf>, String> {
    let payload = std::str::from_utf8(payload)
        .map_err(|_| "URI-list clipboard is not valid UTF-8".to_owned())?;
    payload
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|uri| {
            gtk::gio::File::for_uri(uri)
                .path()
                .ok_or_else(|| format!("URI-list entry is not a local file: {uri}"))
        })
        .collect()
}

fn file_inputs(files: &gdk::FileList) -> Vec<UploadInput> {
    files
        .files()
        .into_iter()
        .filter_map(|file| file.path().map(UploadInput::File))
        .collect()
}

fn begin_upload(
    weak: &std::rc::Weak<RefCell<ApplicationShell>>,
    pane_id: &str,
    inputs: Vec<UploadInput>,
) {
    let Some(shell) = weak.upgrade() else {
        return;
    };
    if inputs.is_empty() {
        eprintln!("zentty-linux: remote-paste pane={pane_id} rejected=no-local-files");
        return;
    }
    let (identity, cancellation) = {
        let mut shell = shell.borrow_mut();
        if shell.remote_panes.uploads.contains_key(pane_id) {
            eprintln!("zentty-linux: remote-paste pane={pane_id} rejected=upload-in-progress");
            return;
        }
        let Some(identity) = shell.remote_panes.identities.get(pane_id).cloned() else {
            return;
        };
        let cancellation = Arc::new(AtomicBool::new(false));
        shell.remote_panes.uploads.insert(
            pane_id.to_owned(),
            RemoteUploadActivity {
                identity: identity.clone(),
                cancellation: Arc::clone(&cancellation),
            },
        );
        if shell.state.reconcile_terminal_progress(
            pane_id,
            zentty_core::TerminalProgressState::Indeterminate,
            unix_time_ms(),
        ) {
            shell.refresh_sidebar_metadata();
        }
        (identity, cancellation)
    };
    eprintln!(
        "zentty-linux: remote-paste pane={pane_id} state=uploading files={} target={}",
        inputs.len(),
        identity.destination.target
    );
    let completion_pane_id = pane_id.to_owned();
    let weak = Rc::downgrade(&shell);
    gtk::glib::spawn_future_local(async move {
        let worker_identity = identity.clone();
        let worker_cancellation = Arc::clone(&cancellation);
        let outcome = gtk::gio::spawn_blocking(move || {
            upload_inputs(&worker_identity, &inputs, &worker_cancellation)
        })
        .await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let outcome = outcome
            .map_err(|_| "remote upload worker panicked".to_owned())
            .and_then(std::convert::identity);
        finish_upload(
            &shell,
            &completion_pane_id,
            &identity,
            &cancellation,
            outcome,
        );
    });
}

fn upload_inputs(
    identity: &PaneSshIdentity,
    inputs: &[UploadInput],
    cancellation: &AtomicBool,
) -> Result<(Vec<String>, Option<SshDestination>), String> {
    let mut receipts = Vec::with_capacity(inputs.len());
    for input in inputs {
        if cancellation.load(Ordering::Acquire) {
            return rollback_batch(
                identity,
                &receipts,
                "upload cancelled after SSH identity changed".to_owned(),
            );
        }
        let result = match input {
            UploadInput::File(path) => upload_file(identity, path, cancellation),
            UploadInput::PngImage(bytes) => upload_png(identity, bytes, cancellation),
        };
        match result {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => return rollback_batch(identity, &receipts, error),
        }
    }
    Ok((
        receipts
            .iter()
            .map(|receipt| receipt.remote_path().to_owned())
            .collect(),
        ssh_identity::probe_ssh_destination(identity.foreground_process_id),
    ))
}

fn rollback_batch(
    identity: &PaneSshIdentity,
    receipts: &[zentty_linux::remote_transfer::RemoteTransferReceipt],
    error: String,
) -> Result<(Vec<String>, Option<SshDestination>), String> {
    match rollback_remote_transfers(&identity.destination, receipts) {
        Ok(()) => Err(error),
        Err(rollback) => Err(format!(
            "{error}; rollback failed: {:?}: {}",
            rollback.failure, rollback.detail
        )),
    }
}

fn upload_file(
    identity: &PaneSshIdentity,
    path: &Path,
    cancellation: &AtomicBool,
) -> Result<zentty_linux::remote_transfer::RemoteTransferReceipt, String> {
    let filename = path
        .file_name()
        .and_then(|filename| filename.to_str())
        .ok_or_else(|| "local upload filename is not valid UTF-8".to_owned())?;
    let upload_path = RemoteUploadPath::for_file(filename, unix_timestamp(), &short_nonce()?)
        .map_err(|error| format!("could not construct remote path: {error:?}"))?;
    execute(
        identity,
        path,
        upload_path,
        MAX_REMOTE_FILE_BYTES,
        cancellation,
    )
}

fn upload_png(
    identity: &PaneSshIdentity,
    bytes: &[u8],
    cancellation: &AtomicBool,
) -> Result<zentty_linux::remote_transfer::RemoteTransferReceipt, String> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_REMOTE_IMAGE_BYTES {
        return Err(format!(
            "clipboard image is {} bytes; limit is {MAX_REMOTE_IMAGE_BYTES}",
            bytes.len()
        ));
    }
    let local = LocalImageFile::create(bytes)?;
    let upload_path = RemoteUploadPath::for_image("png", unix_timestamp(), &short_nonce()?)
        .map_err(|error| format!("could not construct remote image path: {error:?}"))?;
    execute(
        identity,
        &local.path,
        upload_path,
        MAX_REMOTE_IMAGE_BYTES,
        cancellation,
    )
}

fn execute(
    identity: &PaneSshIdentity,
    source: &Path,
    upload_path: RemoteUploadPath,
    maximum_bytes: u64,
    cancellation: &AtomicBool,
) -> Result<zentty_linux::remote_transfer::RemoteTransferReceipt, String> {
    execute_remote_transfer(
        &RemoteTransferRequest {
            source: source.to_owned(),
            destination: identity.destination.clone(),
            upload_path,
            maximum_bytes,
            timeout: Duration::from_mins(10),
        },
        cancellation,
    )
    .map_err(|error| format!("{:?}: {}", error.failure, error.detail))
}

struct LocalImageFile {
    path: PathBuf,
}

impl LocalImageFile {
    fn create(bytes: &[u8]) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("zentty-image-{}", long_nonce()?));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|error| format!("could not create private image staging file: {error}"))?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.flush()) {
            let _ = std::fs::remove_file(&path);
            return Err(format!("could not stage clipboard image: {error}"));
        }
        Ok(Self { path })
    }
}

impl Drop for LocalImageFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn finish_upload(
    shell: &Rc<RefCell<ApplicationShell>>,
    pane_id: &str,
    identity: &PaneSshIdentity,
    cancellation: &Arc<AtomicBool>,
    outcome: Result<(Vec<String>, Option<SshDestination>), String>,
) {
    let mut shell = shell.borrow_mut();
    let is_current_activity = shell
        .remote_panes
        .uploads
        .get(pane_id)
        .is_some_and(|activity| Arc::ptr_eq(&activity.cancellation, cancellation));
    if !is_current_activity {
        return;
    }
    shell.remote_panes.uploads.remove(pane_id);
    let current_process_id = shell
        .pane_runtime
        .surface(pane_id)
        .and_then(zentty_ghostty::GhosttySurface::foreground_process_id);
    let current_identity = shell.remote_panes.identities.get(pane_id);
    let result = match outcome {
        Ok((paths, confirmed_destination))
            if !cancellation.load(Ordering::Acquire)
                && current_process_id == Some(identity.foreground_process_id)
                && current_identity == Some(identity)
                && confirmed_destination.as_ref() == Some(&identity.destination) =>
        {
            let inserted = paths
                .iter()
                .map(|path| escape_remote_path_for_shell(path))
                .collect::<Vec<_>>()
                .join(" ");
            shell
                .pane_runtime
                .surface(pane_id)
                .ok_or_else(|| "remote pane disappeared before path insertion".to_owned())
                .and_then(|surface| {
                    surface
                        .send_text(&inserted)
                        .map_err(|error| error.to_string())
                })
                .map(|()| paths)
        }
        Ok(_) => Err("SSH identity changed before path insertion".to_owned()),
        Err(error) => Err(error),
    };
    let progress = if result.is_ok() {
        zentty_core::TerminalProgressState::Remove
    } else {
        zentty_core::TerminalProgressState::Error
    };
    if shell
        .state
        .reconcile_terminal_progress(pane_id, progress, unix_time_ms())
    {
        shell.refresh_sidebar_metadata();
    }
    match result {
        Ok(paths) => eprintln!(
            "zentty-linux: remote-paste pane={pane_id} state=complete files={} paths={}",
            paths.len(),
            paths.join(",")
        ),
        Err(error) => {
            eprintln!("zentty-linux: remote-paste pane={pane_id} state=failed detail={error}");
        }
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn short_nonce() -> Result<String, String> {
    use std::io::Read as _;
    let mut bytes = [0u8; 4];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(|error| format!("could not obtain upload path entropy: {error}"))?;
    Ok(format!("{:08x}", u32::from_be_bytes(bytes)))
}

fn long_nonce() -> Result<String, String> {
    use std::fmt::Write as _;
    use std::io::Read as _;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(|error| format!("could not obtain image staging entropy: {error}"))?;
    let mut nonce = String::with_capacity(32);
    for byte in bytes {
        write!(&mut nonce, "{byte:02x}").expect("formatting into a string cannot fail");
    }
    Ok(nonce)
}

#[cfg(test)]
mod tests {
    use super::parse_uri_list;
    use std::path::PathBuf;

    #[test]
    fn uri_list_accepts_comments_crlf_and_percent_encoded_local_paths() {
        assert_eq!(
            parse_uri_list(b"# copied files\r\nfile:///tmp/Quarterly%20Report.txt\r\n\r\n")
                .unwrap(),
            vec![PathBuf::from("/tmp/Quarterly Report.txt")]
        );
    }

    #[test]
    fn uri_list_rejects_non_local_and_non_utf8_entries() {
        assert!(parse_uri_list(b"https://example.invalid/file\r\n").is_err());
        assert!(parse_uri_list(&[0xff]).is_err());
    }
}
