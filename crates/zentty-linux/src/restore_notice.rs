use gtk::prelude::*;

#[derive(Clone)]
pub(crate) struct RestoreNotice {
    root: gtk::Revealer,
    message: gtk::Label,
}

impl RestoreNotice {
    pub(crate) fn new() -> Self {
        let root = gtk::Revealer::new();
        root.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Start);
        root.set_margin_top(46);
        let banner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        banner.add_css_class("restore-notice");
        banner.append(&gtk::Image::from_icon_name("dialog-warning-symbolic"));
        let message = gtk::Label::new(None);
        message.set_max_width_chars(88);
        message.set_wrap(true);
        message.set_xalign(0.0);
        message.set_hexpand(true);
        banner.append(&message);
        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.set_has_frame(false);
        dismiss.set_tooltip_text(Some("Dismiss restore warning"));
        banner.append(&dismiss);
        root.set_child(Some(&banner));
        root.set_accessible_role(gtk::AccessibleRole::Alert);
        let revealer = root.clone();
        dismiss.connect_clicked(move |_| revealer.set_reveal_child(false));
        Self { root, message }
    }

    pub(crate) fn widget(&self) -> &gtk::Revealer {
        &self.root
    }

    pub(crate) fn show(&self, message: &str) {
        self.message.set_label(message);
        self.root
            .update_property(&[gtk::accessible::Property::Label(message)]);
        self.root.set_reveal_child(true);
    }
}
