use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use zentty_core::{
    BookmarkStore, BookmarkStoreSnapshot, TemplateKind, TemplateRestoreFallback, WorkspaceTemplate,
    WorkspaceTemplateExportEnvelope,
};
use zentty_linux::platform::{UserDirectory, resolve_user_path};

use super::{ApplicationShell, pane_runtime::PaneRuntimeCoordinator};

mod capture;
mod storage;
pub(super) fn initialize(shell: &Rc<RefCell<ApplicationShell>>) {
    storage::initialize(shell);
}

impl ApplicationShell {
    pub(crate) fn ensure_bookmark_storage_idle(&self) -> Result<(), String> {
        if !storage::is_busy() {
            return Ok(());
        }
        let message = "Bookmark storage is still working. Wait for it to finish, then close Tornado TTY again; your save has not been cancelled.";
        self.restore_notice.show(message);
        eprintln!("zentty-linux: bookmark-storage exit-refused=true");
        Err(message.to_owned())
    }
}

#[derive(Clone)]
pub(super) struct BookmarkRuntime {
    store: BookmarkStore,
    snapshot: BookmarkStoreSnapshot,
}

impl BookmarkRuntime {
    pub(super) fn load_default() -> Result<Self, String> {
        let store = BookmarkStore::new(default_bookmark_path()?);
        let snapshot = BookmarkStoreSnapshot {
            templates: Vec::new(),
            quarantined_path: None,
        };
        Ok(Self { store, snapshot })
    }

    pub(super) fn templates(&self) -> &[WorkspaceTemplate] {
        &self.snapshot.templates
    }

    fn reload(&mut self) -> Result<(), String> {
        self.store = BookmarkStore::new_resolving_final_symlink(self.store.path())
            .map_err(|error| format!("could not resolve bookmark storage: {error}"))?;
        self.snapshot = self
            .store
            .load()
            .map_err(|error| format!("could not reload bookmarks: {error}"))?;
        if let Some(path) = &self.snapshot.quarantined_path {
            eprintln!(
                "zentty-linux: bookmarks quarantined-invalid-input path={}",
                path.display()
            );
        }
        Ok(())
    }
}

pub(super) fn save_active(
    shell: &Rc<RefCell<ApplicationShell>>,
    name: &str,
    kind: TemplateKind,
) -> Result<(), String> {
    let capture = capture::Capture::snapshot(&shell.borrow())?;
    let id = format!("template-{}", glib::uuid_string_random());
    let name = name.to_owned();
    mutate_and_refresh(shell, "save-template", move |runtime, now| {
        let template = capture.finish(&id, &name, kind, now);
        runtime
            .store
            .upsert(template, now)
            .map_err(|error| format!("could not save template: {error}"))?;
        eprintln!("zentty-linux: bookmark-saved id={id} kind={kind:?}");
        Ok(())
    })
}

pub(super) fn activate(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
) -> Result<(), String> {
    let (template, worklane_id, identities, directory, width, source) = {
        let mut shell_ref = shell.borrow_mut();
        let template = shell_ref
            .bookmark_runtime
            .snapshot
            .template(template_id)
            .cloned()
            .ok_or_else(|| format!("bookmark {template_id:?} no longer exists"))?;
        let pane_id = shell_ref.state.focused_pane_id().ok_or("No focused pane")?;
        let source = super::pane_context::PaneContext::capture(&shell_ref, pane_id)
            .ok_or("Focused pane disappeared")?;
        let directory = source.working_directory.clone();
        let identity_count = template
            .columns
            .iter()
            .map(|column| column.panes.len() + 1)
            .sum::<usize>();
        let identities = (0..identity_count)
            .map(|_| shell_ref.take_pane_id())
            .collect::<Vec<_>>();
        let worklane_id = format!("worklane-{}", shell_ref.next_worklane_number);
        shell_ref.next_worklane_number += 1;
        let readable_width = f64::from(shell_ref.pane_viewport_width());
        (
            template,
            worklane_id,
            identities,
            directory,
            readable_width,
            source,
        )
    };
    let template_id = template_id.to_owned();
    let recency_id = template_id.clone();
    storage::run_with_after(
        shell,
        "activate-template",
        move |runtime, _| {
            if runtime.snapshot.template(&template.id) != Some(&template) {
                return Err("Bookmark changed during restore. Reopen it and retry.".to_owned());
            }
            let fallback_directory = focused_fallback_directory(directory)?;
            let mut identities = identities.into_iter();
            template
                .restore(
                    &worklane_id,
                    &mut identities,
                    &fallback_directory,
                    width,
                    width,
                    command_is_available,
                )
                .map_err(|error| format!("could not restore bookmark: {error:?}"))
        },
        move |shell, restored| {
            let shell_ref = shell.borrow();
            if !source.is_current(&shell_ref)
                || shell_ref.state.focused_pane_id() != Some(source.pane_id.as_str())
            {
                return Err("The source pane changed during bookmark restore. Retry from the intended pane.".to_owned());
            }
            drop(shell_ref);
            apply_restore(shell, &template_id, restored)
        },
        Some(Box::new(move |runtime, now| {
            runtime
                .store
                .record_use(&recency_id, now)
                .map(|_| ())
                .map_err(|error| {
                    format!("Worklane opened, but bookmark recency could not be saved: {error}")
                })
        })),
    )
}

fn apply_restore(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
    restored: zentty_core::WorkspaceTemplateRestore,
) -> Result<(), String> {
    let previous = shell.borrow().state.clone();
    let (pane_ids, fallbacks) = {
        let mut shell_ref = shell.borrow_mut();
        let pane_ids = restored
            .recipe
            .columns
            .iter()
            .flat_map(|column| &column.panes)
            .map(|pane| pane.id.clone())
            .collect::<Vec<_>>();
        shell_ref
            .state
            .insert_worklane_recipe(restored.recipe)
            .map_err(|error| format!("could not insert restored worklane: {error:?}"))?;
        for (pane_id, launch) in restored.launches {
            if let Some(command) = launch.command {
                shell_ref.pane_runtime.queue_launch(
                    &pane_id,
                    command,
                    launch.environment.into_iter().collect(),
                );
            }
            if let Some(prefill) = launch.prefill {
                shell_ref.pane_runtime.queue_prefill(&pane_id, prefill);
            }
        }
        (pane_ids, restored.fallbacks)
    };
    let mut created: Vec<String> = Vec::new();
    for pane_id in &pane_ids {
        if let Err(error) = PaneRuntimeCoordinator::create_surface(shell, pane_id) {
            eprintln!("zentty-linux: bookmark-restore surface-error pane={pane_id} error={error}");
            let mut shell_ref = shell.borrow_mut();
            for created_id in &created {
                let _ = shell_ref.pane_runtime.remove(created_id, false);
                shell_ref.agent_events.unregister_pane(created_id);
            }
            for pending_id in &pane_ids {
                shell_ref.agent_events.unregister_pane(pending_id);
                shell_ref.pane_runtime.cancel_failed_creation(pending_id);
            }
            shell_ref.state = previous;
            shell_ref.render();
            shell_ref.focus_selected_surface();
            return Err(error);
        }
        created.push(pane_id.clone());
    }
    let shell_ref = shell.borrow();
    shell_ref.render();
    shell_ref.focus_selected_surface();
    for fallback in &fallbacks {
        eprintln!("zentty-linux: bookmark-restore fallback={fallback:?}");
    }
    if let Some(message) = restore_warning(&fallbacks) {
        shell_ref.restore_notice.show(&message);
        eprintln!(
            "zentty-linux: restore-notice visible=true fallbacks={}",
            fallbacks.len()
        );
    }
    eprintln!(
        "zentty-linux: bookmark-restored id={template_id} panes={} fallbacks={} at={}",
        pane_ids.len(),
        fallbacks.len(),
        now_iso8601()?
    );
    Ok(())
}

fn restore_warning(fallbacks: &[TemplateRestoreFallback]) -> Option<String> {
    let warnings = fallbacks
        .iter()
        .map(|fallback| match fallback {
            TemplateRestoreFallback::MissingDirectory {
                requested,
                fell_back_to,
                ..
            } => format!("Could not restore {requested}; the pane opened in {fell_back_to}."),
            TemplateRestoreFallback::MissingCommand { command, .. } => format!(
                "{command} is unavailable; it was inserted into the pane without being run."
            ),
        })
        .collect::<Vec<_>>();
    (!warnings.is_empty()).then(|| warnings.join(" "))
}

pub(super) fn rename(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
    name: &str,
) -> Result<(), String> {
    let template_id = template_id.to_owned();
    let name = name.to_owned();
    mutate_and_refresh(shell, "rename-template", move |runtime, now| {
        runtime
            .store
            .rename(&template_id, &name, now)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

pub(super) fn edit(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
    json: &str,
) -> Result<(), String> {
    let mut edited = serde_json::from_str::<WorkspaceTemplate>(json)
        .map_err(|error| format!("could not decode edited template: {error}"))?;
    if edited.id != template_id {
        return Err("edited template identity does not match its action target".to_owned());
    }
    let template_id = template_id.to_owned();
    mutate_and_refresh(shell, "edit-template", move |runtime, now| {
        let existing = runtime
            .snapshot
            .template(&template_id)
            .ok_or_else(|| format!("bookmark {template_id:?} no longer exists"))?;
        edited.created_at.clone_from(&existing.created_at);
        edited.pinned = existing.pinned;
        edited.last_used_at.clone_from(&existing.last_used_at);
        runtime
            .store
            .upsert(edited, now)
            .map_err(|error| error.to_string())
    })
}

pub(super) fn toggle_pin(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
) -> Result<(), String> {
    let template_id = template_id.to_owned();
    mutate_and_refresh(shell, "toggle-template-pin", move |runtime, now| {
        let pinned = runtime
            .snapshot
            .template(&template_id)
            .ok_or_else(|| format!("bookmark {template_id:?} no longer exists"))?
            .pinned;
        runtime
            .store
            .set_pinned(&template_id, !pinned, now)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

pub(super) fn duplicate(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
) -> Result<(), String> {
    let id = format!("template-{}", glib::uuid_string_random());
    let template_id = template_id.to_owned();
    mutate_and_refresh(shell, "duplicate-template", move |runtime, now| {
        runtime
            .store
            .duplicate(&template_id, &id, now)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

pub(super) fn convert(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
) -> Result<(), String> {
    let id = format!("template-{}", glib::uuid_string_random());
    let capture = capture::Capture::snapshot(&shell.borrow())?;
    let template_id = template_id.to_owned();
    mutate_and_refresh(shell, "convert-template", move |runtime, now| {
        let source = runtime
            .snapshot
            .template(&template_id)
            .cloned()
            .ok_or_else(|| format!("bookmark {template_id:?} no longer exists"))?;
        let converted = match source.kind {
            TemplateKind::Bookmark => {
                let mut converted = source.into_portable_preset(now);
                converted.id.clone_from(&id);
                converted.name = converted_name(&converted.name, TemplateKind::Preset);
                now.clone_into(&mut converted.created_at);
                now.clone_into(&mut converted.updated_at);
                converted.pinned = false;
                converted.last_used_at = None;
                converted
            }
            TemplateKind::Preset => {
                let mut converted = capture.finish(
                    &id,
                    &converted_name(&source.name, TemplateKind::Bookmark),
                    TemplateKind::Bookmark,
                    now,
                );
                converted.color = source.color;
                converted
            }
        };
        runtime
            .store
            .upsert(converted, now)
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn converted_name(name: &str, kind: TemplateKind) -> String {
    let name = name.trim();
    match (name.is_empty(), kind) {
        (true, TemplateKind::Bookmark) => "Untitled bookmark".to_owned(),
        (true, TemplateKind::Preset) => "Untitled preset".to_owned(),
        (false, TemplateKind::Bookmark) => format!("{name} (bookmark)"),
        (false, TemplateKind::Preset) => format!("{name} (preset)"),
    }
}

pub(super) fn delete(
    shell: &Rc<RefCell<ApplicationShell>>,
    template_id: &str,
) -> Result<(), String> {
    let template_id = template_id.to_owned();
    mutate_and_refresh(shell, "delete-template", move |runtime, now| {
        runtime
            .store
            .delete(&template_id, now)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

pub(super) fn update_linked(shell: &Rc<RefCell<ApplicationShell>>) -> Result<(), String> {
    let shell_ref = shell.borrow();
    let capture = capture::Capture::snapshot(&shell_ref)?;
    let origin_id = shell_ref
        .state
        .active_worklane()
        .bookmark_origin_id
        .clone()
        .ok_or_else(|| "active worklane is not linked to a bookmark".to_owned())?;
    drop(shell_ref);
    mutate_and_refresh(shell, "update-linked-template", move |runtime, now| {
        let existing = runtime
            .snapshot
            .template(&origin_id)
            .cloned()
            .ok_or_else(|| format!("linked bookmark {origin_id:?} no longer exists"))?;
        let mut updated = capture.finish(&origin_id, &existing.name, existing.kind, now);
        updated.created_at = existing.created_at;
        updated.pinned = existing.pinned;
        updated.last_used_at = existing.last_used_at;
        runtime
            .store
            .upsert(updated, now)
            .map_err(|error| format!("could not update linked bookmark: {error}"))?;
        Ok(())
    })
}

pub(super) fn unlink(shell: &Rc<RefCell<ApplicationShell>>) -> Result<(), String> {
    let mut shell_ref = shell.borrow_mut();
    let worklane_id = shell_ref.state.active_worklane_id().to_owned();
    if !shell_ref.state.set_bookmark_origin(&worklane_id, None) {
        return Err("active worklane is not linked to a bookmark".to_owned());
    }
    drop(shell_ref);
    defer_sidebar_refresh(shell);
    eprintln!("zentty-linux: action=unlink-template result=ok");
    Ok(())
}

pub(super) fn choose_import(shell: &Rc<RefCell<ApplicationShell>>) {
    eprintln!("zentty-linux: bookmark-import-chooser-requested=true");
    let window = shell.borrow().window.clone();
    let dialog = gtk::FileDialog::builder()
        .title("Import Tornado TTY preset")
        .accept_label("Import")
        .modal(true)
        .build();
    let weak = Rc::downgrade(shell);
    let parent = window.clone();
    dialog.open(
        Some(&window),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            let Ok(file) = result else {
                parent.present();
                return;
            };
            parent.present();
            let weak = weak.clone();
            glib::MainContext::default().spawn_local(async move {
                let result = async {
                    let bytes = read_import(&file)
                        .await
                        .map_err(|error| format!("could not read imported preset: {error}"))?;
                    let id = format!("template-{}", glib::uuid_string_random());
                    let shell = weak
                        .upgrade()
                        .ok_or_else(|| "window closed during preset import".to_owned())?;
                    mutate_and_refresh(&shell, "import-template", move |runtime, now| {
                        let template = WorkspaceTemplateExportEnvelope::import(&bytes, &id, now)
                            .map_err(|error| format!("could not import preset: {error}"))?;
                        runtime
                            .store
                            .upsert(template, now)
                            .map_err(|error| format!("could not persist imported preset: {error}"))
                    })
                }
                .await;
                if let Err(error) = result
                    && let Some(shell) = weak.upgrade()
                {
                    storage::fail(&shell, "import-template", &error);
                }
            });
        },
    );
}

async fn read_import(file: &gtk::gio::File) -> Result<Vec<u8>, String> {
    let stream = file
        .read_future(glib::Priority::DEFAULT)
        .await
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    loop {
        // Read at most one byte beyond the store's existing limit, including
        // for non-local Gio sources. Never load an unbounded file onto GTK.
        let remaining = BookmarkStore::MAX_FILE_BYTES + 1 - bytes.len();
        let chunk = stream
            .read_bytes_future(remaining.min(64 * 1024), glib::Priority::DEFAULT)
            .await
            .map_err(|error| error.to_string())?;
        if chunk.is_empty() {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() > BookmarkStore::MAX_FILE_BYTES {
            return Err("Preset exceeds the 1 MiB import limit".to_owned());
        }
    }
}

#[allow(deprecated)]
pub(super) fn choose_export(shell: &Rc<RefCell<ApplicationShell>>, template_id: &str) {
    eprintln!("zentty-linux: bookmark-export-chooser-requested=true");
    let prepared = (|| {
        let shell_ref = shell.borrow();
        let template = shell_ref
            .bookmark_runtime
            .snapshot
            .template(template_id)
            .cloned()
            .ok_or_else(|| format!("bookmark {template_id:?} no longer exists"))?;
        let now = now_iso8601()?;
        let bytes = WorkspaceTemplateExportEnvelope::export(template.clone(), &now)
            .map_err(|error| format!("could not export preset: {error}"))?;
        Ok::<_, String>((template.name, bytes))
    })();
    let Ok((name, bytes)) = prepared else {
        report_async_result("export-template", prepared.map(|_| ()));
        return;
    };
    let window = shell.borrow().window.clone();
    let dialog = gtk::FileChooserDialog::builder()
        .title("Export Tornado TTY preset")
        .action(gtk::FileChooserAction::Save)
        .transient_for(&window)
        .modal(true)
        .build();
    dialog.add_buttons(&[
        ("_Cancel", gtk::ResponseType::Cancel),
        ("_Save", gtk::ResponseType::Accept),
    ]);
    dialog.set_default_response(gtk::ResponseType::Accept);
    dialog.set_current_name(&format!("{}.zenttypreset", safe_filename(&name)));
    if let Ok(directory) = std::env::current_dir() {
        let _ = dialog.set_current_folder(Some(&gtk::gio::File::for_path(directory)));
    }
    glib::MainContext::default().spawn_local(async move {
        let response = dialog.run_future().await;
        if response != gtk::ResponseType::Accept {
            dialog.destroy();
            return;
        }
        let file = dialog.file();
        dialog.destroy();
        window.present();
        let Some(file) = file else {
            report_async_result(
                "export-template",
                Err("could not select exported preset destination".to_owned()),
            );
            return;
        };
        let result = file
            .replace_contents_future(
                bytes,
                None,
                false,
                gtk::gio::FileCreateFlags::REPLACE_DESTINATION,
            )
            .await
            .map(|_| ())
            .map_err(|(_, error)| format!("could not write exported preset: {error}"));
        report_async_result("export-template", result);
    });
}

fn safe_filename(name: &str) -> String {
    let name = name
        .chars()
        .map(|character| match character {
            '/' | '\0' => '_',
            character => character,
        })
        .collect::<String>();
    let name = name.trim();
    if name.is_empty() {
        "Tornado TTY preset".to_owned()
    } else {
        name.to_owned()
    }
}

fn report_async_result(action: &str, result: Result<(), String>) {
    match result {
        Ok(()) => eprintln!("zentty-linux: action={action} result=ok"),
        Err(error) => eprintln!("zentty-linux: action={action} failed: {error}"),
    }
}

fn defer_sidebar_refresh(shell: &Rc<RefCell<ApplicationShell>>) {
    let weak = Rc::downgrade(shell);
    glib::idle_add_local_once(move || {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().render_sidebar();
        }
    });
}

fn mutate_and_refresh(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &'static str,
    mutation: impl FnOnce(&mut BookmarkRuntime, &str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    storage::run(shell, action, mutation, |_, ()| Ok(()))
}

fn live_capture_context(
    shell: &ApplicationShell,
    worklane: &zentty_core::WorklaneRecipe,
) -> (
    BTreeMap<String, u64>,
    BTreeMap<String, BTreeMap<String, String>>,
) {
    let mut commands = BTreeMap::new();
    let mut environments = BTreeMap::new();
    for pane in worklane.columns.iter().flat_map(|column| &column.panes) {
        if let Some(environment) = shell.pane_runtime.explicit_environment(&pane.id) {
            environments.insert(pane.id.clone(), environment.clone());
        }
        let Some(pid) = shell
            .pane_runtime
            .surface(&pane.id)
            .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
        else {
            continue;
        };
        commands.insert(pane.id.clone(), pid);
    }
    (commands, environments)
}

fn read_proc_command(pid: u64) -> Option<String> {
    let bytes = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let parts = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| quote_proc_argument(&String::from_utf8_lossy(part)))
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn quote_proc_argument(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn focused_fallback_directory(directory: Option<PathBuf>) -> Result<String, String> {
    directory
        .map(|path| path.to_string_lossy().into_owned())
        .filter(|path| Path::new(path).is_dir())
        .or_else(|| std::env::var_os("HOME").map(|path| path.to_string_lossy().into_owned()))
        .ok_or_else(|| "no focused working directory or HOME is available".to_owned())
}

fn command_is_available(command: &str) -> bool {
    let Some(program) = command.split_whitespace().next() else {
        return false;
    };
    let program = Path::new(program);
    if program.components().count() > 1 {
        return program.is_file();
    }
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| directory.join(program).is_file())
    })
}

fn now_iso8601() -> Result<String, String> {
    glib::DateTime::now_utc()
        .and_then(|now| now.format_iso8601())
        .map(|value| value.to_string())
        .map_err(|error| format!("could not construct bookmark timestamp: {error}"))
}

fn default_bookmark_path() -> Result<PathBuf, String> {
    default_bookmark_path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn default_bookmark_path_from(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    resolve_user_path(
        UserDirectory::Config,
        xdg_config_home,
        home,
        Path::new("zentty/bookmarks.json"),
    )
    .map_err(|error| format!("could not resolve bookmarks: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        command_is_available, converted_name, default_bookmark_path_from, quote_proc_argument,
        restore_warning,
    };
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use zentty_core::{TemplateKind, TemplateRestoreFallback};

    #[test]
    fn import_reads_real_files_without_truncation_and_rejects_oversize_or_missing_input() {
        let directory = std::env::temp_dir().join(format!(
            "bookmark-import-{}",
            gtk::glib::uuid_string_random()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("preset");
        let context = gtk::glib::MainContext::new();
        context
            .with_thread_default(|| {
                let file = gtk::gio::File::for_path(&path);
                for size in [
                    0,
                    17,
                    zentty_core::BookmarkStore::MAX_FILE_BYTES,
                    zentty_core::BookmarkStore::MAX_FILE_BYTES + 1,
                ] {
                    let bytes = vec![b'x'; size];
                    std::fs::write(&path, &bytes).unwrap();
                    let result = context.block_on(super::read_import(&file));
                    if size > zentty_core::BookmarkStore::MAX_FILE_BYTES {
                        assert!(result.is_err(), "oversized import was accepted");
                    } else {
                        assert_eq!(
                            result.unwrap(),
                            bytes,
                            "valid import was truncated or changed"
                        );
                    }
                }
                std::fs::remove_file(&path).unwrap();
                assert!(context.block_on(super::read_import(&file)).is_err());
            })
            .unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn bookmark_path_obeys_xdg_then_home_and_rejects_relative_roots() {
        assert_eq!(
            default_bookmark_path_from(Some(OsStr::new("/xdg")), None).unwrap(),
            PathBuf::from("/xdg/zentty/bookmarks.json")
        );
        assert_eq!(
            default_bookmark_path_from(None, Some(OsStr::new("/home/tester"))).unwrap(),
            PathBuf::from("/home/tester/.config/zentty/bookmarks.json")
        );
        assert!(default_bookmark_path_from(Some(OsStr::new("relative")), None).is_err());
    }

    #[test]
    fn command_availability_uses_executable_path_without_a_shell() {
        assert!(command_is_available("sh -lc true"));
        assert!(!command_is_available(
            "zentty-command-that-does-not-exist --flag"
        ));
    }

    #[test]
    fn proc_arguments_are_reconstructed_without_shell_injection_or_space_loss() {
        assert_eq!(quote_proc_argument("cargo"), "cargo");
        assert_eq!(quote_proc_argument("hello world"), "'hello world'");
        assert_eq!(quote_proc_argument("it's"), "'it'\\''s'");
    }

    #[test]
    fn restore_warning_explains_every_non_silent_fallback() {
        let warnings = [
            TemplateRestoreFallback::MissingDirectory {
                pane_id: "pane-1".to_owned(),
                requested: "/gone".to_owned(),
                fell_back_to: "/home/tester".to_owned(),
            },
            TemplateRestoreFallback::MissingCommand {
                pane_id: "pane-1".to_owned(),
                command: "missing-command --flag".to_owned(),
            },
        ];
        assert_eq!(
            restore_warning(&warnings).as_deref(),
            Some(
                "Could not restore /gone; the pane opened in /home/tester. missing-command --flag is unavailable; it was inserted into the pane without being run."
            )
        );
        assert_eq!(restore_warning(&[]), None);
    }

    #[test]
    fn conversion_names_match_the_source_copy_semantics() {
        assert_eq!(
            converted_name("Demo", TemplateKind::Preset),
            "Demo (preset)"
        );
        assert_eq!(
            converted_name("Demo", TemplateKind::Bookmark),
            "Demo (bookmark)"
        );
        assert_eq!(
            converted_name("  ", TemplateKind::Preset),
            "Untitled preset"
        );
        assert_eq!(
            converted_name("  ", TemplateKind::Bookmark),
            "Untitled bookmark"
        );
    }
}
