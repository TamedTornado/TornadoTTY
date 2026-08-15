use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use zentty_core::{AttentionItem, AttentionTarget};

pub(crate) fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".attention-inbox { min-width: 340px; padding: 8px; }\n\
         .attention-inbox-header { margin: 2px 4px 8px 4px; }\n\
         .attention-inbox-row { padding: 8px; border-radius: 6px; }\n\
         .attention-inbox-row.resolved { opacity: 0.62; }\n\
         .attention-inbox-status { font-weight: 700; }\n\
         .attention-inbox-primary { font-size: 0.95em; }\n\
         .attention-inbox-location { opacity: 0.72; font-size: 0.88em; }\n\
         .attention-inbox-empty { padding: 32px 20px; opacity: 0.72; }\n\
         .attention-count { font-size: 0.72em; font-weight: 700; padding: 0 4px; border-radius: 8px; background: @accent_bg_color; color: @accent_fg_color; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub(crate) fn button_content(count: usize) -> (gtk::Box, gtk::Label) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    content.append(&gtk::Image::from_icon_name(
        "preferences-system-notifications-symbolic",
    ));
    let badge = gtk::Label::new(None);
    badge.add_css_class("attention-count");
    badge.set_visible(false);
    content.append(&badge);
    update_badge(&badge, count);
    (content, badge)
}

pub(crate) fn update_badge(badge: &gtk::Label, count: usize) {
    badge.set_text(&count.min(99).to_string());
    badge.set_visible(count > 0);
}

pub(crate) fn popover(items: &[AttentionItem]) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
    root.add_css_class("attention-inbox");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.add_css_class("attention-inbox-header");
    let title = gtk::Label::new(Some("Notifications"));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    header.append(&title);
    let clear = gtk::Button::from_icon_name("user-trash-symbolic");
    clear.set_has_frame(false);
    clear.set_tooltip_text(Some("Clear notifications"));
    clear.update_property(&[gtk::accessible::Property::Label("Clear notifications")]);
    clear.set_action_name(Some("workspace.clear-attention"));
    clear.set_sensitive(!items.is_empty());
    header.append(&clear);
    root.append(&header);

    if items.is_empty() {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.add_css_class("attention-inbox-empty");
        empty.set_halign(gtk::Align::Center);
        empty.append(&gtk::Image::from_icon_name(
            "preferences-system-notifications-symbolic",
        ));
        let label = gtk::Label::new(Some("No notifications"));
        label.add_css_class("heading");
        empty.append(&label);
        empty.append(&gtk::Label::new(Some(
            "Agent requests and completed work will appear here.",
        )));
        root.append(&empty);
    } else {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        for item in items {
            list.append(&row(item));
        }
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_max_content_height(460);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        root.append(&scroll);
    }
    popover.set_child(Some(&root));
    popover
}

fn row(item: &AttentionItem) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class("attention-inbox-row");
    if item.is_resolved() {
        row.add_css_class("resolved");
    }

    let activate = gtk::Button::new();
    activate.set_has_frame(false);
    activate.set_hexpand(true);
    activate.set_action_name(Some("workspace.activate-attention"));
    activate.set_action_target_value(Some(&target_variant(&item.target)));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let status = gtk::Label::new(Some(&format!(
        "{} · {}{}",
        item.agent_name,
        item.status_text,
        if item.is_resolved() {
            " · Resolved"
        } else {
            ""
        }
    )));
    status.add_css_class("attention-inbox-status");
    status.set_xalign(0.0);
    let primary = gtk::Label::new(Some(&item.primary_text));
    primary.add_css_class("attention-inbox-primary");
    primary.set_xalign(0.0);
    primary.set_wrap(true);
    primary.set_max_width_chars(42);
    let location = gtk::Label::new(Some(&format!(
        "{} · {}",
        item.location_text
            .as_deref()
            .unwrap_or_else(|| item.target.worklane_id.as_str()),
        relative_timestamp(item.created_at_ms)
    )));
    location.add_css_class("attention-inbox-location");
    location.set_xalign(0.0);
    text.append(&status);
    text.append(&primary);
    text.append(&location);
    activate.set_child(Some(&text));
    row.append(&activate);

    let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
    dismiss.set_has_frame(false);
    dismiss.set_valign(gtk::Align::Center);
    dismiss.set_tooltip_text(Some("Dismiss notification"));
    dismiss.update_property(&[gtk::accessible::Property::Label("Dismiss notification")]);
    dismiss.set_action_name(Some("workspace.dismiss-attention"));
    dismiss.set_action_target_value(Some(&item.id.to_variant()));
    row.append(&dismiss);
    row
}

fn relative_timestamp(created_at_ms: u64) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let elapsed_seconds = now_ms.saturating_sub(created_at_ms) / 1_000;
    match elapsed_seconds {
        0..=59 => "Just now".to_owned(),
        60..=3_599 => format!("{}m ago", elapsed_seconds / 60),
        3_600..=86_399 => format!("{}h ago", elapsed_seconds / 3_600),
        _ => format!("{}d ago", elapsed_seconds / 86_400),
    }
}

fn target_variant(target: &AttentionTarget) -> gtk::glib::Variant {
    (
        target.window_id.clone(),
        target.worklane_id.clone(),
        target.pane_id.clone(),
    )
        .to_variant()
}
