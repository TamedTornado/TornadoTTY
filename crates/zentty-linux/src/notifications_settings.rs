use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::NotificationsConfig;

use crate::notification_service::{NotificationService, SOUND_CHOICES};

pub(crate) type ApplyNotifications = Rc<dyn Fn(NotificationsConfig)>;

pub(crate) fn build(initial: NotificationsConfig, apply: &ApplyNotifications) -> gtk::Widget {
    eprintln!(
        "zentty-linux: notification-settings loaded sound={:?}",
        initial.sound_name
    );
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.append(&section_heading());

    let state = Rc::new(RefCell::new(initial));
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.append(&notification_row(&state));
    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    card.append(&sound_row(&state, apply));
    content.append(&card);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&content));
    scroll.upcast()
}

fn section_heading() -> gtk::Widget {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = gtk::Label::new(Some("Notifications"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Desktop alerts and notification sound"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    labels.append(&title);
    labels.append(&subtitle);
    labels.upcast()
}

fn notification_row(state: &Rc<RefCell<NotificationsConfig>>) -> gtk::Widget {
    let available = NotificationService::is_available();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let labels = row_labels(
        "Desktop Notifications",
        if available {
            "Desktop notifications are available through the freedesktop notification service."
        } else {
            "No freedesktop notification service is currently available."
        },
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let status = gtk::Label::new(Some(if available {
        "Available"
    } else {
        "Unavailable"
    }));
    status.set_widget_name("notification-status");
    status.add_css_class(if available { "success" } else { "error" });
    row.append(&status);

    let open = gtk::Button::with_mnemonic("_Open Settings");
    open.set_widget_name("notification-open-settings");
    open.connect_clicked(|_| match NotificationService::open_settings() {
        Ok(()) => eprintln!("zentty-linux: notification-settings action=open result=launched"),
        Err(error) => eprintln!(
            "zentty-linux: notification-settings action=open result=unavailable detail={error}"
        ),
    });
    row.append(&open);

    let send = gtk::Button::with_mnemonic("Send _Test");
    send.set_widget_name("notification-send-test");
    send.set_sensitive(available);
    let send_focus = gtk::EventControllerFocus::new();
    send_focus.connect_enter(|_| {
        eprintln!("zentty-linux: notification-settings focus=send-test");
    });
    send.add_controller(send_focus);
    let state_for_send = Rc::clone(state);
    send.connect_clicked(move |_| {
        match NotificationService::send(
            "Zentty",
            "This is a test notification.",
            &state_for_send.borrow(),
        ) {
            Ok(id) => eprintln!(
                "zentty-linux: notification-settings action=send-test result=sent id={id}"
            ),
            Err(error) => eprintln!(
                "zentty-linux: notification-settings action=send-test result=error detail={error}"
            ),
        }
    });
    row.append(&send);
    row.upcast()
}

fn sound_row(state: &Rc<RefCell<NotificationsConfig>>, apply: &ApplyNotifications) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let labels = row_labels(
        "_Notification Sound",
        "Freedesktop sound-theme name played when a notification arrives.",
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let sounds = gtk::DropDown::from_strings(
        &SOUND_CHOICES
            .iter()
            .map(|(_, label)| *label)
            .collect::<Vec<_>>(),
    );
    sounds.set_widget_name("notification-sound");
    if let Some(title) = labels.first_child().and_downcast::<gtk::Label>() {
        title.set_use_underline(true);
        title.set_mnemonic_widget(Some(&sounds));
    }
    let sound_focus = gtk::EventControllerFocus::new();
    sound_focus.connect_enter(|_| {
        eprintln!("zentty-linux: notification-settings focus=sound");
    });
    sounds.add_controller(sound_focus);
    let selected = SOUND_CHOICES
        .iter()
        .position(|(name, _)| *name == state.borrow().sound_name)
        .unwrap_or(0);
    sounds.set_selected(u32::try_from(selected).unwrap_or(0));
    let state_for_change = Rc::clone(state);
    let apply_for_change = Rc::clone(apply);
    sounds.connect_selected_notify(move |dropdown| {
        let Some((sound_name, _)) = SOUND_CHOICES.get(dropdown.selected() as usize) else {
            return;
        };
        let mut next = state_for_change.borrow().clone();
        next.sound_name = (*sound_name).into();
        next.custom_sound_display_name = None;
        *state_for_change.borrow_mut() = next.clone();
        eprintln!(
            "zentty-linux: notification-settings action=sound value={:?}",
            next.sound_name
        );
        apply_for_change(next);
    });
    row.append(&sounds);

    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play.set_widget_name("notification-sound-preview");
    play.set_tooltip_text(Some("Preview sound"));
    let state_for_preview = Rc::clone(state);
    play.connect_clicked(move |_| match NotificationService::preview_sound(&state_for_preview.borrow()) {
        Ok(()) => eprintln!("zentty-linux: notification-settings action=preview result=played"),
        Err(error) => eprintln!(
            "zentty-linux: notification-settings action=preview result=unavailable detail={error}"
        ),
    });
    row.append(&play);
    row.upcast()
}

fn row_labels(title: &str, subtitle: &str) -> gtk::Box {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let title = gtk::Label::new(Some(title));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    labels.append(&title);
    labels.append(&subtitle);
    labels
}

fn set_row_margins(row: &gtk::Box) {
    row.set_margin_top(14);
    row.set_margin_bottom(14);
    row.set_margin_start(16);
    row.set_margin_end(16);
}
