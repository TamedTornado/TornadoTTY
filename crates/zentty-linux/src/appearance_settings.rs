use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use zentty_core::{
    AppearanceConfig, BackgroundOpacity, FALLBACK_DARK_THEME, FALLBACK_LIGHT_THEME,
    SidebarSelectionEmphasis, ThemeMode,
};

use crate::theme_catalog::{ThemeFilter, ThemePreview, default_theme_directories, discover_themes};
use crate::theme_preview;

pub(crate) type ApplyAppearance = Rc<dyn Fn(AppearanceConfig) -> Result<(), String>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThemeSlot {
    Dark,
    Light,
}

struct State {
    appearance: AppearanceConfig,
    themes: Vec<ThemePreview>,
    filtered: Vec<usize>,
    slot: ThemeSlot,
    filter: ThemeFilter,
    query: String,
    apply: ApplyAppearance,
    list: gtk::ListBox,
    summary: gtk::Label,
    preview: gtk::DrawingArea,
    slot_status: gtk::Label,
    dark_slot_name: gtk::Label,
    dark_slot_preview: gtk::DrawingArea,
    light_slot_name: gtk::Label,
    light_slot_preview: gtk::DrawingArea,
    count: gtk::Label,
    opacity_apply_source: Option<gtk::glib::SourceId>,
}

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings page.
pub(crate) fn build(
    initial: AppearanceConfig,
    apply: ApplyAppearance,
) -> (gtk::Widget, gtk::SearchEntry) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(24);
    root.set_margin_end(24);
    root.set_widget_name("appearance-settings-page");

    let title = gtk::Label::new(Some("Appearance"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    root.append(&title);
    let subtitle = gtk::Label::new(Some(
        "Choose independent dark and light terminal themes. Changes apply to every live pane.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    root.append(&subtitle);

    let mode_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    mode_box.add_css_class("zentty-settings-card");
    mode_box.append(&section_title("Theme behavior"));
    let mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let dark_mode = gtk::CheckButton::with_label("Always Dark");
    let automatic_mode = gtk::CheckButton::with_label("Follow System");
    let light_mode = gtk::CheckButton::with_label("Always Light");
    automatic_mode.set_group(Some(&dark_mode));
    light_mode.set_group(Some(&dark_mode));
    for button in [&dark_mode, &automatic_mode, &light_mode] {
        mode_row.append(button);
    }
    mode_box.append(&mode_row);
    root.append(&mode_box);

    let picker = gtk::Box::new(gtk::Orientation::Vertical, 8);
    picker.add_css_class("zentty-settings-card");
    let picker_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    picker_header.append(&section_title("Themes"));
    let count = gtk::Label::new(None);
    count.add_css_class("dim-label");
    count.set_hexpand(true);
    count.set_halign(gtk::Align::End);
    picker_header.append(&count);
    picker.append(&picker_header);
    let picker_help = gtk::Label::new(Some(
        "Choose the saved dark and light themes used by the behavior above.",
    ));
    picker_help.add_css_class("dim-label");
    picker_help.set_halign(gtk::Align::Start);
    picker_help.set_wrap(true);
    picker.append(&picker_help);

    let slot_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let (dark_slot, dark_slot_name, dark_slot_preview) = theme_slot_button("Dark Theme");
    dark_slot.set_widget_name("appearance-dark-slot");
    let (light_slot, light_slot_name, light_slot_preview) = theme_slot_button("Light Theme");
    light_slot.set_group(Some(&dark_slot));
    light_slot.set_widget_name("appearance-light-slot");
    for (button, slot) in [(&dark_slot, "dark"), (&light_slot, "light")] {
        button.connect_has_focus_notify(move |button| {
            if button.has_focus() {
                eprintln!("zentty-linux: appearance-settings focus=theme-slot-{slot}");
            }
        });
    }
    slot_row.append(&dark_slot);
    slot_row.append(&light_slot);
    picker.append(&slot_row);

    let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search themes")
        .hexpand(true)
        .build();
    search.set_widget_name("appearance-theme-search");
    let filter = gtk::DropDown::from_strings(&["Dark", "Light", "All"]);
    filter.set_widget_name("appearance-theme-filter");
    search_row.append(&search);
    search_row.append(&filter);
    picker.append(&search_row);

    let catalog_pane = gtk::Paned::new(gtk::Orientation::Horizontal);
    catalog_pane.set_position(360);
    catalog_pane.set_wide_handle(true);
    catalog_pane.set_vexpand(true);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_widget_name("appearance-theme-list");
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(320)
        .child(&list)
        .build();
    catalog_pane.set_start_child(Some(&scroll));
    let preview = gtk::Box::new(gtk::Orientation::Vertical, 10);
    preview.set_margin_start(20);
    preview.set_margin_end(12);
    preview.set_margin_top(14);
    let preview_title = section_title("Terminal preview");
    preview.append(&preview_title);
    let slot_status = gtk::Label::new(None);
    slot_status.set_halign(gtk::Align::Start);
    slot_status.set_wrap(true);
    slot_status.add_css_class("dim-label");
    slot_status.set_widget_name("appearance-theme-slot-status");
    preview.append(&slot_status);
    let preview_area = theme_preview::detail_area();
    preview_area.set_widget_name("appearance-theme-preview");
    preview.append(&preview_area);
    let summary = gtk::Label::new(Some("Select a theme"));
    summary.set_halign(gtk::Align::Start);
    summary.set_wrap(true);
    summary.set_selectable(true);
    summary.set_widget_name("appearance-theme-summary");
    preview.append(&summary);
    catalog_pane.set_end_child(Some(&preview));
    picker.append(&catalog_pane);
    root.append(&picker);

    let opacity_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    opacity_card.add_css_class("zentty-settings-card");
    let opacity_title = gtk::Label::with_mnemonic("Terminal background _opacity");
    opacity_title.add_css_class("heading");
    opacity_title.set_halign(gtk::Align::Start);
    opacity_card.append(&opacity_title);
    let opacity_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.20, 1.0, 0.01);
    opacity.set_hexpand(true);
    opacity.set_draw_value(false);
    opacity.set_widget_name("appearance-opacity");
    opacity_title.set_mnemonic_widget(Some(&opacity));
    opacity.connect_has_focus_notify(|opacity| {
        if opacity.has_focus() {
            eprintln!("zentty-linux: appearance-settings focus=background-opacity");
        }
    });
    let initial_opacity = initial
        .background_opacity
        .map_or(1.0, |value| f64::from(value.percent()) / 100.0);
    opacity.set_value(initial_opacity);
    let opacity_value = gtk::Label::new(Some(&format!("{:.0}%", initial_opacity * 100.0)));
    opacity_value.set_width_chars(4);
    opacity_row.append(&opacity);
    opacity_row.append(&opacity_value);
    opacity_card.append(&opacity_row);
    root.append(&opacity_card);

    let opencode_card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    opencode_card.add_css_class("zentty-settings-card");
    let opencode_copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
    opencode_copy.set_hexpand(true);
    opencode_copy.append(&section_title("Sync OpenCode Theme"));
    let opencode_help = gtk::Label::new(Some(
        "Override OpenCode's launch theme to match your current terminal theme.",
    ));
    opencode_help.add_css_class("dim-label");
    opencode_help.set_halign(gtk::Align::Start);
    opencode_help.set_wrap(true);
    opencode_copy.append(&opencode_help);
    opencode_card.append(&opencode_copy);
    let opencode_sync = gtk::Switch::new();
    opencode_sync.set_widget_name("appearance-opencode-sync");
    opencode_sync.set_valign(gtk::Align::Center);
    opencode_sync.set_active(initial.sync_opencode_theme_with_terminal);
    opencode_sync.update_property(&[gtk::accessible::Property::Label("Sync OpenCode Theme")]);
    opencode_sync.connect_has_focus_notify(|toggle| {
        if toggle.has_focus() {
            eprintln!("zentty-linux: appearance-settings focus=opencode-theme-sync");
        }
    });
    opencode_card.append(&opencode_sync);
    root.append(&opencode_card);

    let emphasis_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    emphasis_card.add_css_class("zentty-settings-card");
    emphasis_card.append(&section_title("Sidebar selection"));
    let emphasis_help = gtk::Label::new(Some("How strongly the active worklane is highlighted."));
    emphasis_help.add_css_class("dim-label");
    emphasis_help.set_halign(gtk::Align::Start);
    emphasis_card.append(&emphasis_help);
    let emphasis_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let subtle_emphasis = gtk::CheckButton::with_mnemonic("_Subtle · Quiet theme tint");
    subtle_emphasis.set_widget_name("appearance-sidebar-subtle");
    let vivid_emphasis = gtk::CheckButton::with_mnemonic("_Vivid · Worklane identity color");
    vivid_emphasis.set_widget_name("appearance-sidebar-vivid");
    vivid_emphasis.set_group(Some(&subtle_emphasis));
    subtle_emphasis
        .set_active(initial.sidebar_selection_emphasis == SidebarSelectionEmphasis::Subtle);
    vivid_emphasis
        .set_active(initial.sidebar_selection_emphasis == SidebarSelectionEmphasis::Vivid);
    for (button, name) in [(&subtle_emphasis, "subtle"), (&vivid_emphasis, "vivid")] {
        button.connect_has_focus_notify(move |button| {
            if button.has_focus() {
                eprintln!("zentty-linux: appearance-settings focus=sidebar-emphasis-{name}");
            }
        });
    }
    emphasis_row.append(&subtle_emphasis);
    emphasis_row.append(&vivid_emphasis);
    emphasis_card.append(&emphasis_row);
    root.append(&emphasis_card);

    let themes = std::env::current_exe()
        .map_err(|error| error.to_string())
        .and_then(|executable| {
            default_theme_directories(
                &executable,
                std::env::var_os("XDG_CONFIG_HOME").as_deref(),
                std::env::var_os("HOME").as_deref(),
            )
        })
        .map_or_else(
            |error| {
                eprintln!("zentty-linux: appearance-catalog result=failed detail={error}");
                Vec::new()
            },
            |(bundled, user)| discover_themes(&bundled, &user),
        );
    let initial_slot = if initial.theme_mode == ThemeMode::Light {
        ThemeSlot::Light
    } else {
        ThemeSlot::Dark
    };
    let state = Rc::new(RefCell::new(State {
        appearance: initial,
        themes,
        filtered: Vec::new(),
        slot: initial_slot,
        filter: match initial_slot {
            ThemeSlot::Dark => ThemeFilter::Dark,
            ThemeSlot::Light => ThemeFilter::Light,
        },
        query: String::new(),
        apply,
        list,
        summary,
        preview: preview_area,
        slot_status,
        dark_slot_name,
        dark_slot_preview,
        light_slot_name,
        light_slot_preview,
        count,
        opacity_apply_source: None,
    }));

    dark_mode.set_active(state.borrow().appearance.theme_mode == ThemeMode::Dark);
    automatic_mode.set_active(state.borrow().appearance.theme_mode == ThemeMode::Automatic);
    light_mode.set_active(state.borrow().appearance.theme_mode == ThemeMode::Light);
    dark_slot.set_active(initial_slot == ThemeSlot::Dark);
    light_slot.set_active(initial_slot == ThemeSlot::Light);
    filter.set_selected(u32::from(initial_slot != ThemeSlot::Dark));
    refresh_catalog(&state);
    refresh_theme_projection(&state, None);

    for (button, mode) in [
        (dark_mode, ThemeMode::Dark),
        (automatic_mode, ThemeMode::Automatic),
        (light_mode, ThemeMode::Light),
    ] {
        let state = Rc::clone(&state);
        button.connect_toggled(move |button| {
            if button.is_active() {
                state.borrow_mut().appearance.theme_mode = mode;
                apply_state(&state, "theme-mode");
                refresh_theme_projection(&state, None);
            }
        });
    }
    for (button, slot, filter_index) in [
        (dark_slot, ThemeSlot::Dark, 0),
        (light_slot, ThemeSlot::Light, 1),
    ] {
        let state = Rc::clone(&state);
        let filter = filter.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                let mut current = state.borrow_mut();
                current.slot = slot;
                current.filter = if slot == ThemeSlot::Dark {
                    ThemeFilter::Dark
                } else {
                    ThemeFilter::Light
                };
                drop(current);
                filter.set_selected(filter_index);
                refresh_catalog(&state);
                refresh_theme_projection(&state, None);
            }
        });
    }
    {
        let state = Rc::clone(&state);
        search.connect_search_changed(move |search| {
            state.borrow_mut().query = search.text().to_string();
            refresh_catalog(&state);
        });
    }
    {
        let state = Rc::clone(&state);
        search.connect_activate(move |_| {
            let mut current = state.borrow_mut();
            if current.filtered.len() != 1 {
                return;
            }
            let theme = current.themes[current.filtered[0]].clone();
            match current.slot {
                ThemeSlot::Dark => {
                    current.appearance.preferred_dark_theme_name = Some(theme.name.clone());
                }
                ThemeSlot::Light => {
                    current.appearance.preferred_light_theme_name = Some(theme.name.clone());
                }
            }
            current.summary.set_text(&preview_summary(&theme));
            log_theme_selection(&theme);
            drop(current);
            apply_state(&state, "theme-selection");
            refresh_theme_projection(&state, Some(&theme));
        });
    }
    {
        let state = Rc::clone(&state);
        filter.connect_selected_notify(move |filter| {
            state.borrow_mut().filter = match filter.selected() {
                0 => ThemeFilter::Dark,
                1 => ThemeFilter::Light,
                _ => ThemeFilter::All,
            };
            refresh_catalog(&state);
        });
    }
    {
        let state = Rc::clone(&state);
        let list = state.borrow().list.clone();
        list.connect_row_selected(move |_, row| {
            let Some(index) = row.and_then(|row| usize::try_from(row.index()).ok()) else {
                return;
            };
            let theme = {
                let current = state.borrow();
                current
                    .filtered
                    .get(index)
                    .and_then(|theme_index| current.themes.get(*theme_index))
                    .cloned()
            };
            if let Some(theme) = theme {
                refresh_theme_projection(&state, Some(&theme));
            }
        });
    }
    {
        let state = Rc::clone(&state);
        let list = state.borrow().list.clone();
        list.connect_row_activated(move |_, row| {
            let index = usize::try_from(row.index()).ok();
            let mut current = state.borrow_mut();
            let Some(theme_index) = index.and_then(|index| current.filtered.get(index).copied())
            else {
                return;
            };
            let theme = current.themes[theme_index].clone();
            match current.slot {
                ThemeSlot::Dark => {
                    current.appearance.preferred_dark_theme_name = Some(theme.name.clone());
                }
                ThemeSlot::Light => {
                    current.appearance.preferred_light_theme_name = Some(theme.name.clone());
                }
            }
            current.summary.set_text(&preview_summary(&theme));
            log_theme_selection(&theme);
            drop(current);
            apply_state(&state, "theme-selection");
            refresh_theme_projection(&state, Some(&theme));
        });
    }
    {
        let state = Rc::clone(&state);
        opacity.connect_value_changed(move |scale| {
            let value = scale.value();
            opacity_value.set_text(&format!("{:.0}%", value * 100.0));
            let mut current = state.borrow_mut();
            current.appearance.background_opacity = BackgroundOpacity::from_fraction(value);
            if let Some(source) = current.opacity_apply_source.take() {
                source.remove();
            }
            let weak_state = Rc::downgrade(&state);
            current.opacity_apply_source = Some(gtk::glib::timeout_add_local_once(
                Duration::from_millis(120),
                move || {
                    let Some(state) = weak_state.upgrade() else {
                        return;
                    };
                    state.borrow_mut().opacity_apply_source = None;
                    apply_state(&state, "background-opacity");
                },
            ));
        });
    }
    {
        let state = Rc::clone(&state);
        opencode_sync.connect_active_notify(move |toggle| {
            state
                .borrow_mut()
                .appearance
                .sync_opencode_theme_with_terminal = toggle.is_active();
            apply_state(&state, "opencode-theme-sync");
        });
    }
    for (button, emphasis) in [
        (subtle_emphasis, SidebarSelectionEmphasis::Subtle),
        (vivid_emphasis, SidebarSelectionEmphasis::Vivid),
    ] {
        let state = Rc::clone(&state);
        button.connect_toggled(move |button| {
            if button.is_active() {
                state.borrow_mut().appearance.sidebar_selection_emphasis = emphasis;
                apply_state(&state, "sidebar-selection-emphasis");
            }
        });
    }

    (root.upcast(), search)
}

fn section_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_halign(gtk::Align::Start);
    label
}

fn theme_slot_button(title: &str) -> (gtk::ToggleButton, gtk::Label, gtk::DrawingArea) {
    let button = gtk::ToggleButton::new();
    button.add_css_class("zentty-theme-slot");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    let preview = theme_preview::compact_area();
    content.append(&preview);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("dim-label");
    title.set_halign(gtk::Align::Start);
    labels.append(&title);
    let name = gtk::Label::new(Some("Default"));
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    labels.append(&name);
    content.append(&labels);
    button.set_child(Some(&content));
    button.set_hexpand(true);
    (button, name, preview)
}

fn configured_theme(state: &State, slot: ThemeSlot) -> Option<&ThemePreview> {
    let name = match slot {
        ThemeSlot::Dark => state
            .appearance
            .preferred_dark_theme_name
            .as_deref()
            .unwrap_or(FALLBACK_DARK_THEME),
        ThemeSlot::Light => state
            .appearance
            .preferred_light_theme_name
            .as_deref()
            .unwrap_or(FALLBACK_LIGHT_THEME),
    };
    state.themes.iter().find(|theme| theme.name == name)
}

fn refresh_theme_projection(state: &Rc<RefCell<State>>, candidate: Option<&ThemePreview>) {
    let state = state.borrow();
    let dark = configured_theme(&state, ThemeSlot::Dark);
    let light = configured_theme(&state, ThemeSlot::Light);
    state
        .dark_slot_name
        .set_text(dark.map_or(FALLBACK_DARK_THEME, |theme| theme.name.as_str()));
    state
        .light_slot_name
        .set_text(light.map_or(FALLBACK_LIGHT_THEME, |theme| theme.name.as_str()));
    theme_preview::configure_compact(&state.dark_slot_preview, dark);
    theme_preview::configure_compact(&state.light_slot_preview, light);

    let shown = candidate.or_else(|| configured_theme(&state, state.slot));
    theme_preview::configure_detail(&state.preview, shown);
    if let Some(theme) = shown {
        state.summary.set_text(&preview_summary(theme));
    }
    let status = theme_slot_status(state.appearance.theme_mode, state.slot);
    state.slot_status.set_text(status);
    eprintln!(
        "zentty-linux: appearance-theme-projection slot={} mode={} active={} preview={:?} palette={} cursor={} selection={}",
        match state.slot {
            ThemeSlot::Dark => "dark",
            ThemeSlot::Light => "light",
        },
        state.appearance.theme_mode.config_value(),
        matches!(
            (state.appearance.theme_mode, state.slot),
            (ThemeMode::Dark, ThemeSlot::Dark) | (ThemeMode::Light, ThemeSlot::Light)
        ),
        shown.map(|theme| theme.name.as_str()),
        shown.map_or(0, |theme| theme.palette.len()),
        shown.is_some_and(|theme| theme.cursor.is_some()),
        shown.is_some_and(|theme| {
            theme.selection_background.is_some() && theme.selection_foreground.is_some()
        }),
    );
}

fn theme_slot_status(mode: ThemeMode, slot: ThemeSlot) -> &'static str {
    match (mode, slot) {
        (ThemeMode::Dark, ThemeSlot::Dark) => "Editing Dark Theme · currently active.",
        (ThemeMode::Dark, ThemeSlot::Light) => {
            "Editing Light Theme · saved for Follow System or Always Light; the current terminal remains dark."
        }
        (ThemeMode::Light, ThemeSlot::Light) => "Editing Light Theme · currently active.",
        (ThemeMode::Light, ThemeSlot::Dark) => {
            "Editing Dark Theme · saved for Follow System or Always Dark; the current terminal remains light."
        }
        (ThemeMode::Automatic, ThemeSlot::Dark) => {
            "Editing Dark Theme · Follow System applies this slot when the desktop is dark."
        }
        (ThemeMode::Automatic, ThemeSlot::Light) => {
            "Editing Light Theme · Follow System applies this slot when the desktop is light."
        }
    }
}

fn refresh_catalog(state: &Rc<RefCell<State>>) {
    let mut state = state.borrow_mut();
    while let Some(child) = state.list.first_child() {
        state.list.remove(&child);
    }
    state.filtered = state
        .themes
        .iter()
        .enumerate()
        .filter_map(|(index, theme)| theme.matches(&state.query, state.filter).then_some(index))
        .collect();
    for theme_index in state.filtered.iter().copied() {
        let theme = &state.themes[theme_index];
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        content.set_margin_top(7);
        content.set_margin_bottom(7);
        content.set_margin_start(8);
        content.set_margin_end(8);
        let swatch = theme_preview::compact_area();
        swatch.set_content_width(128);
        swatch.set_content_height(32);
        theme_preview::configure_compact(&swatch, Some(theme));
        content.append(&swatch);
        let name = gtk::Label::new(Some(&theme.name));
        name.set_halign(gtk::Align::Start);
        name.set_hexpand(true);
        content.append(&name);
        if theme.user_owned {
            let badge = gtk::Label::new(Some("User"));
            badge.add_css_class("dim-label");
            content.append(&badge);
        }
        row.set_child(Some(&content));
        state.list.append(&row);
    }
    state
        .count
        .set_text(&format!("{} themes", state.filtered.len()));
    eprintln!(
        "zentty-linux: appearance-catalog query={:?} results={}",
        state.query,
        state.filtered.len()
    );
}

fn preview_summary(theme: &ThemePreview) -> String {
    format!(
        "{}\nBackground {}  Foreground {}\n{} palette colors{}",
        theme.name,
        theme.background.hex(),
        theme.foreground.hex(),
        theme.palette.len(),
        if theme.user_owned {
            " · User theme"
        } else {
            ""
        }
    )
}

fn log_theme_selection(theme: &ThemePreview) {
    eprintln!(
        "zentty-linux: appearance-theme selected={:?} source={} background={}",
        theme.name,
        if theme.user_owned { "user" } else { "bundled" },
        theme.background.hex()
    );
}

fn apply_state(state: &Rc<RefCell<State>>, action: &str) {
    let (appearance, apply) = {
        let state = state.borrow();
        (state.appearance.clone(), Rc::clone(&state.apply))
    };
    match apply(appearance) {
        Ok(()) => eprintln!("zentty-linux: appearance-settings action={action} result=applied"),
        Err(error) => {
            eprintln!(
                "zentty-linux: appearance-settings action={action} result=failed detail={error}"
            );
        }
    }
}

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".zentty-settings-card { background-color: alpha(@theme_fg_color, 0.06); border: 1px solid alpha(@theme_fg_color, 0.16); border-radius: 8px; padding: 14px; }\n\
         #appearance-theme-list row { border-bottom: 1px solid alpha(@theme_fg_color, 0.14); }\n\
         #appearance-theme-list row:selected { background-color: @theme_selected_bg_color; color: @theme_selected_fg_color; }\n\
         .zentty-theme-slot { padding: 0; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{ThemeMode, ThemeSlot, theme_slot_status};

    #[test]
    fn theme_slots_explain_active_and_saved_inactive_behavior() {
        assert_eq!(
            theme_slot_status(ThemeMode::Dark, ThemeSlot::Dark),
            "Editing Dark Theme · currently active."
        );
        assert!(
            theme_slot_status(ThemeMode::Dark, ThemeSlot::Light)
                .contains("current terminal remains dark")
        );
        assert_eq!(
            theme_slot_status(ThemeMode::Light, ThemeSlot::Light),
            "Editing Light Theme · currently active."
        );
        assert!(
            theme_slot_status(ThemeMode::Light, ThemeSlot::Dark)
                .contains("current terminal remains light")
        );
        assert!(
            theme_slot_status(ThemeMode::Automatic, ThemeSlot::Dark).contains("desktop is dark")
        );
        assert!(
            theme_slot_status(ThemeMode::Automatic, ThemeSlot::Light).contains("desktop is light")
        );
    }
}
