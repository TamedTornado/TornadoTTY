use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{ErrorReportingConfig, UpdateChannel, UpdatesConfig};

pub(crate) type ApplyUpdates = Rc<dyn Fn(UpdatesConfig)>;

const CHANNELS: &[(UpdateChannel, &str)] = &[
    (UpdateChannel::Stable, "Stable"),
    (UpdateChannel::Beta, "Beta"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PageModel {
    updates: UpdatesConfig,
    error_reporting: ErrorReportingConfig,
    error_reporting_available: bool,
}

impl PageModel {
    fn error_reporting_subtitle(self) -> &'static str {
        if self.error_reporting_available {
            "Send anonymous crash reports to help improve Zentty. Privacy-first by design."
        } else {
            "Error reporting is unavailable in this build."
        }
    }
}

pub(crate) fn build(
    updates: UpdatesConfig,
    error_reporting: ErrorReportingConfig,
    apply: &ApplyUpdates,
) -> gtk::Widget {
    let model = PageModel {
        updates,
        error_reporting,
        // Linux has no reviewed crash transport yet. This must remain false
        // until issue #23 supplies the complete consent/redaction boundary.
        error_reporting_available: false,
    };
    eprintln!(
        "zentty-linux: updates-privacy-settings loaded channel={} error-reporting-available={}",
        model.updates.channel.config_value(),
        model.error_reporting_available
    );

    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.append(&section_heading());

    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.append(&update_channel_row(model, apply));
    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    card.append(&error_reporting_row(model));
    content.append(&card);

    let boundary = gtk::Label::new(Some(
        "Selecting a channel does not install updates. Linux package discovery and trusted upgrades remain explicit release work.",
    ));
    boundary.set_widget_name("updates-platform-boundary");
    boundary.set_halign(gtk::Align::Start);
    boundary.set_wrap(true);
    boundary.add_css_class("dim-label");
    content.append(&boundary);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&content));
    scroll.upcast()
}

fn section_heading() -> gtk::Widget {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = gtk::Label::new(Some("Updates & Privacy"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Update channel and crash reporting"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    labels.append(&title);
    labels.append(&subtitle);
    labels.upcast()
}

fn update_channel_row(model: PageModel, apply: &ApplyUpdates) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let labels = row_labels(
        "_Update Channel",
        "Stable gets regular releases. Beta includes prerelease updates.",
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let choices = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    choices.add_css_class("linked");
    choices.set_widget_name("updates-channel");
    let stable = gtk::ToggleButton::with_mnemonic(&format!("_{}", CHANNELS[0].1));
    stable.set_widget_name("updates-channel-stable");
    let beta = gtk::ToggleButton::with_mnemonic(&format!("_{}", CHANNELS[1].1));
    beta.set_widget_name("updates-channel-beta");
    beta.set_group(Some(&stable));
    stable.set_active(model.updates.channel == UpdateChannel::Stable);
    beta.set_active(model.updates.channel == UpdateChannel::Beta);
    if let Some(title) = labels.first_child().and_downcast::<gtk::Label>() {
        title.set_use_underline(true);
        title.set_mnemonic_widget(Some(&stable));
    }
    let current = Rc::new(RefCell::new(model.updates));
    for (button, channel) in [
        (stable.clone(), UpdateChannel::Stable),
        (beta.clone(), UpdateChannel::Beta),
    ] {
        let current = Rc::clone(&current);
        let apply = Rc::clone(apply);
        button.connect_toggled(move |button| {
            if !button.is_active() || current.borrow().channel == channel {
                return;
            }
            let next = UpdatesConfig { channel };
            *current.borrow_mut() = next;
            eprintln!(
                "zentty-linux: updates-privacy-settings action=channel value={}",
                channel.config_value()
            );
            apply(next);
        });
    }
    choices.append(&stable);
    choices.append(&beta);
    row.append(&choices);
    row.upcast()
}

fn error_reporting_row(model: PageModel) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let labels = row_labels("Error Reporting", model.error_reporting_subtitle());
    labels.set_hexpand(true);
    row.append(&labels);

    let status = gtk::Label::new(Some(if model.error_reporting_available {
        ""
    } else {
        "Unavailable"
    }));
    status.set_widget_name("error-reporting-status");
    status.add_css_class("dim-label");
    row.append(&status);

    let toggle = gtk::Switch::new();
    toggle.set_widget_name("error-reporting-enabled");
    toggle.set_active(model.error_reporting.enabled);
    toggle.set_sensitive(model.error_reporting_available);
    toggle.set_tooltip_text(Some(model.error_reporting_subtitle()));
    row.append(&toggle);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_channel_order_and_tokens_are_stable() {
        assert_eq!(
            CHANNELS,
            &[
                (UpdateChannel::Stable, "Stable"),
                (UpdateChannel::Beta, "Beta")
            ]
        );
        assert_eq!(CHANNELS[0].0.config_value(), "stable");
        assert_eq!(CHANNELS[1].0.config_value(), "beta");
    }

    #[test]
    fn unavailable_build_cannot_claim_error_reporting() {
        let model = PageModel {
            updates: UpdatesConfig::default(),
            error_reporting: ErrorReportingConfig::default(),
            error_reporting_available: false,
        };
        assert_eq!(
            model.error_reporting_subtitle(),
            "Error reporting is unavailable in this build."
        );
    }
}
