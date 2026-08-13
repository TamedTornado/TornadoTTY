use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk::prelude::*;
use zentty_core::NotificationsConfig;

use crate::custom_sound_store::CustomSoundStore;
use crate::notification_service::{NotificationService, SOUND_CHOICES};

pub(crate) type ApplyNotifications = Rc<dyn Fn(NotificationsConfig) -> Result<(), String>>;

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
        "Choose a sound-theme event or import a private custom audio file up to 30 seconds.",
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let sounds = gtk::DropDown::new(None::<gtk::gio::ListModel>, None::<gtk::Expression>);
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
    let sound_names = Rc::new(RefCell::new(Vec::<String>::new()));
    let rebuilding = Rc::new(Cell::new(false));
    rebuild_sound_choices(&sounds, &sound_names, &state.borrow(), &rebuilding);
    let status = gtk::Label::new(None);
    status.set_widget_name("notification-sound-status");
    status.set_halign(gtk::Align::End);
    status.add_css_class("dim-label");
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.append(&sounds);

    connect_sound_selection(&sounds, state, apply, &sound_names, &rebuilding, &status);

    let play = gtk::Button::new();
    let play_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    play_content.append(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
    let play_label = gtk::Label::with_mnemonic("_Preview");
    play_label.set_mnemonic_widget(Some(&play));
    play_content.append(&play_label);
    play.set_child(Some(&play_content));
    play.set_widget_name("notification-sound-preview");
    play.set_tooltip_text(Some("Preview sound"));
    let state_for_preview = Rc::clone(state);
    let status_for_preview = status.clone();
    play.connect_clicked(move |_| match NotificationService::preview_sound(&state_for_preview.borrow()) {
        Ok(()) => {
            status_for_preview.set_text("Preview played.");
            eprintln!("zentty-linux: notification-settings action=preview result=played");
        }
        Err(error) => {
            status_for_preview.set_text(&format!("Preview unavailable: {error}"));
            eprintln!(
                "zentty-linux: notification-settings action=preview result=unavailable detail={error}"
            );
        }
    });
    controls.append(&play);

    let import = gtk::Button::with_mnemonic("_Import Audio…");
    import.set_widget_name("notification-sound-import");
    let import_focus = gtk::EventControllerFocus::new();
    import_focus.connect_enter(|_| {
        eprintln!("zentty-linux: notification-settings focus=import-custom");
    });
    import.add_controller(import_focus);
    let state_for_import = Rc::clone(state);
    let apply_for_import = Rc::clone(apply);
    let sounds_for_import = sounds.clone();
    let names_for_import = Rc::clone(&sound_names);
    let rebuilding_for_import = Rc::clone(&rebuilding);
    let status_for_import = status.clone();
    let play_for_import = play.clone();
    let controls_for_import = controls.clone();
    import.connect_clicked(move |button| {
        eprintln!("zentty-linux: notification-settings action=import-custom result=chooser-opened");
        let Some(window) = button.root().and_downcast::<gtk::Window>() else {
            eprintln!(
                "zentty-linux: notification-settings action=import-custom result=error detail=settings window is unavailable"
            );
            return;
        };
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Audio files"));
        filter.add_mime_type("audio/*");
        let chooser = gtk::FileDialog::builder()
            .title("Import Notification Audio")
            .accept_label("Import")
            .modal(true)
            .default_filter(&filter)
            .build();
        let state = Rc::clone(&state_for_import);
        let apply = Rc::clone(&apply_for_import);
        let sounds = sounds_for_import.clone();
        let names = Rc::clone(&names_for_import);
        let rebuilding = Rc::clone(&rebuilding_for_import);
        let status = status_for_import.clone();
        let import = button.clone();
        let play = play_for_import.clone();
        let controls = controls_for_import.clone();
        chooser.open(
            Some(&window),
            None::<&gtk::gio::Cancellable>,
            move |result| {
                let file = match result {
                    Ok(file) => file,
                    Err(error) => {
                        eprintln!(
                            "zentty-linux: notification-settings action=import-custom result=cancelled detail={error}"
                        );
                        return;
                    }
                };
                let Some(path) = file.path() else {
                    status.set_text("The selected audio is not a local file.");
                    return;
                };
                status.set_text("Installing custom audio…");
                import.set_sensitive(false);
                play.set_sensitive(false);
                let (sender, receiver) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let _ = sender.send(CustomSoundStore::prepare(&path));
                });
                let state = Rc::clone(&state);
                let apply = Rc::clone(&apply);
                let sounds = sounds.clone();
                let names = Rc::clone(&names);
                let rebuilding = Rc::clone(&rebuilding);
                let status = status.clone();
                let import = import.clone();
                let play = play.clone();
                let controls = controls.clone();
                gtk::glib::timeout_add_local(Duration::from_millis(20), move || {
                    let result = match receiver.try_recv() {
                        Ok(result) => result,
                        Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            import.set_sensitive(true);
                            play.set_sensitive(true);
                            status.set_text("Custom audio worker stopped unexpectedly.");
                            return gtk::glib::ControlFlow::Break;
                        }
                    };
                    import.set_sensitive(true);
                    play.set_sensitive(true);
                    match result {
                        Ok(prepared) => {
                            let mut next = state.borrow().clone();
                            prepared.internal_name().clone_into(&mut next.sound_name);
                            next.custom_sound_display_name =
                                Some(prepared.display_name().to_owned());
                            match apply(next.clone()) {
                                Ok(()) => match CustomSoundStore::finish(prepared) {
                                    Ok(()) => {
                                        *state.borrow_mut() = next.clone();
                                        rebuild_sound_choices(
                                            &sounds,
                                            &names,
                                            &next,
                                            &rebuilding,
                                        );
                                        match NotificationService::preview_sound(&next) {
                                            Ok(()) => {
                                                status.set_text(
                                                    "Custom audio installed and previewed.",
                                                );
                                                eprintln!(
                                                    "zentty-linux: notification-settings action=import-preview result=played"
                                                );
                                            }
                                            Err(error) => {
                                                status.set_text(&format!(
                                                    "Custom audio installed; preview unavailable: {error}"
                                                ));
                                                eprintln!(
                                                    "zentty-linux: notification-settings action=import-preview result=unavailable detail={error}"
                                                );
                                            }
                                        }
                                        set_named_child_sensitive(
                                            &controls,
                                            "notification-sound-remove",
                                            true,
                                        );
                                        eprintln!(
                                            "zentty-linux: notification-settings action=import-custom result=installed sound={:?}",
                                            next.sound_name
                                        );
                                    }
                                    Err(error) => {
                                        status.set_text(&format!(
                                            "Custom audio saved; cleanup failed: {error}"
                                        ));
                                    }
                                },
                                Err(error) => {
                                    CustomSoundStore::rollback(prepared);
                                    status.set_text(&format!(
                                        "Could not save custom audio: {error}"
                                    ));
                                    eprintln!(
                                        "zentty-linux: notification-settings action=import-custom result=rollback detail={error}"
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            status.set_text(&format!("Could not install custom audio: {error}"));
                            eprintln!(
                                "zentty-linux: notification-settings action=import-custom result=error detail={error}"
                            );
                        }
                    }
                    gtk::glib::ControlFlow::Break
                });
            },
        );
    });
    controls.append(&import);

    let remove = gtk::Button::with_mnemonic("_Remove Custom");
    remove.set_widget_name("notification-sound-remove");
    remove.set_sensitive(CustomSoundStore::is_custom_name(&state.borrow().sound_name));
    let state_for_remove = Rc::clone(state);
    let apply_for_remove = Rc::clone(apply);
    let sounds_for_remove = sounds.clone();
    let names_for_remove = Rc::clone(&sound_names);
    let rebuilding_for_remove = Rc::clone(&rebuilding);
    let status_for_remove = status.clone();
    remove.connect_clicked(move |button| {
        let previous = state_for_remove.borrow().clone();
        if !CustomSoundStore::is_custom_name(&previous.sound_name) {
            return;
        }
        let mut next = previous.clone();
        next.sound_name.clear();
        next.custom_sound_display_name = None;
        match apply_for_remove(next.clone()) {
            Ok(()) => {
                *state_for_remove.borrow_mut() = next.clone();
                rebuild_sound_choices(
                    &sounds_for_remove,
                    &names_for_remove,
                    &next,
                    &rebuilding_for_remove,
                );
                button.set_sensitive(false);
                match CustomSoundStore::prune(None) {
                    Ok(()) => {
                    status_for_remove.set_text("Custom audio removed.");
                    eprintln!(
                        "zentty-linux: notification-settings action=remove-custom result=removed"
                    );
                    }
                    Err(error) => status_for_remove
                        .set_text(&format!("Sound unselected; cleanup failed: {error}")),
                }
            }
            Err(error) => {
                status_for_remove.set_text(&format!("Could not unselect custom audio: {error}"));
            }
        }
    });
    controls.append(&remove);

    let right = gtk::Box::new(gtk::Orientation::Vertical, 6);
    right.set_halign(gtk::Align::End);
    right.append(&controls);
    right.append(&status);
    row.append(&right);
    row.upcast()
}

fn connect_sound_selection(
    sounds: &gtk::DropDown,
    state: &Rc<RefCell<NotificationsConfig>>,
    apply: &ApplyNotifications,
    names: &Rc<RefCell<Vec<String>>>,
    rebuilding: &Rc<Cell<bool>>,
    status: &gtk::Label,
) {
    let state = Rc::clone(state);
    let apply = Rc::clone(apply);
    let names = Rc::clone(names);
    let rebuilding = Rc::clone(rebuilding);
    let status = status.clone();
    sounds.connect_selected_notify(move |dropdown| {
        if rebuilding.get() {
            return;
        }
        let Some(sound_name) = names
            .borrow()
            .get(dropdown.selected() as usize)
            .cloned()
        else {
            return;
        };
        let previous = state.borrow().clone();
        if sound_name == previous.sound_name {
            return;
        }
        let mut next = state.borrow().clone();
        next.sound_name.clone_from(&sound_name);
        next.custom_sound_display_name = None;
        match apply(next.clone()) {
            Ok(()) => {
                *state.borrow_mut() = next;
                if CustomSoundStore::is_custom_name(&previous.sound_name)
                    && let Err(error) = CustomSoundStore::prune(None)
                {
                    status.set_text(&format!("Saved; cleanup failed: {error}"));
                } else {
                    status.set_text("Sound saved.");
                }
                set_named_sibling_sensitive(dropdown, "notification-sound-remove", false);
                eprintln!(
                    "zentty-linux: notification-settings action=sound result=applied value={sound_name:?}"
                );
            }
            Err(error) => {
                status.set_text(&format!("Could not save sound: {error}"));
                rebuild_sound_choices(dropdown, &names, &previous, &rebuilding);
                eprintln!(
                    "zentty-linux: notification-settings action=sound result=error detail={error}"
                );
            }
        }
    });
}

fn set_named_sibling_sensitive(widget: &impl IsA<gtk::Widget>, name: &str, sensitive: bool) {
    if let Some(parent) = widget.parent() {
        set_named_child_sensitive(&parent, name, sensitive);
    }
}

fn set_named_child_sensitive(parent: &impl IsA<gtk::Widget>, name: &str, sensitive: bool) {
    let mut child = parent.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == name {
            widget.set_sensitive(sensitive);
            return;
        }
        child = widget.next_sibling();
    }
}

fn rebuild_sound_choices(
    dropdown: &gtk::DropDown,
    names: &Rc<RefCell<Vec<String>>>,
    config: &NotificationsConfig,
    rebuilding: &Rc<Cell<bool>>,
) {
    let mut entries = SOUND_CHOICES
        .iter()
        .map(|(name, label)| ((*name).to_owned(), (*label).to_owned()))
        .collect::<Vec<_>>();
    if CustomSoundStore::is_custom_name(&config.sound_name) {
        entries.push((
            config.sound_name.clone(),
            format!(
                "Custom: {}",
                config
                    .custom_sound_display_name
                    .as_deref()
                    .unwrap_or("Custom audio")
            ),
        ));
    }
    let selected = entries
        .iter()
        .position(|(name, _)| name == &config.sound_name)
        .unwrap_or(0);
    let labels = entries
        .iter()
        .map(|(_, label)| label.as_str())
        .collect::<Vec<_>>();
    rebuilding.set(true);
    dropdown.set_model(Some(&gtk::StringList::new(&labels)));
    *names.borrow_mut() = entries.into_iter().map(|(name, _)| name).collect();
    dropdown.set_selected(u32::try_from(selected).unwrap_or(0));
    rebuilding.set(false);
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
