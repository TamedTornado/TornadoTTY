use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{OpenWithConfig, OpenWithCustomApp, OpenWithTarget};

pub(crate) type ApplyOpenWith = Rc<dyn Fn(OpenWithConfig) -> Result<(), String>>;
pub(crate) type RefreshOpenWith = Rc<dyn Fn() -> Result<OpenWithProjection, String>>;

#[derive(Clone)]
pub(crate) struct OpenWithProjection {
    pub(crate) config: OpenWithConfig,
    pub(crate) available: Vec<OpenWithTarget>,
    pub(crate) removed_unavailable_ids: Vec<String>,
}

impl OpenWithProjection {
    pub(crate) fn reconcile(config: OpenWithConfig, available: Vec<OpenWithTarget>) -> Self {
        use std::collections::HashSet;

        let available_ids = available
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();
        let available_set = available_ids.iter().collect::<HashSet<_>>();
        let mut removed_unavailable_ids = config
            .enabled_target_ids
            .iter()
            .chain(std::iter::once(&config.primary_target_id))
            .chain(config.custom_apps.iter().map(|app| &app.id))
            .filter(|id| !available_set.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        removed_unavailable_ids.sort();
        removed_unavailable_ids.dedup();
        Self {
            config: config.reconciled_available(&available_ids),
            available,
            removed_unavailable_ids,
        }
    }
}

struct State {
    config: OpenWithConfig,
    available: Vec<OpenWithTarget>,
    apply: ApplyOpenWith,
    refresh: RefreshOpenWith,
    primary: gtk::DropDown,
    primary_ids: Vec<String>,
    no_targets: gtk::Label,
    targets: gtk::Box,
    status: gtk::Label,
    rebuilding: Rc<Cell<bool>>,
}

fn primary_control() -> gtk::DropDown {
    let primary = gtk::DropDown::from_strings(&[]);
    primary.set_widget_name("settings-open-with-primary");
    primary.set_enable_search(true);
    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter(|_| eprintln!("zentty-linux: open-with-settings focus=primary"));
    primary.add_controller(focus);
    let keys = gtk::EventControllerKey::new();
    let control = primary.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        let selected = if key == gtk::gdk::Key::Home {
            Some(0)
        } else if key == gtk::gdk::Key::End {
            control
                .model()
                .and_then(|model| model.n_items().checked_sub(1))
        } else {
            None
        };
        selected.map_or(gtk::glib::Propagation::Proceed, |selected| {
            control.set_selected(selected);
            gtk::glib::Propagation::Stop
        })
    });
    primary.add_controller(keys);
    primary
}

fn available_controls() -> (gtk::Box, gtk::Button, gtk::Button, gtk::Box) {
    let card = card("Available Apps");
    let add = gtk::Button::with_mnemonic("_Add App…");
    add.set_widget_name("settings-open-with-add");
    let add_focus = gtk::EventControllerFocus::new();
    add_focus.connect_enter(|_| eprintln!("zentty-linux: open-with-settings focus=add"));
    add.add_controller(add_focus);
    let refresh = gtk::Button::with_mnemonic("Re_fresh Apps");
    refresh.set_widget_name("settings-open-with-refresh");
    let refresh_focus = gtk::EventControllerFocus::new();
    refresh_focus.connect_enter(|_| eprintln!("zentty-linux: open-with-settings focus=refresh"));
    refresh.add_controller(refresh_focus);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.append(&add);
    actions.append(&refresh);
    card.append(&actions);
    let targets = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.append(&targets);
    (card, add, refresh, targets)
}

pub(crate) fn build(
    projection: OpenWithProjection,
    apply: ApplyOpenWith,
    refresh: RefreshOpenWith,
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
    let primary = primary_control();
    primary_card.append(&setting_row(
        "_Primary application",
        "Used by the title-bar Open button and file-opening utilities.",
        &primary,
    ));
    let no_targets = gtk::Label::new(Some(
        "No applications are enabled. Enable or add an application below.",
    ));
    no_targets.set_widget_name("settings-open-with-no-targets");
    no_targets.set_halign(gtk::Align::Start);
    no_targets.set_wrap(true);
    no_targets.add_css_class("dim-label");
    no_targets.set_visible(false);
    primary_card.append(&no_targets);
    root.append(&primary_card);

    let (available_card, add, refresh_button, targets) = available_controls();
    root.append(&available_card);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    if !projection.removed_unavailable_ids.is_empty() {
        status.set_text(&format!(
            "Removed unavailable apps: {}",
            projection.removed_unavailable_ids.join(", ")
        ));
    }

    let state = Rc::new(RefCell::new(State {
        config: projection.config,
        available: projection.available,
        apply,
        refresh,
        primary,
        primary_ids: Vec::new(),
        no_targets,
        targets,
        status,
        rebuilding: Rc::new(Cell::new(false)),
    }));
    rebuild(&state);

    {
        let state = Rc::clone(&state);
        let primary = state.borrow().primary.clone();
        let rebuilding = Rc::clone(&state.borrow().rebuilding);
        primary.connect_selected_notify(move |control| {
            if rebuilding.get() {
                return;
            }
            let id = state
                .borrow()
                .primary_ids
                .get(control.selected() as usize)
                .cloned();
            if let Some(id) = id {
                if state.borrow().config.primary_target_id == id {
                    return;
                }
                let mut next = state.borrow().config.clone();
                next.primary_target_id = id;
                apply_without_rebuild(&state, next, "primary");
            }
        });
    }
    {
        let state = Rc::clone(&state);
        add.connect_clicked(move |button| choose_custom_app(button, &state));
    }
    {
        let state = Rc::clone(&state);
        refresh_button.connect_clicked(move |_| refresh_projection(&state));
    }

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label("Open With Settings")]);
    scroll.upcast()
}

fn rebuild(state: &Rc<RefCell<State>>) {
    state.borrow().rebuilding.set(true);
    while let Some(child) = state.borrow().targets.first_child() {
        state.borrow().targets.remove(&child);
    }

    let available = state.borrow().available.clone();
    let custom = state.borrow().config.custom_apps.clone();
    for target in available {
        let id = target.id.clone();
        let enabled = state.borrow().config.enabled_target_ids.contains(&id);
        let toggle = gtk::CheckButton::with_label(&target.name);
        toggle.set_focusable(true);
        toggle.set_active(enabled);
        toggle.set_widget_name(&format!("settings-open-with-target-{id}"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toggle.set_hexpand(true);
        toggle.set_halign(gtk::Align::Start);
        row.append(&toggle);
        if custom.iter().any(|app| app.id == id) {
            let remove = gtk::Button::with_label("Remove");
            remove.set_widget_name(&format!("settings-open-with-remove-{id}"));
            let remove_focus = gtk::EventControllerFocus::new();
            let id_for_focus = id.clone();
            remove_focus.connect_enter(move |_| {
                eprintln!("zentty-linux: open-with-settings focus=remove id={id_for_focus}");
            });
            remove.add_controller(remove_focus);
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
                let next = current.normalized();
                let available = state_for_remove
                    .borrow()
                    .available
                    .iter()
                    .filter(|target| target.id != id_for_remove)
                    .cloned()
                    .collect();
                apply_and_rebuild(&state_for_remove, next, Some(available), "remove-custom");
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
            let next = current.normalized();
            apply_and_rebuild(&state_for_toggle, next, None, "target-enabled");
        });
        state.borrow().targets.append(&row);
    }
    rebuild_primary(state);
    state.borrow().rebuilding.set(false);
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
    state.no_targets.set_visible(state.primary_ids.is_empty());
    if state.primary_ids.is_empty() {
        eprintln!("zentty-linux: open-with-settings primary-state=none");
    }
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
    eprintln!("zentty-linux: open-with-settings control=add-custom result=chooser-opened");
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
    let (next, available) = {
        let state = state.borrow();
        let mut next = state.config.clone();
        let mut available = state.available.clone();
        if let Some(existing) = next.custom_apps.iter().find(|app| app.path == path) {
            let existing_id = existing.id.clone();
            if !next.enabled_target_ids.contains(&existing_id) {
                next.enabled_target_ids.push(existing_id);
            }
        } else {
            next.custom_apps.push(OpenWithCustomApp {
                id: id.clone(),
                name: name.clone(),
                path: path.clone(),
            });
            next.enabled_target_ids.push(id.clone());
            available.push(OpenWithTarget {
                id,
                name,
                kind: zentty_core::OpenWithTargetKind::Editor,
                launcher: zentty_core::OpenWithLauncher::Executable {
                    path,
                    prefix_args: Vec::new(),
                },
            });
        }
        (next.normalized(), available)
    };
    apply_and_rebuild(state, next, Some(available), "add-custom");
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

fn apply_and_rebuild(
    state: &Rc<RefCell<State>>,
    config: OpenWithConfig,
    available: Option<Vec<OpenWithTarget>>,
    control: &str,
) {
    let config = config.normalized();
    let apply = Rc::clone(&state.borrow().apply);
    match apply(config.clone()) {
        Ok(()) => {
            let mut model = state.borrow_mut();
            model.config = config;
            if let Some(available) = available {
                model.available = available;
            }
            drop(model);
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

fn apply_without_rebuild(state: &Rc<RefCell<State>>, config: OpenWithConfig, control: &str) {
    let config = config.normalized();
    let apply = Rc::clone(&state.borrow().apply);
    match apply(config.clone()) {
        Ok(()) => {
            let mut model = state.borrow_mut();
            model.config = config;
            model.status.set_text("");
            eprintln!("zentty-linux: open-with-settings control={control} result=applied");
        }
        Err(error) => {
            state
                .borrow()
                .status
                .set_text(&format!("Could not save Open With settings: {error}"));
        }
    }
}

fn refresh_projection(state: &Rc<RefCell<State>>) {
    let refresh = Rc::clone(&state.borrow().refresh);
    match refresh() {
        Ok(projection) => {
            let message = if projection.removed_unavailable_ids.is_empty() {
                "Application list refreshed.".to_owned()
            } else {
                format!(
                    "Removed unavailable apps: {}",
                    projection.removed_unavailable_ids.join(", ")
                )
            };
            let mut model = state.borrow_mut();
            model.config = projection.config;
            model.available = projection.available;
            model.status.set_text(&message);
            drop(model);
            rebuild(state);
            eprintln!("zentty-linux: open-with-settings control=refresh result=applied");
        }
        Err(error) => {
            state
                .borrow()
                .status
                .set_text(&format!("Could not refresh applications: {error}"));
            eprintln!(
                "zentty-linux: open-with-settings control=refresh result=error error={error}"
            );
        }
    }
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
    title.set_use_underline(true);
    title.set_mnemonic_widget(Some(control));
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
    use super::{OpenWithProjection, stable_path_id};
    use zentty_core::{
        OpenWithConfig, OpenWithCustomApp, OpenWithLauncher, OpenWithTarget, OpenWithTargetKind,
    };

    #[test]
    fn custom_ids_are_stable_and_path_specific() {
        assert_eq!(stable_path_id("/opt/tool"), stable_path_id("/opt/tool"));
        assert_ne!(stable_path_id("/opt/tool"), stable_path_id("/opt/other"));
    }

    #[test]
    fn projection_reports_each_removed_unavailable_id_and_falls_back_deterministically() {
        let config = OpenWithConfig {
            primary_target_id: "custom:missing".into(),
            enabled_target_ids: vec![
                "custom:missing".into(),
                "system-terminal".into(),
                "xcode".into(),
            ],
            custom_apps: vec![OpenWithCustomApp {
                id: "custom:missing".into(),
                name: "Missing Editor".into(),
                path: "/missing/editor".into(),
            }],
        };
        let available = vec![OpenWithTarget {
            id: "system-terminal".into(),
            name: "System Terminal".into(),
            kind: OpenWithTargetKind::Terminal,
            launcher: OpenWithLauncher::ExecutableDirectoryOption {
                path: "/usr/bin/terminal".into(),
                option_prefix: "--dir=".into(),
            },
        }];

        let projection = OpenWithProjection::reconcile(config, available);
        assert_eq!(
            projection.removed_unavailable_ids,
            ["custom:missing", "xcode"]
        );
        assert_eq!(projection.config.enabled_target_ids, ["system-terminal"]);
        assert_eq!(projection.config.primary_target_id, "system-terminal");
        assert!(projection.config.custom_apps.is_empty());
    }
}
