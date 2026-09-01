use crate::about_catalog::{
    AboutMetadata, LicenseCatalog, LicenseEntry, default_icon_paths, load_default_catalog,
};
use gtk::prelude::*;
use std::rc::Rc;

const DOCS_URL: &str = "https://github.com/TamedTornado/tornadotty#readme";
const SOURCE_URL: &str = "https://github.com/TamedTornado/tornadotty";

pub(crate) fn show(parent: &gtk::Window) -> gtk::Window {
    install_styles();
    let window = gtk::Window::builder()
        .title(format!("About {}", zentty_core::PRODUCT_NAME))
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(760)
        .default_height(640)
        .build();
    window.set_widget_name("zentty-about-window");
    window.update_property(&[gtk::accessible::Property::Label("About Tornado TTY")]);

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::None);
    let (licenses, license_search) = licenses_page(&stack);
    let (about, initial_focus) = about_page(&stack, license_search.as_ref());
    stack.add_named(&about, Some("about"));
    stack.add_named(&licenses, Some("licenses"));
    stack.set_visible_child_name("about");
    window.set_child(Some(&stack));
    window.connect_close_request(|window| {
        window.set_visible(false);
        gtk::glib::Propagation::Stop
    });
    window.connect_is_active_notify(|window| {
        if window.is_active() {
            eprintln!("zentty-linux: about-view state=active");
        }
    });
    window.present();
    let focus = initial_focus.clone();
    gtk::glib::idle_add_local_once(move || {
        focus.grab_focus();
    });
    eprintln!("zentty-linux: about-view state=shown");
    window
}

fn about_page(
    stack: &gtk::Stack,
    license_search: Option<&gtk::SearchEntry>,
) -> (gtk::Widget, gtk::Button) {
    let metadata = AboutMetadata::compiled();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    root.set_margin_top(34);
    root.set_margin_bottom(34);
    root.set_margin_start(48);
    root.set_margin_end(48);
    root.set_halign(gtk::Align::Fill);

    let icon = application_icon();
    icon.set_halign(gtk::Align::Center);
    icon.set_pixel_size(112);
    icon.set_widget_name("about-application-icon");
    icon.update_property(&[gtk::accessible::Property::Label(
        "Tornado TTY application icon",
    )]);
    root.append(&icon);

    let title = gtk::Label::new(Some(zentty_core::PRODUCT_NAME));
    title.add_css_class("title-1");
    title.set_widget_name("about-title");
    root.append(&title);

    let subtitle = gtk::Label::new(Some(
        "A Ghostty-based native Linux terminal for agent-native development.",
    ));
    subtitle.set_wrap(true);
    subtitle.set_justify(gtk::Justification::Center);
    subtitle.add_css_class("dim-label");
    subtitle.set_widget_name("about-subtitle");
    root.append(&subtitle);

    let attribution = gtk::Label::new(Some(zentty_core::FORK_ATTRIBUTION));
    attribution.set_wrap(true);
    attribution.set_justify(gtk::Justification::Center);
    attribution.add_css_class("dim-label");
    attribution.set_widget_name("about-fork-attribution");
    root.append(&attribution);

    let metadata_grid = gtk::Grid::new();
    metadata_grid.set_column_spacing(14);
    metadata_grid.set_row_spacing(7);
    metadata_grid.set_halign(gtk::Align::Center);
    metadata_grid.set_widget_name("about-build-metadata");
    metadata_row(
        &metadata_grid,
        0,
        "Version",
        &metadata.version,
        "about-version",
    );
    metadata_row(&metadata_grid, 1, "Build", &metadata.build, "about-build");
    metadata_row(
        &metadata_grid,
        2,
        "Commit",
        &metadata.commit,
        "about-commit",
    );
    root.append(&metadata_grid);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.set_halign(gtk::Align::Center);
    let docs = action_button("Docs", "about-open-docs");
    docs.connect_clicked(|_| open_reviewed_uri("docs", DOCS_URL));
    let source = action_button("Source", "about-open-source");
    source.connect_clicked(|_| open_reviewed_uri("source", SOURCE_URL));
    let licenses = action_button("Licenses", "about-open-licenses");
    let stack_for_licenses = stack.clone();
    let search_for_licenses = license_search.cloned();
    licenses.connect_clicked(move |_| {
        stack_for_licenses.set_visible_child_name("licenses");
        if let Some(search) = search_for_licenses.as_ref() {
            let focused = search.grab_focus();
            eprintln!(
                "zentty-linux: about-focus widget=licenses-search requested result={focused}"
            );
        }
        eprintln!("zentty-linux: about-view page=licenses");
    });
    actions.append(&docs);
    actions.append(&source);
    actions.append(&licenses);
    root.append(&actions);
    (root.upcast(), docs)
}

fn licenses_page(stack: &gtk::Stack) -> (gtk::Widget, Option<gtk::SearchEntry>) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.set_margin_top(14);
    root.set_margin_bottom(14);
    root.set_margin_start(14);
    root.set_margin_end(14);
    root.set_widget_name("licenses-page");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let back = action_button("Back", "licenses-back");
    let stack_for_back = stack.clone();
    back.connect_clicked(move |_| {
        stack_for_back.set_visible_child_name("about");
        eprintln!("zentty-linux: about-view page=about");
    });
    let heading = gtk::Label::new(Some("Third-Party Licenses"));
    heading.add_css_class("title-2");
    heading.set_hexpand(true);
    heading.set_xalign(0.0);
    header.append(&back);
    header.append(&heading);
    root.append(&header);

    let search = match load_default_catalog() {
        Ok(catalog) => {
            let (catalog, search) = license_catalog_widget(catalog);
            root.append(&catalog);
            Some(search)
        }
        Err(error) => {
            let error_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
            error_box.add_css_class("zentty-about-error");
            error_box.set_widget_name("licenses-error");
            let title = gtk::Label::new(Some("License resources could not be loaded"));
            title.add_css_class("heading");
            title.set_xalign(0.0);
            let detail = gtk::Label::new(Some(&error));
            detail.set_wrap(true);
            detail.set_selectable(true);
            detail.set_xalign(0.0);
            error_box.append(&title);
            error_box.append(&detail);
            root.append(&error_box);
            eprintln!("zentty-linux: licenses-catalog result=failed detail={error}");
            None
        }
    };
    (root.upcast(), search)
}

fn license_catalog_widget(catalog: LicenseCatalog) -> (gtk::Widget, gtk::SearchEntry) {
    let entries = Rc::new(catalog.entries.into_iter().map(Rc::new).collect::<Vec<_>>());
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search names, versions, and licenses")
        .hexpand(true)
        .build();
    search.set_widget_name("licenses-search");
    search.update_property(&[gtk::accessible::Property::Label(
        "Search third-party licenses",
    )]);
    search.connect_has_focus_notify(|search| {
        if search.has_focus() {
            eprintln!("zentty-linux: about-focus widget=licenses-search");
        }
    });
    let count = gtk::Label::new(Some(&format!("{} components", entries.len())));
    count.add_css_class("dim-label");
    count.set_widget_name("licenses-count");
    search_row.append(&search);
    search_row.append(&count);
    root.append(&search_row);

    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_position(260);
    split.set_vexpand(true);
    split.set_wide_handle(true);
    split.set_widget_name("licenses-split");
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_widget_name("licenses-list");
    let list_scroll = gtk::ScrolledWindow::builder()
        .min_content_width(220)
        .child(&list)
        .build();
    let detail = gtk::Box::new(gtk::Orientation::Vertical, 8);
    detail.set_margin_start(12);
    detail.set_widget_name("license-detail");
    split.set_start_child(Some(&list_scroll));
    split.set_end_child(Some(&detail));
    root.append(&split);

    render_license_list(&list, &detail, &entries, "", &count);
    let list_for_search = list.clone();
    let detail_for_search = detail.clone();
    let entries_for_search = Rc::clone(&entries);
    let count_for_search = count.clone();
    search.connect_search_changed(move |search| {
        render_license_list(
            &list_for_search,
            &detail_for_search,
            &entries_for_search,
            search.text().as_str(),
            &count_for_search,
        );
    });
    eprintln!(
        "zentty-linux: licenses-catalog result=loaded entries={} zentty-revision={} ghostty-revision={}",
        entries.len(),
        catalog.zentty_revision,
        catalog.ghostty_revision
    );
    (root.upcast(), search)
}

fn render_license_list(
    list: &gtk::ListBox,
    detail: &gtk::Box,
    entries: &Rc<Vec<Rc<LicenseEntry>>>,
    query: &str,
    count: &gtk::Label,
) {
    clear_list(list);
    clear_box(detail);
    let filtered = entries
        .iter()
        .filter(|entry| entry.matches(query))
        .map(Rc::clone)
        .collect::<Vec<_>>();
    count.set_text(&format!(
        "{} of {} components",
        filtered.len(),
        entries.len()
    ));
    for entry in &filtered {
        let row = gtk::ListBoxRow::new();
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.set_hexpand(true);
        button.set_widget_name(&format!("license-entry-{}", widget_id(&entry.id)));
        button.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, version {}, {}",
            entry.display_name, entry.version, entry.license
        ))]);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let name = gtk::Label::new(Some(&entry.display_name));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        let summary = gtk::Label::new(Some(&format!("{} · {}", entry.version, entry.license)));
        summary.set_xalign(0.0);
        summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        summary.add_css_class("dim-label");
        labels.append(&name);
        labels.append(&summary);
        button.set_child(Some(&labels));
        let detail_for_click = detail.clone();
        let selected = Rc::clone(entry);
        button.connect_clicked(move |_| render_license_detail(&detail_for_click, &selected));
        row.set_child(Some(&button));
        list.append(&row);
    }
    if let Some(first) = filtered.first() {
        render_license_detail(detail, first);
        if let Some(row) = list.row_at_index(0) {
            list.select_row(Some(&row));
        }
    } else {
        let empty = gtk::Label::new(Some("No licenses match this search."));
        empty.set_xalign(0.0);
        empty.set_widget_name("licenses-empty");
        detail.append(&empty);
    }
    eprintln!(
        "zentty-linux: licenses-search query={query:?} results={}",
        filtered.len()
    );
}

fn render_license_detail(detail: &gtk::Box, entry: &LicenseEntry) {
    clear_box(detail);
    let title = gtk::Label::new(Some(&entry.display_name));
    title.add_css_class("title-3");
    title.set_xalign(0.0);
    title.set_widget_name("license-detail-name");
    let metadata = gtk::Label::new(Some(&format!(
        "Version {} · {} · {}",
        entry.version, entry.license, entry.ecosystem
    )));
    metadata.set_xalign(0.0);
    metadata.set_selectable(true);
    metadata.set_wrap(true);
    metadata.add_css_class("dim-label");
    metadata.set_widget_name("license-detail-metadata");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let source = action_button("Source", "license-open-source");
    let source_url = entry.source_url.clone();
    source.connect_clicked(move |_| open_reviewed_uri("license-source", &source_url));
    actions.append(&source);
    if let Some(homepage_url) = entry.homepage_url.clone() {
        let homepage = action_button("Homepage", "license-open-homepage");
        homepage.connect_clicked(move |_| open_reviewed_uri("license-homepage", &homepage_url));
        actions.append(&homepage);
    }
    let text = gtk::TextView::new();
    text.set_editable(false);
    text.set_cursor_visible(false);
    text.set_monospace(true);
    text.set_wrap_mode(gtk::WrapMode::WordChar);
    text.buffer().set_text(&entry.full_text);
    text.set_widget_name("license-full-text");
    text.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} license text",
        entry.display_name
    ))]);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .child(&text)
        .build();
    detail.append(&title);
    detail.append(&metadata);
    detail.append(&actions);
    detail.append(&scroll);
    eprintln!("zentty-linux: license-detail id={}", entry.id);
}

fn metadata_row(grid: &gtk::Grid, row: i32, title: &str, value: &str, widget_name: &str) {
    let title = gtk::Label::new(Some(title));
    title.set_xalign(1.0);
    title.add_css_class("dim-label");
    let value = gtk::Label::new(Some(value));
    value.set_xalign(0.0);
    value.set_selectable(true);
    value.add_css_class("monospace");
    value.set_widget_name(widget_name);
    grid.attach(&title, 0, row, 1, 1);
    grid.attach(&value, 1, row, 1, 1);
}

fn action_button(label: &str, widget_name: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_widget_name(widget_name);
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    let focus_name = widget_name.to_owned();
    button.connect_has_focus_notify(move |button| {
        if button.has_focus() {
            eprintln!("zentty-linux: about-focus widget={focus_name}");
        }
    });
    button
}

fn application_icon() -> gtk::Image {
    std::env::current_exe()
        .ok()
        .and_then(|executable| default_icon_paths(&executable).ok())
        .and_then(|paths| paths.into_iter().find(|path| path.is_file()))
        .map_or_else(gtk::Image::new, gtk::Image::from_file)
}

fn open_reviewed_uri(action: &str, uri: &str) {
    match zentty_linux::platform::open_uri(uri) {
        Ok(()) => eprintln!("zentty-linux: about-link action={action} uri={uri} result=opened"),
        Err(error) => eprintln!(
            "zentty-linux: about-link action={action} uri={uri} result=failed detail={error}"
        ),
    }
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn widget_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn install_styles() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        "#zentty-about-window #licenses-list row { padding: 2px; }\n\
         #zentty-about-window #licenses-list button { padding: 8px; }\n\
         #zentty-about-window #license-full-text { padding: 10px; }\n\
         #zentty-about-window .zentty-about-error { padding: 14px; border-radius: 8px; }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_external_destinations_are_reviewed_https() {
        assert!(DOCS_URL.starts_with("https://"));
        assert!(SOURCE_URL.starts_with("https://"));
        assert_eq!(SOURCE_URL, "https://github.com/TamedTornado/tornadotty");
    }

    #[test]
    fn widget_ids_cannot_escape_the_widget_name_contract() {
        assert_eq!(widget_id("cargo/example/1.2.3"), "cargo-example-1-2-3");
        assert_eq!(widget_id("safe_name"), "safe_name");
    }
}
