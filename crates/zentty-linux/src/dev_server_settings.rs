use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use zentty_core::{
    SYSTEM_DEFAULT_BROWSER_ID, ServerBrowserTarget, ServerDetectionConfig, ServerPortRule,
};

pub(crate) type ApplyDevServers = Rc<dyn Fn(ServerDetectionConfig) -> Result<(), String>>;

struct State {
    config: ServerDetectionConfig,
    available: Vec<ServerBrowserTarget>,
    apply: ApplyDevServers,
    preferred: gtk::DropDown,
    preferred_ids: Vec<String>,
    browsers: gtk::Box,
    ports: gtk::Box,
    port_entry: gtk::Entry,
    status: gtk::Label,
    rebuilding: Rc<Cell<bool>>,
}

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings page.
pub(crate) fn build(
    config: ServerDetectionConfig,
    available: Vec<ServerBrowserTarget>,
    apply: ApplyDevServers,
) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(28);
    root.set_margin_bottom(28);
    root.set_margin_start(30);
    root.set_margin_end(30);
    let title = gtk::Label::new(Some("Dev Servers"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    root.append(&title);
    let subtitle = gtk::Label::new(Some(
        "Configure passive server detection, browser targets, and ignored ports.",
    ));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.add_css_class("dim-label");
    root.append(&subtitle);

    let detection = card("Detection");
    let passive = gtk::Switch::builder()
        .active(config.passive_detection_enabled)
        .valign(gtk::Align::Center)
        .build();
    passive.set_widget_name("settings-dev-servers-passive-detection");
    detection.append(&setting_row(
        "Automatically detect development servers",
        "Inspect listeners owned by pane process trees without changing those processes.",
        &passive,
    ));
    root.append(&detection);

    let browser_card = card("Browsers");
    let preferred = gtk::DropDown::from_strings(&[]);
    preferred.set_widget_name("settings-dev-servers-preferred-browser");
    browser_card.append(&setting_row(
        "Default browser",
        "Used when opening the selected development server.",
        &preferred,
    ));
    let add_browser = gtk::Button::with_label("Add App…");
    add_browser.set_widget_name("settings-dev-servers-add-browser");
    browser_card.append(&add_browser);
    let browsers = gtk::Box::new(gtk::Orientation::Vertical, 6);
    browser_card.append(&browsers);
    root.append(&browser_card);

    let ports_card = card("Ignored ports");
    let port_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let port_entry = gtk::Entry::builder()
        .placeholder_text("Port or range, e.g. 9229 or 24678-24680")
        .hexpand(true)
        .build();
    port_entry.set_widget_name("settings-dev-servers-ignored-port-entry");
    let add_port = gtk::Button::with_label("Ignore");
    add_port.set_widget_name("settings-dev-servers-add-ignored-port");
    port_row.append(&port_entry);
    port_row.append(&add_port);
    ports_card.append(&port_row);
    let ports = gtk::Box::new(gtk::Orientation::Vertical, 6);
    ports_card.append(&ports);
    root.append(&ports_card);
    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    let state = Rc::new(RefCell::new(State {
        config,
        available,
        apply,
        preferred,
        preferred_ids: Vec::new(),
        browsers,
        ports,
        port_entry,
        status,
        rebuilding: Rc::new(Cell::new(false)),
    }));
    rebuild(&state);
    {
        let state = Rc::clone(&state);
        passive.connect_active_notify(move |control| {
            state.borrow_mut().config.passive_detection_enabled = control.is_active();
            apply_and_rebuild(&state, "passive-detection");
        });
    }
    {
        let state = Rc::clone(&state);
        let preferred = state.borrow().preferred.clone();
        let rebuilding = Rc::clone(&state.borrow().rebuilding);
        preferred.connect_selected_notify(move |control| {
            if rebuilding.get() {
                return;
            }
            let id = state
                .borrow()
                .preferred_ids
                .get(control.selected() as usize)
                .cloned();
            if let Some(id) = id {
                state.borrow_mut().config.preferred_browser_id = id;
                apply_and_rebuild(&state, "preferred-browser");
            }
        });
    }
    {
        let state = Rc::clone(&state);
        add_port.connect_clicked(move |_| add_ignored_rule(&state));
    }
    {
        let state = Rc::clone(&state);
        add_browser.connect_clicked(move |button| choose_custom_browser(button, &state));
    }
    {
        let state = Rc::clone(&state);
        let entry = state.borrow().port_entry.clone();
        entry.connect_activate(move |_| add_ignored_rule(&state));
    }
    {
        let weak_root = root.downgrade();
        let weak_state = Rc::downgrade(&state);
        gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
            let (Some(_root), Some(state)) = (weak_root.upgrade(), weak_state.upgrade()) else {
                return gtk::glib::ControlFlow::Break;
            };
            reconcile_live_custom_browsers(&state);
            gtk::glib::ControlFlow::Continue
        });
    }

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label("Dev Servers Settings")]);
    scroll.upcast()
}

#[allow(clippy::too_many_lines)] // Reconciles one dynamic browser/port settings view.
fn rebuild(state: &Rc<RefCell<State>>) {
    state.borrow().rebuilding.set(true);
    while let Some(child) = state.borrow().browsers.first_child() {
        state.borrow().browsers.remove(&child);
    }
    while let Some(child) = state.borrow().ports.first_child() {
        state.borrow().ports.remove(&child);
    }
    for browser in state.borrow().available.clone() {
        let id = browser.id.clone();
        let enabled = id == SYSTEM_DEFAULT_BROWSER_ID
            || state.borrow().config.enabled_browser_target_ids.is_empty()
            || state
                .borrow()
                .config
                .enabled_browser_target_ids
                .contains(&id);
        let toggle = gtk::CheckButton::with_label(&browser.name);
        toggle.set_active(enabled);
        toggle.set_sensitive(id != SYSTEM_DEFAULT_BROWSER_ID);
        toggle.set_halign(gtk::Align::Start);
        if id != SYSTEM_DEFAULT_BROWSER_ID {
            let state_for_toggle = Rc::clone(state);
            let toggle_id = id.clone();
            toggle.connect_toggled(move |toggle| {
                let mut current = state_for_toggle.borrow().config.clone();
                if current.enabled_browser_target_ids.is_empty() {
                    current.enabled_browser_target_ids = state_for_toggle
                        .borrow()
                        .available
                        .iter()
                        .filter(|browser| browser.id != SYSTEM_DEFAULT_BROWSER_ID)
                        .map(|browser| browser.id.clone())
                        .collect();
                }
                if toggle.is_active() {
                    if !current.enabled_browser_target_ids.contains(&toggle_id) {
                        current.enabled_browser_target_ids.push(toggle_id.clone());
                    }
                } else {
                    current
                        .enabled_browser_target_ids
                        .retain(|candidate| candidate != &toggle_id);
                    if current.enabled_browser_target_ids.is_empty() {
                        // Empty is the source-compatible "all discovered browsers"
                        // default. Keep the always-available system target as an
                        // explicit sentinel when the operator disables every
                        // optional browser so that "none" cannot become "all".
                        current
                            .enabled_browser_target_ids
                            .push(SYSTEM_DEFAULT_BROWSER_ID.into());
                    }
                    if current.preferred_browser_id == toggle_id {
                        current.preferred_browser_id = SYSTEM_DEFAULT_BROWSER_ID.into();
                    }
                }
                state_for_toggle.borrow_mut().config = current.normalized();
                apply_and_rebuild(&state_for_toggle, "browser-enabled");
            });
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toggle.set_hexpand(true);
        row.append(&toggle);
        if state
            .borrow()
            .config
            .custom_browsers
            .iter()
            .any(|browser| browser.id == id)
        {
            let remove = gtk::Button::with_label("Remove");
            let state_for_remove = Rc::clone(state);
            let remove_id = id.clone();
            remove.connect_clicked(move |_| {
                let mut current = state_for_remove.borrow().config.clone();
                current
                    .custom_browsers
                    .retain(|browser| browser.id != remove_id);
                current
                    .enabled_browser_target_ids
                    .retain(|candidate| candidate != &remove_id);
                if current.preferred_browser_id == remove_id {
                    current.preferred_browser_id = SYSTEM_DEFAULT_BROWSER_ID.into();
                }
                state_for_remove.borrow_mut().config = current.normalized();
                state_for_remove
                    .borrow_mut()
                    .available
                    .retain(|browser| browser.id != remove_id);
                apply_and_rebuild(&state_for_remove, "remove-browser");
            });
            row.append(&remove);
        }
        state.borrow().browsers.append(&row);
    }
    for rule in state.borrow().config.ignored_port_rules.clone() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(&rule));
        label.set_halign(gtk::Align::Start);
        label.set_hexpand(true);
        let remove = gtk::Button::with_label("Stop Ignoring");
        let state_for_remove = Rc::clone(state);
        remove.connect_clicked(move |_| {
            state_for_remove
                .borrow_mut()
                .config
                .ignored_port_rules
                .retain(|candidate| candidate != &rule);
            apply_and_rebuild(&state_for_remove, "remove-ignored-port");
        });
        row.append(&label);
        row.append(&remove);
        state.borrow().ports.append(&row);
    }
    rebuild_preferred(state);
    state.borrow().rebuilding.set(false);
}

fn rebuild_preferred(state: &Rc<RefCell<State>>) {
    let (ids, names, selected_id) = {
        let state = state.borrow();
        let enabled = state
            .available
            .iter()
            .filter(|browser| {
                browser.id == SYSTEM_DEFAULT_BROWSER_ID
                    || state.config.enabled_browser_target_ids.is_empty()
                    || state
                        .config
                        .enabled_browser_target_ids
                        .contains(&browser.id)
            })
            .collect::<Vec<_>>();
        (
            enabled
                .iter()
                .map(|browser| browser.id.clone())
                .collect::<Vec<_>>(),
            enabled
                .iter()
                .map(|browser| browser.name.clone())
                .collect::<Vec<_>>(),
            state.config.preferred_browser_id.clone(),
        )
    };
    let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let model = gtk::StringList::new(&name_refs);
    let mut state = state.borrow_mut();
    state.preferred.set_model(Some(&model));
    state.preferred_ids = ids;
    let selected = state
        .preferred_ids
        .iter()
        .position(|id| id == &selected_id)
        .unwrap_or(0);
    state
        .preferred
        .set_selected(u32::try_from(selected).unwrap_or(0));
}

fn add_ignored_rule(state: &Rc<RefCell<State>>) {
    let raw = state.borrow().port_entry.text().to_string();
    let Some(rule) = ServerPortRule::parse(&raw) else {
        state
            .borrow()
            .status
            .set_text("Enter a valid port from 1–65535 or an inclusive range.");
        eprintln!(
            "zentty-linux: dev-server-settings control=add-ignored-port result=rejected-invalid"
        );
        return;
    };
    let previous = ServerPortRule::normalize(&state.borrow().config.ignored_port_rules)
        .into_iter()
        .map(|rule| rule.canonical())
        .collect::<Vec<_>>();
    let mut values = previous.clone();
    values.push(rule.canonical());
    let normalized = ServerPortRule::normalize(&values)
        .into_iter()
        .map(|rule| rule.canonical())
        .collect::<Vec<_>>();
    if normalized == previous {
        state
            .borrow()
            .status
            .set_text("That port is already ignored.");
        eprintln!(
            "zentty-linux: dev-server-settings control=add-ignored-port result=rejected-duplicate"
        );
        return;
    }
    state.borrow_mut().config.ignored_port_rules = normalized;
    state.borrow().port_entry.set_text("");
    apply_and_rebuild(state, "add-ignored-port");
}

fn choose_custom_browser(button: &gtk::Button, state: &Rc<RefCell<State>>) {
    let Some(window) = button.root().and_downcast::<gtk::Window>() else {
        return;
    };
    let chooser = gtk::FileDialog::builder()
        .title("Add Browser Application")
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
            let Some(canonical) =
                crate::application_shell::open_with_runtime::canonical_executable(&path)
            else {
                state
                    .borrow()
                    .status
                    .set_text("The selected file is not executable.");
                return;
            };
            let path = canonical.to_string_lossy().into_owned();
            let name = canonical
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Custom Browser")
                .to_owned();
            let id = format!("custom:{}", stable_path_id(&path));
            let mut current = state.borrow().config.clone();
            if !current
                .custom_browsers
                .iter()
                .any(|browser| browser.path == path)
            {
                current
                    .custom_browsers
                    .push(zentty_core::ServerBrowserCustomApp {
                        id: id.clone(),
                        name: name.clone(),
                        path: path.clone(),
                        bundle_identifier: None,
                    });
                current.enabled_browser_target_ids.push(id.clone());
                state.borrow_mut().available.push(ServerBrowserTarget {
                    id,
                    name,
                    launcher: zentty_core::ServerBrowserLauncher::Executable { path },
                });
            }
            state.borrow_mut().config = current.normalized();
            apply_and_rebuild(&state, "add-browser");
        },
    );
}

fn reconcile_live_custom_browsers(state: &Rc<RefCell<State>>) {
    let missing = state
        .borrow()
        .config
        .custom_browsers
        .iter()
        .filter(|browser| {
            crate::application_shell::open_with_runtime::canonical_executable(std::path::Path::new(
                &browser.path,
            ))
            .is_none()
        })
        .map(|browser| browser.id.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return;
    }
    {
        let mut state = state.borrow_mut();
        state
            .config
            .custom_browsers
            .retain(|browser| !missing.contains(&browser.id));
        state
            .config
            .enabled_browser_target_ids
            .retain(|id| !missing.contains(id));
        if missing.contains(&state.config.preferred_browser_id) {
            state.config.preferred_browser_id = SYSTEM_DEFAULT_BROWSER_ID.into();
        }
        state
            .available
            .retain(|browser| !missing.contains(&browser.id));
        state.config = state.config.clone().normalized();
    }
    eprintln!(
        "zentty-linux: dev-server-settings live-browser-invalidation removed={}",
        missing.join(",")
    );
    apply_and_rebuild(state, "browser-invalidated");
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
    let apply = Rc::clone(&state.borrow().apply);
    match apply(config.clone()) {
        Ok(()) => {
            state.borrow_mut().config = config;
            state.borrow().status.set_text("");
            eprintln!("zentty-linux: dev-server-settings control={control} result=applied");
        }
        Err(error) => state
            .borrow()
            .status
            .set_text(&format!("Could not save Dev Servers settings: {error}")),
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
    use zentty_core::ServerPortRule;

    #[test]
    fn ignored_port_input_uses_the_runtime_rule_authority() {
        assert_eq!(
            ServerPortRule::parse("3000-3002").unwrap().canonical(),
            "3000-3002"
        );
        assert!(ServerPortRule::parse("0").is_none());
    }

    #[test]
    fn custom_browser_ids_are_stable_and_path_specific() {
        assert_eq!(
            stable_path_id("/opt/browser"),
            stable_path_id("/opt/browser")
        );
        assert_ne!(stable_path_id("/opt/browser"), stable_path_id("/opt/other"));
    }
}
