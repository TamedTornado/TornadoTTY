use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::settings_navigation::{SettingsHistory, SettingsSection};

struct State {
    section: SettingsSection,
    history: SettingsHistory,
    stack: gtk::Stack,
    buttons: Vec<(SettingsSection, gtk::ToggleButton)>,
    back: gtk::Button,
    forward: gtk::Button,
}

pub(crate) struct SettingsShell {
    pub(crate) widget: gtk::Widget,
    pub(crate) actions: gtk::gio::SimpleActionGroup,
    pub(crate) initial_focus: gtk::Widget,
}

#[derive(Clone, Copy)]
pub(crate) struct SettingsPages<'a> {
    pub(crate) general: &'a gtk::Widget,
    pub(crate) appearance: &'a gtk::Widget,
    pub(crate) shortcuts: &'a gtk::Widget,
    pub(crate) notifications: &'a gtk::Widget,
    pub(crate) updates_privacy: &'a gtk::Widget,
    pub(crate) workspace_panes: &'a gtk::Widget,
    pub(crate) open_with: &'a gtk::Widget,
    pub(crate) dev_servers: &'a gtk::Widget,
    pub(crate) agents: &'a gtk::Widget,
}

struct Widgets {
    root: gtk::Box,
    section_search: gtk::SearchEntry,
    stack: gtk::Stack,
    buttons: Vec<(SettingsSection, gtk::ToggleButton)>,
    back: gtk::Button,
    forward: gtk::Button,
}

pub(crate) fn build(
    pages: SettingsPages<'_>,
    appearance_search: &gtk::SearchEntry,
    shortcut_search: &gtk::SearchEntry,
    initial: SettingsSection,
) -> SettingsShell {
    let Widgets {
        root,
        section_search,
        stack,
        buttons,
        back,
        forward,
    } = build_widgets(pages);
    let state = Rc::new(RefCell::new(State {
        section: initial,
        history: SettingsHistory::new(initial),
        stack,
        buttons,
        back,
        forward,
    }));
    apply_selection(&state, initial, false);
    connect_navigation(&state, &section_search);
    let actions = navigation_actions(&state);
    root.add_controller(key_controller(
        &state,
        appearance_search,
        shortcut_search,
        &section_search,
    ));

    SettingsShell {
        widget: root.upcast(),
        actions,
        initial_focus: section_search.upcast(),
    }
}

fn build_widgets(pages: SettingsPages<'_>) -> Widgets {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
    sidebar.set_width_request(250);
    sidebar.set_margin_top(12);
    sidebar.set_margin_bottom(12);
    sidebar.set_margin_start(12);
    sidebar.set_margin_end(12);
    sidebar.add_css_class("zentty-settings-sidebar");

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some("Back (Ctrl+[)"));
    back.set_widget_name("settings-back");
    let forward = gtk::Button::from_icon_name("go-next-symbolic");
    forward.set_tooltip_text(Some("Forward (Ctrl+])"));
    forward.set_widget_name("settings-forward");
    let heading = gtk::Label::new(Some("Settings"));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    heading.set_hexpand(true);
    toolbar.append(&back);
    toolbar.append(&forward);
    toolbar.append(&heading);
    sidebar.append(&toolbar);

    let section_search = gtk::SearchEntry::builder()
        .placeholder_text("Search settings")
        .build();
    section_search.set_widget_name("settings-section-search");
    sidebar.append(&section_search);

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let mut buttons = Vec::new();
    let mut group = None::<gtk::ToggleButton>;
    for (index, section) in SettingsSection::ALL.into_iter().enumerate() {
        if index == 5 {
            let workspace = gtk::Label::new(Some("Workspace"));
            workspace.add_css_class("caption-heading");
            workspace.set_halign(gtk::Align::Start);
            workspace.set_margin_top(10);
            workspace.set_margin_start(8);
            sidebar.append(&workspace);
        }
        let button = gtk::ToggleButton::new();
        button.set_widget_name(&format!("settings-nav-{}", section.id()));
        if let Some(first) = &group {
            button.set_group(Some(first));
        } else {
            group = Some(button.clone());
        }
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let title = gtk::Label::new(Some(section.title()));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("heading");
        let subtitle = gtk::Label::new(Some(section.subtitle()));
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
        subtitle.add_css_class("dim-label");
        labels.append(&title);
        labels.append(&subtitle);
        button.set_child(Some(&labels));
        sidebar.append(&button);
        let page = match section {
            SettingsSection::General => pages.general.clone(),
            SettingsSection::Appearance => pages.appearance.clone(),
            SettingsSection::Shortcuts => pages.shortcuts.clone(),
            SettingsSection::Notifications => pages.notifications.clone(),
            SettingsSection::UpdatesPrivacy => pages.updates_privacy.clone(),
            SettingsSection::PaneLayout => pages.workspace_panes.clone(),
            SettingsSection::OpenWith => pages.open_with.clone(),
            SettingsSection::DevServers => pages.dev_servers.clone(),
            SettingsSection::Agents => pages.agents.clone(),
        };
        stack.add_named(&page, Some(section.id()));
        buttons.push((section, button));
    }
    root.append(&sidebar);
    root.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    root.append(&stack);

    Widgets {
        root,
        section_search,
        stack,
        buttons,
        back,
        forward,
    }
}

fn connect_navigation(state: &Rc<RefCell<State>>, section_search: &gtk::SearchEntry) {
    for (section, button) in state.borrow().buttons.clone() {
        let state = Rc::clone(state);
        button.connect_toggled(move |button| {
            if button.is_active() && state.borrow().section != section {
                apply_selection(&state, section, true);
            }
        });
    }
    {
        let state = Rc::clone(state);
        let back = state.borrow().back.clone();
        back.connect_clicked(move |_| navigate_back(&state));
    }
    {
        let state = Rc::clone(state);
        let forward = state.borrow().forward.clone();
        forward.connect_clicked(move |_| navigate_forward(&state));
    }
    {
        let state = Rc::clone(state);
        section_search.connect_search_changed(move |search| {
            let query = search.text();
            let mut results = 0;
            for (section, button) in &state.borrow().buttons {
                let matches = section.matches(query.as_str());
                button.set_visible(matches);
                results += usize::from(matches);
            }
            eprintln!("zentty-linux: settings-search query={query:?} results={results}");
        });
    }
}

fn navigation_actions(state: &Rc<RefCell<State>>) -> gtk::gio::SimpleActionGroup {
    let actions = gtk::gio::SimpleActionGroup::new();
    let select =
        gtk::gio::SimpleAction::new("select-section", Some(&String::static_variant_type()));
    {
        let state = Rc::clone(state);
        select.connect_activate(move |_, parameter| {
            let Some(value) = parameter.and_then(gtk::glib::Variant::str) else {
                return;
            };
            if let Some(section) = SettingsSection::parse(value) {
                apply_selection(&state, section, true);
            }
        });
    }
    actions.add_action(&select);
    actions
}

fn key_controller(
    state: &Rc<RefCell<State>>,
    appearance_search: &gtk::SearchEntry,
    shortcut_search: &gtk::SearchEntry,
    section_search: &gtk::SearchEntry,
) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let state_for_keys = Rc::clone(state);
    let appearance_search = appearance_search.clone();
    let shortcut_search = shortcut_search.clone();
    let sidebar_search = section_search.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            return gtk::glib::Propagation::Proceed;
        }
        if key == gtk::gdk::Key::_1 {
            apply_selection(&state_for_keys, SettingsSection::Appearance, true);
            return gtk::glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::_2 {
            apply_selection(&state_for_keys, SettingsSection::Shortcuts, true);
            return gtk::glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::bracketleft {
            navigate_back(&state_for_keys);
            return gtk::glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::bracketright {
            navigate_forward(&state_for_keys);
            return gtk::glib::Propagation::Stop;
        }
        if matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F) {
            match state_for_keys.borrow().section {
                SettingsSection::Appearance => {
                    appearance_search.grab_focus();
                }
                SettingsSection::Shortcuts => {
                    shortcut_search.grab_focus();
                    eprintln!("zentty-linux: shortcut-settings search-shortcut");
                }
                _ => {
                    sidebar_search.grab_focus();
                }
            }
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    keys
}

fn apply_selection(state: &Rc<RefCell<State>>, section: SettingsSection, record: bool) {
    let (stack, buttons, back, forward, can_back, can_forward) = {
        let mut state = state.borrow_mut();
        if record {
            state.history.record(section);
        }
        state.section = section;
        (
            state.stack.clone(),
            state.buttons.clone(),
            state.back.clone(),
            state.forward.clone(),
            state.history.can_back(),
            state.history.can_forward(),
        )
    };
    stack.set_visible_child_name(section.id());
    for (candidate, button) in &buttons {
        button.set_active(*candidate == section);
    }
    back.set_sensitive(can_back);
    forward.set_sensitive(can_forward);
    eprintln!("zentty-linux: settings-section selected={}", section.id());
}

fn navigate_back(state: &Rc<RefCell<State>>) {
    let section = state.borrow_mut().history.back();
    if let Some(section) = section {
        apply_selection(state, section, false);
    }
}

fn navigate_forward(state: &Rc<RefCell<State>>) {
    let section = state.borrow_mut().history.forward();
    if let Some(section) = section {
        apply_selection(state, section, false);
    }
}
