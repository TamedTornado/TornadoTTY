use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use zentty_core::{
    AppearanceConfig, KeyboardShortcut, ShortcutBinding, ShortcutKey, ShortcutManager,
    ShortcutModifier,
};

use crate::agents_settings::ApplyAgents;
use crate::appearance_settings::ApplyAppearance;
use crate::dev_server_settings::ApplyDevServers;
use crate::general_settings::{ApplyGeneral, GeneralSettings};
use crate::notifications_settings::ApplyNotifications;
use crate::open_with_settings::{ApplyOpenWith, OpenWithProjection, RefreshOpenWith};
use crate::settings_navigation::SettingsSection;
use crate::updates_privacy_settings::{ApplyErrorReporting, ApplyUpdates};
use crate::workspace_pane_settings::ApplyWorkspacePanes;

use crate::application_shell::shortcut_registry::{
    COMMANDS, ShortcutCategory, ShortcutCommandSpec, definitions,
};
use crate::application_shell::shortcut_runtime::shortcut_from_event;

type ApplyBindings = Rc<dyn Fn(Vec<ShortcutBinding>) -> Result<(), String>>;
const MAX_SHORTCUT_IMPORT_BYTES: u64 = 1024 * 1024;

struct ViewState {
    selected: String,
    query: String,
    recording: bool,
    clicked_modifiers: HashSet<ShortcutModifier>,
    pending_conflict: Option<(String, KeyboardShortcut)>,
    manager: Rc<RefCell<ShortcutManager>>,
    apply: ApplyBindings,
    browser: gtk::Box,
    command_title: gtk::Label,
    category: gtk::Label,
    description: gtk::Label,
    shortcut: gtk::Button,
    default_value: gtk::Label,
    conflict: gtk::Label,
    replace: gtk::Button,
    clear: gtk::Button,
    physical: gtk::Label,
    modifier_buttons: Vec<(ShortcutModifier, gtk::ToggleButton)>,
}

pub(crate) struct SettingsContext {
    pub(crate) appearance: AppearanceConfig,
    pub(crate) apply_appearance: ApplyAppearance,
    pub(crate) general: GeneralSettings,
    pub(crate) apply_general: ApplyGeneral,
    pub(crate) notifications: zentty_core::NotificationsConfig,
    pub(crate) apply_notifications: ApplyNotifications,
    pub(crate) updates: zentty_core::UpdatesConfig,
    pub(crate) error_reporting: zentty_core::ErrorReportingConfig,
    pub(crate) apply_updates: ApplyUpdates,
    pub(crate) apply_error_reporting: ApplyErrorReporting,
    pub(crate) worklanes: zentty_core::WorklaneConfig,
    pub(crate) pane_layout: zentty_core::PaneLayoutConfig,
    pub(crate) panes: zentty_core::PaneConfig,
    pub(crate) apply_workspace_panes: ApplyWorkspacePanes,
    pub(crate) open_with_projection: OpenWithProjection,
    pub(crate) apply_open_with: ApplyOpenWith,
    pub(crate) refresh_open_with: RefreshOpenWith,
    pub(crate) server_detection: zentty_core::ServerDetectionConfig,
    pub(crate) server_browser_targets: Vec<zentty_core::ServerBrowserTarget>,
    pub(crate) apply_dev_servers: ApplyDevServers,
    pub(crate) agent_teams: zentty_core::AgentTeamsConfig,
    pub(crate) agent_caffeination: zentty_core::AgentCaffeinationConfig,
    pub(crate) menu_bar: zentty_core::MenuBarConfig,
    pub(crate) agent_integrations: zentty_core::AgentIntegrationsConfig,
    pub(crate) available_agent_wrappers: std::collections::BTreeSet<String>,
    pub(crate) apply_agents: ApplyAgents,
    pub(crate) initial_section: SettingsSection,
}

pub(crate) struct SettingsWindow {
    pub(crate) window: gtk::Window,
    pub(crate) current_section: Rc<std::cell::Cell<SettingsSection>>,
}

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings view.
pub(crate) fn show(
    parent: &gtk::Window,
    manager: Rc<RefCell<ShortcutManager>>,
    apply: ApplyBindings,
    settings_context: SettingsContext,
    restore_parent_focus: &Rc<dyn Fn()>,
) -> SettingsWindow {
    let initial_section = settings_context.initial_section;
    install_styles();
    crate::appearance_settings::install_styles();
    let window = gtk::Window::builder()
        .title("Zentty Settings")
        .default_width(1080)
        .default_height(720)
        .modal(false)
        .build();
    window.set_hide_on_close(true);
    window.connect_focus_widget_notify(|window| {
        let name = gtk::prelude::GtkWindowExt::focus(window).map_or_else(
            || "none".to_owned(),
            |widget| widget.widget_name().to_string(),
        );
        eprintln!("zentty-linux: settings-focus widget={name}");
    });
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.add_css_class("zentty-shortcuts-settings");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search shortcuts")
        .hexpand(true)
        .build();
    header.append(&search);
    for (label, action) in [
        ("Left-hand preset", HeaderAction::LeftPreset),
        ("Right-hand preset", HeaderAction::RightPreset),
        ("Import…", HeaderAction::Import),
        ("Export…", HeaderAction::Export),
        ("Reset", HeaderAction::Reset),
    ] {
        let button = gtk::Button::with_label(label);
        button.set_tooltip_text(Some(match action {
            HeaderAction::LeftPreset => "Replace bindings with the source left-hand preset",
            HeaderAction::RightPreset => "Replace bindings with the source right-hand preset",
            HeaderAction::Import => "Import source-compatible shortcut TOML",
            HeaderAction::Export => "Export source-compatible shortcut TOML",
            HeaderAction::Reset => "Restore all default bindings",
        }));
        header.append(&button);
        button.set_widget_name(match action {
            HeaderAction::LeftPreset => "shortcut-preset-left",
            HeaderAction::RightPreset => "shortcut-preset-right",
            HeaderAction::Import => "shortcut-import",
            HeaderAction::Export => "shortcut-export",
            HeaderAction::Reset => "shortcut-reset",
        });
    }
    root.append(&header);

    let content = gtk::Paned::new(gtk::Orientation::Horizontal);
    content.set_wide_handle(true);
    content.set_position(275);
    content.set_vexpand(true);
    let browser = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let browser_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&browser)
        .build();
    browser_scroll.add_css_class("zentty-shortcuts-browser");
    content.set_start_child(Some(&browser_scroll));

    let detail = gtk::Box::new(gtk::Orientation::Vertical, 12);
    detail.set_margin_start(24);
    detail.set_margin_end(12);
    detail.set_margin_top(8);
    let category = gtk::Label::new(None);
    category.set_halign(gtk::Align::Start);
    category.add_css_class("dim-label");
    let command_title = gtk::Label::new(None);
    command_title.set_halign(gtk::Align::Start);
    command_title.add_css_class("title-2");
    let description = gtk::Label::new(None);
    description.set_halign(gtk::Align::Start);
    description.set_wrap(true);
    description.set_max_width_chars(60);
    detail.append(&category);
    detail.append(&command_title);
    detail.append(&description);

    let shortcut_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let shortcut = gtk::Button::with_label("Record Shortcut");
    shortcut.add_css_class("suggested-action");
    let clear = gtk::Button::with_label("Clear");
    shortcut_row.append(&shortcut);
    shortcut_row.append(&clear);
    detail.append(&shortcut_row);
    let default_value = gtk::Label::new(None);
    default_value.set_halign(gtk::Align::Start);
    default_value.add_css_class("dim-label");
    detail.append(&default_value);
    let conflict = gtk::Label::new(None);
    conflict.set_halign(gtk::Align::Start);
    conflict.set_wrap(true);
    conflict.add_css_class("error");
    let replace = gtk::Button::with_label("Replace conflicting binding");
    replace.set_halign(gtk::Align::Start);
    replace.add_css_class("destructive-action");
    detail.append(&conflict);
    detail.append(&replace);

    let preview_title = gtk::Label::new(Some("Keyboard preview"));
    preview_title.set_halign(gtk::Align::Start);
    preview_title.add_css_class("heading");
    detail.append(&preview_title);
    let modifier_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let modifier_buttons = [
        (ShortcutModifier::Command, "Ctrl"),
        (ShortcutModifier::Control, "Super"),
        (ShortcutModifier::Option, "Alt"),
        (ShortcutModifier::Shift, "Shift"),
    ]
    .into_iter()
    .map(|(modifier, label)| {
        let button = gtk::ToggleButton::with_label(label);
        modifier_row.append(&button);
        (modifier, button)
    })
    .collect::<Vec<_>>();
    detail.append(&modifier_row);
    let keyboard = build_keyboard_preview();
    detail.append(&keyboard);
    instrument_keyboard_layout(&keyboard, &detail);
    let physical = gtk::Label::new(Some(
        "Recorder uses GDK physical key events and the current keyboard layout.",
    ));
    physical.set_halign(gtk::Align::Start);
    physical.add_css_class("dim-label");
    detail.append(&physical);
    let detail_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&detail)
        .build();
    detail_scroll.set_widget_name("shortcut-detail-scroll");
    content.set_end_child(Some(&detail_scroll));
    root.append(&content);
    let (appearance_page, appearance_search) = crate::appearance_settings::build(
        settings_context.appearance,
        settings_context.apply_appearance,
    );
    let general_page =
        crate::general_settings::build(settings_context.general, &settings_context.apply_general);
    let notifications_page = crate::notifications_settings::build(
        settings_context.notifications,
        &settings_context.apply_notifications,
    );
    let updates_privacy_page = crate::updates_privacy_settings::build(
        &window,
        settings_context.updates,
        settings_context.error_reporting,
        &settings_context.apply_updates,
        &settings_context.apply_error_reporting,
    );
    let workspace_panes_page = crate::workspace_pane_settings::build(
        settings_context.worklanes,
        settings_context.pane_layout,
        settings_context.panes,
        settings_context.apply_workspace_panes,
    );
    let open_with_page = crate::open_with_settings::build(
        settings_context.open_with_projection,
        settings_context.apply_open_with,
        settings_context.refresh_open_with,
    );
    let dev_servers_page = crate::dev_server_settings::build(
        settings_context.server_detection,
        settings_context.server_browser_targets,
        settings_context.apply_dev_servers,
    );
    let agents_page = crate::agents_settings::build(
        settings_context.agent_teams,
        settings_context.agent_caffeination,
        settings_context.menu_bar,
        settings_context.agent_integrations,
        &settings_context.available_agent_wrappers,
        settings_context.apply_agents,
    );
    let settings = crate::settings_shell::build(
        crate::settings_shell::SettingsPages {
            general: &general_page,
            appearance: &appearance_page,
            shortcuts: &root.clone().upcast(),
            notifications: &notifications_page,
            updates_privacy: &updates_privacy_page,
            workspace_panes: &workspace_panes_page,
            open_with: &open_with_page,
            dev_servers: &dev_servers_page,
            agents: &agents_page,
        },
        &appearance_search,
        &search,
        settings_context.initial_section,
    );
    window.insert_action_group("settings", Some(&settings.actions));
    window.set_child(Some(&settings.widget));

    let state = Rc::new(RefCell::new(ViewState {
        selected: COMMANDS[0].command_id.into(),
        query: String::new(),
        recording: false,
        clicked_modifiers: HashSet::new(),
        pending_conflict: None,
        manager,
        apply,
        browser,
        command_title,
        category,
        description,
        shortcut,
        default_value,
        conflict,
        replace,
        clear,
        physical,
        modifier_buttons,
    }));
    rebuild_browser(&state);
    refresh_detail(&state);
    connect_search(&search, &state);
    connect_detail_controls(&state);
    connect_preview(&keyboard, &state);
    connect_header(&header, &window, &state);
    install_window_shortcuts(&window, parent, &search, Rc::clone(restore_parent_focus));
    install_recorder(&window, &state);
    let initial_search = search.clone();
    let initial_focus = settings.initial_focus.clone();
    window.connect_map(move |window| {
        let focus = if initial_section == crate::settings_navigation::SettingsSection::Shortcuts {
            initial_search.clone().upcast::<gtk::Widget>()
        } else {
            initial_focus.clone()
        };
        gtk::prelude::GtkWindowExt::set_focus(window, Some(&focus));
        glib::idle_add_local_once(move || {
            focus.grab_focus();
            eprintln!(
                "zentty-linux: shortcut-settings initial-focus value={}",
                focus.has_focus()
            );
        });
    });
    let active_search = search.clone();
    let active_section = Rc::clone(&settings.current_section);
    window.connect_is_active_notify(move |window| {
        if !window.is_active() {
            return;
        }
        let section = active_section.get();
        eprintln!(
            "zentty-linux: shortcut-settings active search-focused={} section={}",
            active_search.has_focus(),
            section.id()
        );
        if section == crate::settings_navigation::SettingsSection::Shortcuts {
            gtk::prelude::GtkWindowExt::set_focus(window, Some(&active_search));
            let search = active_search.clone();
            glib::idle_add_local_once(move || {
                search.grab_focus();
                eprintln!(
                    "zentty-linux: shortcut-settings active search-focused={}",
                    search.has_focus()
                );
            });
        }
    });
    eprintln!(
        "zentty-linux: shortcut-settings visible commands={}",
        COMMANDS.len()
    );
    window.set_transient_for(Some(parent));
    window.present();
    if initial_section == crate::settings_navigation::SettingsSection::Shortcuts {
        search.grab_focus();
    } else {
        settings.initial_focus.grab_focus();
    }
    let hide_window = window.clone();
    let parent_window = parent.clone();
    let keep_state = Rc::clone(&state);
    let restore_focus = Rc::clone(restore_parent_focus);
    window.connect_close_request(move |_| {
        let _ = &keep_state;
        hide_window.set_visible(false);
        parent_window.present();
        restore_focus();
        eprintln!("zentty-linux: shortcut-settings hidden parent-presented=true");
        glib::Propagation::Stop
    });
    SettingsWindow {
        window,
        current_section: settings.current_section,
    }
}

fn install_window_shortcuts(
    window: &gtk::Window,
    parent: &gtk::Window,
    _search: &gtk::SearchEntry,
    restore_parent_focus: Rc<dyn Fn()>,
) {
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let settings = window.clone();
    let parent = parent.clone();
    controller.connect_key_pressed(move |_, key, _, modifiers| {
        let relevant = modifiers
            & (gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SHIFT_MASK
                | gdk::ModifierType::SUPER_MASK);
        if key == gdk::Key::Escape && relevant.is_empty() {
            settings.set_visible(false);
            parent.present();
            restore_parent_focus();
            eprintln!("zentty-linux: shortcut-settings hidden parent-presented=true");
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(controller);
}

#[derive(Clone, Copy)]
enum HeaderAction {
    LeftPreset,
    RightPreset,
    Import,
    Export,
    Reset,
}

fn connect_search(search: &gtk::SearchEntry, state: &Rc<RefCell<ViewState>>) {
    let weak = Rc::downgrade(state);
    search.connect_search_changed(move |search| {
        let Some(state) = weak.upgrade() else { return };
        state.borrow_mut().query = search.text().trim().to_lowercase();
        eprintln!(
            "zentty-linux: shortcut-settings query={:?}",
            state.borrow().query
        );
        rebuild_browser(&state);
    });
    let weak = Rc::downgrade(state);
    search.connect_activate(move |search| {
        let Some(state) = weak.upgrade() else { return };
        let query = search.text().trim().to_lowercase();
        let Some(command) = COMMANDS.iter().find(|command| {
            format!("{} {}", command.title, command.description)
                .to_lowercase()
                .contains(&query)
        }) else {
            return;
        };
        {
            let mut state = state.borrow_mut();
            state.selected = command.command_id.into();
            state.recording = false;
            state.pending_conflict = None;
        }
        rebuild_browser(&state);
        refresh_detail(&state);
        state.borrow().shortcut.grab_focus();
        eprintln!(
            "zentty-linux: shortcut-settings selected={}",
            command.command_id
        );
    });
}

fn rebuild_browser(state: &Rc<RefCell<ViewState>>) {
    let (browser, query, selected) = {
        let state = state.borrow();
        (
            state.browser.clone(),
            state.query.clone(),
            state.selected.clone(),
        )
    };
    while let Some(child) = browser.first_child() {
        browser.remove(&child);
    }
    let mut count = 0;
    for category in ShortcutCategory::ALL {
        let matches = COMMANDS
            .iter()
            .filter(|command| command.category == category)
            .filter(|command| {
                query.is_empty()
                    || format!("{} {}", command.title, command.description)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            continue;
        }
        let heading = gtk::Label::new(Some(category.title()));
        heading.set_halign(gtk::Align::Start);
        heading.add_css_class("zentty-shortcut-category");
        browser.append(&heading);
        for command in matches {
            count += 1;
            let button = gtk::Button::with_label(command.title);
            button.set_halign(gtk::Align::Fill);
            button.set_hexpand(true);
            button.add_css_class("flat");
            if command.command_id == selected {
                button.add_css_class("zentty-shortcut-selected");
            }
            let id = command.command_id.to_owned();
            let weak = Rc::downgrade(state);
            button.connect_clicked(move |_| {
                let Some(state) = weak.upgrade() else { return };
                {
                    let mut state = state.borrow_mut();
                    state.selected.clone_from(&id);
                    state.recording = false;
                    state.pending_conflict = None;
                }
                rebuild_browser(&state);
                refresh_detail(&state);
            });
            browser.append(&button);
        }
    }
    if count == 0 {
        let empty = gtk::Label::new(Some("No shortcuts match your search."));
        empty.set_margin_top(24);
        empty.add_css_class("dim-label");
        browser.append(&empty);
    }
}

fn command_for(state: &ViewState) -> &'static ShortcutCommandSpec {
    COMMANDS
        .iter()
        .find(|command| command.command_id == state.selected)
        .unwrap_or(&COMMANDS[0])
}

fn refresh_detail(state: &Rc<RefCell<ViewState>>) {
    let state = state.borrow();
    let command = command_for(&state);
    state.command_title.set_text(command.title);
    state.category.set_text(command.category.title());
    state.description.set_text(command.description);
    let current = state
        .manager
        .borrow()
        .shortcut_for(command.command_id)
        .cloned();
    state.shortcut.set_label(if state.recording {
        "Press a shortcut…"
    } else {
        current
            .as_ref()
            .map_or("Record Shortcut", |_| "Change Shortcut")
    });
    state.default_value.set_text(&format!(
        "Current: {}    Default: {}",
        current
            .as_ref()
            .map_or("Unbound".into(), KeyboardShortcut::display),
        command
            .default
            .and_then(KeyboardShortcut::parse)
            .as_ref()
            .map_or("Unbound".into(), KeyboardShortcut::display)
    ));
    state.clear.set_sensitive(current.is_some());
    let conflict_text = state.pending_conflict.as_ref().map(|(id, shortcut)| {
        let title = COMMANDS
            .iter()
            .find(|command| command.command_id == id)
            .map_or(id.as_str(), |command| command.title);
        format!("{} is already assigned to {title}.", shortcut.display())
    });
    state
        .conflict
        .set_text(conflict_text.as_deref().unwrap_or(""));
    state.conflict.set_visible(conflict_text.is_some());
    state.replace.set_visible(conflict_text.is_some());
}

fn connect_detail_controls(state: &Rc<RefCell<ViewState>>) {
    let weak = Rc::downgrade(state);
    state.borrow().shortcut.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else { return };
        let mut state = state.borrow_mut();
        state.recording = true;
        state.pending_conflict = None;
        state
            .physical
            .set_text("Recording: press a physical key chord, or bare Delete to clear.");
        drop(state);
        refresh_detail(&weak.upgrade().unwrap());
    });
    let weak = Rc::downgrade(state);
    state.borrow().clear.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else { return };
        assign(&state, None);
    });
    let weak = Rc::downgrade(state);
    state.borrow().replace.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else { return };
        replace_conflict(&state);
    });
    for (modifier, button) in state.borrow().modifier_buttons.clone() {
        let weak = Rc::downgrade(state);
        button.connect_toggled(move |button| {
            let Some(state) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            if button.is_active() {
                state.clicked_modifiers.insert(modifier);
            } else {
                state.clicked_modifiers.remove(&modifier);
            }
        });
    }
}

fn build_keyboard_preview() -> gtk::Box {
    let keyboard = gtk::Box::new(gtk::Orientation::Vertical, 4);
    keyboard.add_css_class("zentty-keyboard-preview");
    for row in [
        vec!["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "⌫"],
        vec!["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
        vec!["A", "S", "D", "F", "G", "H", "J", "K", "L", "↵"],
        vec!["Z", "X", "C", "V", "B", "N", "M", "←", "↓", "↑", "→"],
        vec!["Space", "Tab"],
    ] {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        line.set_homogeneous(true);
        for key in row {
            let button = gtk::Button::with_label(key);
            button.set_widget_name(&format!("shortcut-key-{key}"));
            button.add_css_class("zentty-keycap");
            line.append(&button);
        }
        keyboard.append(&line);
    }
    keyboard
}

fn instrument_keyboard_layout(keyboard: &gtk::Box, detail: &gtk::Box) {
    let detail = detail.clone();
    keyboard.add_tick_callback(move |keyboard, _| {
        let keyboard_width = keyboard.width();
        let detail_width = detail.width();
        if keyboard_width <= 0 || detail_width <= 0 {
            return glib::ControlFlow::Continue;
        }
        eprintln!(
            "zentty-linux: shortcut-settings keyboard-layout detail={detail_width} keyboard={keyboard_width} fits={}",
            keyboard_width <= detail_width
        );
        glib::ControlFlow::Break
    });
}

fn connect_preview(keyboard: &gtk::Box, state: &Rc<RefCell<ViewState>>) {
    let mut row = keyboard.first_child();
    while let Some(line) = row {
        let mut key = line.first_child();
        while let Some(widget) = key {
            if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
                let weak = Rc::downgrade(state);
                button.connect_clicked(move |button| {
                    let Some(state) = weak.upgrade() else { return };
                    let Some(label) = button.label() else { return };
                    let Some(key) = preview_key(&label) else {
                        return;
                    };
                    let modifiers = state.borrow().clicked_modifiers.clone();
                    attempt_assignment(&state, KeyboardShortcut { key, modifiers });
                });
            }
            key = widget.next_sibling();
        }
        row = line.next_sibling();
    }
}

fn preview_key(label: &str) -> Option<ShortcutKey> {
    match label {
        "Space" => Some(ShortcutKey::Space),
        "⌫" | "Delete" => Some(ShortcutKey::Delete),
        "↵" | "Return" => Some(ShortcutKey::Return),
        "Tab" => Some(ShortcutKey::Tab),
        "←" | "Left" => Some(ShortcutKey::Left),
        "→" | "Right" => Some(ShortcutKey::Right),
        "↑" | "Up" => Some(ShortcutKey::Up),
        "↓" | "Down" => Some(ShortcutKey::Down),
        value if value.len() <= 3 && value.starts_with('F') => value[1..]
            .parse::<u8>()
            .ok()
            .filter(|number| (1..=12).contains(number))
            .map(ShortcutKey::Function),
        label => {
            let lowered = label.to_lowercase();
            let mut characters = lowered.chars();
            let character = characters.next()?;
            characters
                .next()
                .is_none()
                .then_some(ShortcutKey::Character(character))
        }
    }
}

fn install_recorder(window: &gtk::Window, state: &Rc<RefCell<ViewState>>) {
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(state);
    controller.connect_key_pressed(move |_, key, keycode, modifiers| {
        let Some(state) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !state.borrow().recording {
            return glib::Propagation::Proceed;
        }
        if key == gdk::Key::Escape {
            state.borrow_mut().recording = false;
            refresh_detail(&state);
            return glib::Propagation::Stop;
        }
        if is_modifier_key(key) {
            state.borrow().physical.set_text(&format!(
                "Physical modifier: {} (hardware keycode {keycode})",
                key.name().as_deref().unwrap_or("unknown")
            ));
            return glib::Propagation::Stop;
        }
        if matches!(key, gdk::Key::Delete | gdk::Key::BackSpace)
            && !modifiers.intersects(
                gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::SUPER_MASK
                    | gdk::ModifierType::META_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SHIFT_MASK,
            )
        {
            assign(&state, None);
            return glib::Propagation::Stop;
        }
        let Some(shortcut) = shortcut_from_event(key, modifiers) else {
            state
                .borrow()
                .physical
                .set_text("That physical key cannot be used as an application shortcut.");
            return glib::Propagation::Stop;
        };
        state.borrow().physical.set_text(&format!(
            "Physical key: {} (hardware keycode {keycode})",
            key.name().as_deref().unwrap_or("unknown")
        ));
        attempt_assignment(&state, shortcut);
        glib::Propagation::Stop
    });
    window.add_controller(controller);
}

fn is_modifier_key(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
    )
}

fn attempt_assignment(state: &Rc<RefCell<ViewState>>, shortcut: KeyboardShortcut) {
    if !shortcut.is_eligible_command_binding() {
        state
            .borrow()
            .physical
            .set_text("Add Ctrl, Super, or Alt to create an application shortcut.");
        return;
    }
    let selected = state.borrow().selected.clone();
    let conflict = state
        .borrow()
        .manager
        .borrow()
        .conflict_for(&shortcut, &selected);
    if let Some(conflict) = conflict {
        let conflicting_id = conflict.command_id.clone();
        let mut view = state.borrow_mut();
        view.recording = false;
        view.pending_conflict = Some((conflict.command_id, shortcut));
        drop(view);
        refresh_detail(state);
        state.borrow().replace.grab_focus();
        eprintln!(
            "zentty-linux: shortcut-settings conflict command={selected} existing={conflicting_id}"
        );
        return;
    }
    assign(state, Some(shortcut));
}

fn assign(state: &Rc<RefCell<ViewState>>, shortcut: Option<KeyboardShortcut>) {
    let (selected, bindings, apply) = {
        let state = state.borrow();
        let bindings = match state
            .manager
            .borrow()
            .updated_bindings(&state.selected, shortcut)
        {
            Ok(bindings) => bindings,
            Err(error) => {
                state.physical.set_text(&error);
                return;
            }
        };
        (state.selected.clone(), bindings, Rc::clone(&state.apply))
    };
    match apply(bindings) {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.selected = selected;
            state.recording = false;
            state.pending_conflict = None;
            state.physical.set_text("Saved to Zentty configuration.");
            eprintln!(
                "zentty-linux: shortcut-settings saved command={} value={}",
                state.selected,
                state
                    .manager
                    .borrow()
                    .shortcut_for(&state.selected)
                    .map_or("unbound".into(), KeyboardShortcut::storage_string)
            );
        }
        Err(error) => state
            .borrow()
            .physical
            .set_text(&format!("Save failed: {error}")),
    }
    rebuild_browser(state);
    refresh_detail(state);
}

fn replace_conflict(state: &Rc<RefCell<ViewState>>) {
    let (selected, conflicting, shortcut, current, apply) = {
        let state = state.borrow();
        let Some((conflicting, shortcut)) = state.pending_conflict.clone() else {
            return;
        };
        (
            state.selected.clone(),
            conflicting,
            shortcut,
            state.manager.borrow().bindings().to_vec(),
            Rc::clone(&state.apply),
        )
    };
    let first = ShortcutManager::new(&definitions(), &current)
        .and_then(|manager| manager.updated_bindings(&conflicting, None));
    let bindings = first.and_then(|bindings| {
        ShortcutManager::new(&definitions(), &bindings)
            .and_then(|manager| manager.updated_bindings(&selected, Some(shortcut)))
    });
    match bindings.and_then(|bindings| apply(bindings)) {
        Ok(()) => {
            state.borrow_mut().pending_conflict = None;
            state
                .borrow()
                .physical
                .set_text("Conflicting command was unbound and this shortcut was saved.");
            eprintln!("zentty-linux: shortcut-settings action=replace-conflict result=applied");
        }
        Err(error) => state
            .borrow()
            .physical
            .set_text(&format!("Save failed: {error}")),
    }
    refresh_detail(state);
}

fn connect_header(header: &gtk::Box, window: &gtk::Window, state: &Rc<RefCell<ViewState>>) {
    let mut child = header.first_child().and_then(|child| child.next_sibling());
    while let Some(widget) = child {
        if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
            let name = button.widget_name();
            let weak = Rc::downgrade(state);
            let window = window.clone();
            button.connect_clicked(move |_| {
                let Some(state) = weak.upgrade() else { return };
                match name.as_str() {
                    "shortcut-preset-left" => apply_preset(&state, Preset::Left),
                    "shortcut-preset-right" => apply_preset(&state, Preset::Right),
                    "shortcut-import" => choose_import(&window, &state),
                    "shortcut-export" => choose_export(&window, &state),
                    "shortcut-reset" => {
                        apply_all(&state, &[], "Defaults restored.", "shortcut-reset");
                    }
                    _ => {}
                }
            });
        }
        child = widget.next_sibling();
    }
}

#[derive(Clone, Copy)]
enum Preset {
    Left,
    Right,
}

fn apply_preset(state: &Rc<RefCell<ViewState>>, preset: Preset) {
    let entries = match preset {
        Preset::Left => left_preset(),
        Preset::Right => right_preset(),
    };
    let assigned = entries.keys().copied().collect::<HashSet<_>>();
    let mut bindings = entries
        .into_iter()
        .map(|(command_id, shortcut)| ShortcutBinding {
            command_id: command_id.into(),
            shortcut: KeyboardShortcut::parse(shortcut),
        })
        .collect::<Vec<_>>();
    for command in COMMANDS {
        if command.default.is_some() && !assigned.contains(command.command_id) {
            bindings.push(ShortcutBinding {
                command_id: command.command_id.into(),
                shortcut: None,
            });
        }
    }
    let message = match preset {
        Preset::Left => "Left-hand preset applied.",
        Preset::Right => "Right-hand preset applied.",
    };
    let action = match preset {
        Preset::Left => "shortcut-preset-left",
        Preset::Right => "shortcut-preset-right",
    };
    apply_all(state, &bindings, message, action);
}

fn apply_all(
    state: &Rc<RefCell<ViewState>>,
    bindings: &[ShortcutBinding],
    message: &str,
    action: &str,
) {
    let result = ShortcutManager::new(&definitions(), bindings)
        .map(|manager| manager.bindings().to_vec())
        .and_then(|bindings| (state.borrow().apply)(bindings));
    match result {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.recording = false;
            state.pending_conflict = None;
            state.physical.set_text(message);
            eprintln!("zentty-linux: shortcut-settings action={action} result=applied");
        }
        Err(error) => state
            .borrow()
            .physical
            .set_text(&format!("Could not apply shortcuts: {error}")),
    }
    rebuild_browser(state);
    refresh_detail(state);
}

fn left_preset() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("pane.focus.up", "command+w"),
        ("pane.focus.left", "command+a"),
        ("pane.focus.down", "command+s"),
        ("pane.focus.right", "command+d"),
        ("pane.resize.up", "command+shift+w"),
        ("pane.resize.left", "command+shift+a"),
        ("pane.resize.down", "command+shift+s"),
        ("pane.resize.right", "command+shift+d"),
        ("pane.search.selection", "command+e"),
        ("pane.search.find", "command+f"),
        ("pane.search.next", "command+g"),
        ("pane.search.previous", "command+shift+g"),
        ("pane.split.horizontal", "command+r"),
        ("pane.split.vertical", "command+shift+r"),
        ("pane.close_focused", "command+c"),
        ("pane.arrange.width.full", "command+1"),
        ("pane.arrange.width.halves", "command+2"),
        ("pane.arrange.width.thirds", "command+3"),
        ("pane.arrange.width.quarters", "command+4"),
        ("pane.arrange.height.full", "command+option+1"),
        ("pane.arrange.height.two_per_column", "command+option+2"),
        ("pane.arrange.height.three_per_column", "command+option+3"),
        ("pane.arrange.height.four_per_column", "command+option+4"),
        ("worklane.new", "command+n"),
        ("app.new_window", "command+shift+n"),
        ("command_palette.show", "command+x"),
        ("sidebar.toggle", "command+b"),
        ("navigate.back", "command+["),
        ("navigate.forward", "command+]"),
        ("app.open_settings", "command+,"),
    ])
}

fn right_preset() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("pane.focus.up", "command+up"),
        ("pane.focus.left", "command+left"),
        ("pane.focus.down", "command+down"),
        ("pane.focus.right", "command+right"),
        ("pane.resize.up", "command+shift+up"),
        ("pane.resize.left", "command+shift+left"),
        ("pane.resize.down", "command+shift+down"),
        ("pane.resize.right", "command+shift+right"),
        ("pane.split.horizontal", "command+j"),
        ("pane.split.vertical", "command+k"),
        ("pane.close_focused", "command+l"),
        ("pane.arrange.width.full", "command+1"),
        ("pane.arrange.width.halves", "command+2"),
        ("pane.arrange.width.thirds", "command+3"),
        ("pane.arrange.width.quarters", "command+4"),
        ("pane.arrange.height.full", "command+option+1"),
        ("pane.arrange.height.two_per_column", "command+option+2"),
        ("pane.arrange.height.three_per_column", "command+option+3"),
        ("pane.arrange.height.four_per_column", "command+option+4"),
        ("worklane.new", "command+n"),
        ("worklane.next", "command+]"),
        ("worklane.previous", "command+["),
        ("app.new_window", "command+shift+n"),
        ("pane.search.selection", "command+e"),
        ("pane.search.find", "command+f"),
        ("pane.search.next", "command+g"),
        ("pane.search.previous", "command+shift+g"),
        ("command_palette.show", "command+;"),
        ("sidebar.toggle", "command+h"),
        ("navigate.back", "command+,"),
        ("navigate.forward", "command+."),
        ("app.open_settings", "command+o"),
    ])
}

fn encode_bindings(bindings: &[ShortcutBinding]) -> String {
    let mut output = String::new();
    for binding in bindings {
        output.push_str("[[shortcuts.bindings]]\ncommand_id = ");
        output.push_str(&toml_string(&binding.command_id));
        output.push_str("\nshortcut = ");
        output.push_str(&toml_string(
            &binding
                .shortcut
                .as_ref()
                .map_or_else(String::new, KeyboardShortcut::storage_string),
        ));
        output.push_str("\n\n");
    }
    output
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string is always JSON encodable")
}

fn choose_export(window: &gtk::Window, state: &Rc<RefCell<ViewState>>) {
    let chooser = gtk::FileDialog::builder()
        .title("Export Zentty Shortcuts")
        .accept_label("Export")
        .initial_name("zentty-shortcuts.toml")
        .modal(true)
        .build();
    chooser.set_initial_folder(Some(&gtk::gio::File::for_path(gtk::glib::home_dir())));
    let bindings = state.borrow().manager.borrow().bindings().to_vec();
    let weak = Rc::downgrade(state);
    chooser.save(
        Some(window),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let result = std::fs::write(&path, encode_bindings(&bindings));
                if let Some(state) = weak.upgrade() {
                    let succeeded = result.is_ok();
                    match result {
                        Ok(()) => state
                            .borrow()
                            .physical
                            .set_text(&format!("Exported shortcuts to {}.", path.display())),
                        Err(error) => {
                            state
                                .borrow()
                                .physical
                                .set_text(&format!("Export failed: {error}"));
                            eprintln!(
                                "zentty-linux: shortcut-settings action=shortcut-export result=failed detail={error}"
                            );
                        }
                    }
                    if succeeded {
                        eprintln!(
                            "zentty-linux: shortcut-settings action=shortcut-export result=applied"
                        );
                    }
                }
            }
        },
    );
}

fn choose_import(window: &gtk::Window, state: &Rc<RefCell<ViewState>>) {
    let chooser = gtk::FileDialog::builder()
        .title("Import Zentty Shortcuts")
        .accept_label("Import")
        .modal(true)
        .build();
    chooser.set_initial_folder(Some(&gtk::gio::File::for_path(gtk::glib::home_dir())));
    let weak = Rc::downgrade(state);
    chooser.open(
        Some(window),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            if let (Some(state), Ok(file)) = (weak.upgrade(), result)
                && let Some(path) = file.path()
            {
                import_path(&state, &path);
            }
        },
    );
}

fn import_path(state: &Rc<RefCell<ViewState>>, path: &Path) {
    let result = decode_import_path(path);
    match result {
        Ok(bindings) => apply_all(state, &bindings, "Imported shortcuts.", "shortcut-import"),
        Err(error) => state
            .borrow()
            .physical
            .set_text(&format!("Import failed: {error}")),
    }
}

fn decode_import_path(path: &Path) -> Result<Vec<ShortcutBinding>, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("could not inspect import: {error}"))?;
    if metadata.len() > MAX_SHORTCUT_IMPORT_BYTES {
        return Err(format!(
            "shortcut import exceeds the {MAX_SHORTCUT_IMPORT_BYTES} byte limit"
        ));
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("could not read import: {error}"))
        .and_then(|source| zentty_core::AppConfig::parse_toml(&source))
        .map(|config| config.shortcuts)
}

fn install_styles() {
    static CSS: &str = r"
        .zentty-shortcuts-settings { background: #17191d; color: #e8eaf0; }
        .zentty-shortcuts-browser { background: #202329; border-radius: 8px; padding: 6px; }
        .zentty-shortcut-category { font-weight: 700; opacity: .65; padding: 12px 8px 4px; }
        .zentty-shortcut-selected { background: #344154; }
        .zentty-keyboard-preview { background: #202329; border-radius: 8px; padding: 10px; }
        .zentty-keycap { min-width: 24px; min-height: 30px; padding: 2px 3px; }
        label.error { color: #ff7b72; }
    ";
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_keyboard_glyphs_preserve_physical_key_meaning() {
        assert_eq!(preview_key("⌫"), Some(ShortcutKey::Delete));
        assert_eq!(preview_key("↵"), Some(ShortcutKey::Return));
        assert_eq!(preview_key("←"), Some(ShortcutKey::Left));
        assert_eq!(preview_key("↓"), Some(ShortcutKey::Down));
        assert_eq!(preview_key("↑"), Some(ShortcutKey::Up));
        assert_eq!(preview_key("→"), Some(ShortcutKey::Right));
    }

    #[test]
    fn preset_bindings_are_conflict_free_for_available_linux_commands() {
        for preset in [Preset::Left, Preset::Right] {
            let entries = match preset {
                Preset::Left => left_preset(),
                Preset::Right => right_preset(),
            };
            let bindings = entries
                .into_iter()
                .map(|(command_id, shortcut)| ShortcutBinding {
                    command_id: command_id.into(),
                    shortcut: KeyboardShortcut::parse(shortcut),
                })
                .collect::<Vec<_>>();
            // Presets override defaults wholesale in the UI; this verifies their
            // explicit assignments do not collide with one another.
            let unique = bindings
                .iter()
                .filter_map(|binding| binding.shortcut.as_ref())
                .collect::<HashSet<_>>();
            assert_eq!(unique.len(), bindings.len());
        }
    }

    #[test]
    fn export_is_source_compatible_and_round_trips_explicit_unbind() {
        let bindings = vec![ShortcutBinding {
            command_id: "sidebar.toggle".into(),
            shortcut: None,
        }];
        let encoded = encode_bindings(&bindings);
        assert_eq!(
            zentty_core::AppConfig::parse_toml(&encoded)
                .unwrap()
                .shortcuts,
            bindings
        );
    }
}
