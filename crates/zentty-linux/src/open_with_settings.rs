use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{OpenWithConfig, OpenWithCustomApp, OpenWithTarget};

pub(crate) type ApplyOpenWith = Rc<dyn Fn(OpenWithConfig) -> Result<(), String>>;

struct State {
    config: OpenWithConfig,
    available: Vec<OpenWithTarget>,
    apply: ApplyOpenWith,
    primary: gtk::DropDown,
    primary_ids: Vec<String>,
    targets: gtk::Box,
    status: gtk::Label,
    rebuilding: bool,
}

pub(crate) fn build(
    config: OpenWithConfig,
    available: Vec<OpenWithTarget>,
    apply: ApplyOpenWith,
) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(28);
    root.set_margin_bottom(28);
    root.set_margin_start(30);
    root.set_margin_end(30);

    let title = gtk::Label::new(Some("Open With"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    root.append(&title);
    let subtitle = gtk::Label::new(Some(
        "Choose which editors and file managers appear in the launcher, and set the default app.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    root.append(&subtitle);

    let primary_card = card("Default app");
    let primary = gtk::DropDown::from_strings(&[]);
    primary.set_widget_name("settings-open-with-primary");
    primary_card.append(&setting_row(
        "Primary application",
        "Used by the title-bar Open button and file-opening utilities.",
        &primary,
    ));
    root.append(&primary_card);

    let available_card = card("Available Apps");
    let add = gtk::Button::with_label("Add App…");
    add.set_widget_name("settings-open-with-add");
    available_card.append(&add);
    let targets = gtk::Box::new(gtk::Orientation::Vertical, 6);
    available_card.append(&targets);
    root.append(&available_card);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    let state = Rc::new(RefCell::new(State {
        config,
        available,
        apply,
        primary,
        primary_ids: Vec::new(),
        targets,
        status,
        rebuilding: false,
    }));
    rebuild(&state);

    {
        let state = Rc::clone(&state);
        let primary = state.borrow().primary.clone();
        primary.connect_selected_notify(move |control| {
            if state.borrow().rebuilding {
                return;
            }
            let id = state
                .borrow()
                .primary_ids
                .get(control.selected() as usize)
                .cloned();
            if let Some(id) = id {
                state.borrow_mut().config.primary_target_id = id;
                apply_and_rebuild(&state, "primary");
            }
        });
    }
    {
        let state = Rc::clone(&state);
        add.connect_clicked(move |button| choose_custom_app(button, &state));
    }

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label("Open With Settings")]);
    scroll.upcast()
}

fn rebuild(state: &Rc<RefCell<State>>) {
    state.borrow_mut().rebuilding = true;
    while let Some(child) = state.borrow().targets.first_child() {
        state.borrow().targets.remove(&child);
    }

    let available = state.borrow().available.clone();
    let custom = state.borrow().config.custom_apps.clone();
    for target in available {
        let id = target.id.clone();
        let enabled = state.borrow().config.enabled_target_ids.contains(&id);
        let toggle = gtk::CheckButton::with_label(&target.name);
        toggle.set_active(enabled);
        toggle.set_widget_name(&format!("settings-open-with-target-{id}"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toggle.set_hexpand(true);
        toggle.set_halign(gtk::Align::Start);
        row.append(&toggle);
        if custom.iter().any(|app| app.id == id) {
            let remove = gtk::Button::with_label("Remove");
            remove.set_widget_name(&format!("settings-open-with-remove-{id}"));
            let state_for_remove = Rc::clone(state);
            let id_for_remove = id.clone();
            remove.connect_clicked(move |_| {
                let mut current = state_for_remove.borrow().config.clone();
                current.custom_apps.retain(|app| app.id != id_for_remove);
                current
                    .enabled_target_ids
                    .retain(|candidate| candidate != &id_for_remove);
                if current.primary_target_id == id_for_remove {
                    current.primary_target_id = current
                        .enabled_target_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "system-file-manager".into());
                }
                state_for_remove.borrow_mut().config = current.normalized();
                apply_and_rebuild(&state_for_remove, "remove-custom");
            });
            row.append(&remove);
        }
        let state_for_toggle = Rc::clone(state);
        toggle.connect_toggled(move |toggle| {
            let mut current = state_for_toggle.borrow().config.clone();
            if toggle.is_active() {
                if !current.enabled_target_ids.contains(&id) {
                    current.enabled_target_ids.push(id.clone());
                }
            } else {
                current
                    .enabled_target_ids
                    .retain(|candidate| candidate != &id);
                if current.primary_target_id == id {
                    current.primary_target_id = current
                        .enabled_target_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "system-file-manager".into());
                }
            }
            state_for_toggle.borrow_mut().config = current.normalized();
            apply_and_rebuild(&state_for_toggle, "target-enabled");
        });
        state.borrow().targets.append(&row);
    }
    rebuild_primary(state);
    state.borrow_mut().rebuilding = false;
}

fn rebuild_primary(state: &Rc<RefCell<State>>) {
    let (ids, names, selected_id) = {
        let state = state.borrow();
        let enabled = state
            .available
            .iter()
            .filter(|target| state.config.enabled_target_ids.contains(&target.id))
            .collect::<Vec<_>>();
        (
            enabled
                .iter()
                .map(|target| target.id.clone())
                .collect::<Vec<_>>(),
            enabled
                .iter()
                .map(|target| target.name.clone())
                .collect::<Vec<_>>(),
            state.config.primary_target_id.clone(),
        )
    };
    let names = names.iter().map(String::as_str).collect::<Vec<_>>();
    let model = gtk::StringList::new(&names);
    let mut state = state.borrow_mut();
    state.primary.set_model(Some(&model));
    state.primary_ids = ids;
    let selected = state
        .primary_ids
        .iter()
        .position(|id| id == &selected_id)
        .unwrap_or(gtk::INVALID_LIST_POSITION as usize);
    state
        .primary
        .set_selected(u32::try_from(selected).unwrap_or(0));
    state.primary.set_sensitive(!state.primary_ids.is_empty());
}

fn choose_custom_app(button: &gtk::Button, state: &Rc<RefCell<State>>) {
    let Some(window) = button.root().and_downcast::<gtk::Window>() else {
        return;
    };
    let chooser = gtk::FileDialog::builder()
        .title("Add Open With Application")
        .accept_label("Add")
        .modal(true)
        .build();
    let weak = Rc::downgrade(state);
    chooser.open(
        Some(&window),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            let (Some(state), Ok(file)) = (weak.upgrade(), result) else {
                return;
            };
            let Some(path) = file.path() else {
                return;
            };
            add_custom_path(&state, &path);
        },
    );
}

fn add_custom_path(state: &Rc<RefCell<State>>, path: &Path) {
    let Some(canonical) = crate::application_shell::open_with_runtime::canonical_executable(path)
    else {
        state
            .borrow()
            .status
            .set_text("The selected file is not an executable application.");
        return;
    };
    let name = canonical
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Custom App")
        .to_owned();
    let path = canonical.to_string_lossy().into_owned();
    let id = format!("custom:{}", stable_path_id(&path));
    {
        let mut state = state.borrow_mut();
        if let Some(existing) = state.config.custom_apps.iter().find(|app| app.path == path) {
            let existing_id = existing.id.clone();
            if !state.config.enabled_target_ids.contains(&existing_id) {
                state.config.enabled_target_ids.push(existing_id);
            }
        } else {
            state.config.custom_apps.push(OpenWithCustomApp {
                id: id.clone(),
                name: name.clone(),
                path: path.clone(),
            });
            state.config.enabled_target_ids.push(id.clone());
            state.available.push(OpenWithTarget {
                id,
                name,
                kind: zentty_core::OpenWithTargetKind::Editor,
                launcher: zentty_core::OpenWithLauncher::Executable {
                    path,
                    prefix_args: Vec::new(),
                },
            });
        }
    }
    apply_and_rebuild(state, "add-custom");
}

fn stable_path_id(path: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let digest = Sha256::digest(path.as_bytes());
    digest[..8].iter().fold(String::new(), |mut id, byte| {
        write!(id, "{byte:02x}").expect("writing into a String cannot fail");
        id
    })
}

fn apply_and_rebuild(state: &Rc<RefCell<State>>, control: &str) {
    let config = state.borrow().config.clone().normalized();
    match (state.borrow().apply)(config.clone()) {
        Ok(()) => {
            state.borrow_mut().config = config;
            state.borrow().status.set_text("");
            eprintln!("zentty-linux: open-with-settings control={control} result=applied");
        }
        Err(error) => {
            state
                .borrow()
                .status
                .set_text(&format!("Could not save Open With settings: {error}"));
        }
    }
    rebuild(state);
}

fn card(title: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("zentty-settings-card");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    card.append(&heading);
    card
}

fn setting_row(title: &str, subtitle: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
    labels.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);
    row.append(control);
    row
}

#[cfg(test)]
mod tests {
    use super::stable_path_id;

    #[test]
    fn custom_ids_are_stable_and_path_specific() {
        assert_eq!(stable_path_id("/opt/tool"), stable_path_id("/opt/tool"));
        assert_ne!(stable_path_id("/opt/tool"), stable_path_id("/opt/other"));
    }
}
