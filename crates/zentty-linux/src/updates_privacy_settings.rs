use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{ErrorReportingConfig, UpdateChannel, UpdatesConfig};

pub(crate) type ApplyUpdates = Rc<dyn Fn(UpdatesConfig)>;
pub(crate) type ApplyErrorReporting = Rc<dyn Fn(ErrorReportingConfig) -> Result<(), String>>;

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
    fn error_reporting_subtitle() -> &'static str {
        "Capture a bounded redacted crash report locally after restart. Nothing is sent automatically."
    }
}

pub(crate) fn build(
    parent: &gtk::Window,
    updates: UpdatesConfig,
    error_reporting: ErrorReportingConfig,
    apply_updates: &ApplyUpdates,
    apply_error_reporting: &ApplyErrorReporting,
) -> gtk::Widget {
    let model = PageModel {
        updates,
        error_reporting,
        error_reporting_available: true,
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
    card.append(&update_channel_row(model, apply_updates));
    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    card.append(&error_reporting_row(parent, model, apply_error_reporting));
    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    card.append(&diagnostic_reports_row(parent));
    content.append(&card);

    let boundary = gtk::Label::new(Some(
        "Updates never install themselves. Diagnostics remain local unless you review a report and separately confirm an available submission.",
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

fn error_reporting_row(
    parent: &gtk::Window,
    model: PageModel,
    apply: &ApplyErrorReporting,
) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let labels = row_labels(
        "_Local Crash Capture",
        PageModel::error_reporting_subtitle(),
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let status = gtk::Label::new(Some(if model.error_reporting.enabled {
        "Enabled after restart"
    } else {
        "Disabled"
    }));
    status.set_widget_name("error-reporting-status");
    status.add_css_class("dim-label");
    row.append(&status);

    let toggle = gtk::Switch::new();
    toggle.set_widget_name("error-reporting-enabled");
    toggle.set_active(model.error_reporting.enabled);
    toggle.set_sensitive(model.error_reporting_available);
    toggle.set_tooltip_text(Some(PageModel::error_reporting_subtitle()));
    if let Some(title) = labels.first_child().and_downcast::<gtk::Label>() {
        title.set_use_underline(true);
        title.set_mnemonic_widget(Some(&toggle));
    }
    let applying = Rc::new(Cell::new(false));
    let apply_toggle = toggle.clone();
    let apply = Rc::clone(apply);
    let status_label = status.clone();
    let parent = parent.clone();
    toggle.connect_active_notify(move |toggle| {
        if applying.replace(true) {
            return;
        }
        let next = ErrorReportingConfig {
            enabled: toggle.is_active(),
        };
        match apply(next) {
            Ok(()) => {
                status_label.set_text(if next.enabled {
                    "Enabled after restart"
                } else {
                    "Disabled after restart"
                });
                show_notice(
                    &parent,
                    if next.enabled {
                        "Local crash capture will be enabled after restarting Zentty. Reports remain on this device until you review and explicitly submit them."
                    } else {
                        "Automatic local crash capture will be disabled after restarting Zentty. Existing local reports are not sent or removed."
                    },
                );
            }
            Err(error) => {
                apply_toggle.set_active(!next.enabled);
                show_notice(&parent, &format!("Could not save the privacy setting: {error}"));
            }
        }
        applying.set(false);
    });
    row.append(&toggle);
    row.upcast()
}

fn diagnostic_reports_row(parent: &gtk::Window) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    set_row_margins(&row);
    let reports = crate::diagnostics_runtime::list_reports().unwrap_or_default();
    let labels = row_labels(
        "Local Support Reports",
        crate::diagnostics_runtime::transport_description(),
    );
    labels.set_hexpand(true);
    row.append(&labels);

    let status = gtk::Label::new(Some(&report_count_label(reports.len())));
    status.set_widget_name("diagnostic-report-status");
    status.add_css_class("dim-label");
    row.append(&status);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let create = gtk::Button::with_mnemonic("_Create & Review…");
    create.set_widget_name("diagnostic-report-create");
    let review = gtk::Button::with_mnemonic("_Review Latest…");
    review.set_widget_name("diagnostic-report-review");
    review.set_sensitive(!reports.is_empty());
    let clear = gtk::Button::with_mnemonic("_Delete All");
    clear.set_widget_name("diagnostic-report-clear");
    clear.set_sensitive(!reports.is_empty());
    actions.append(&create);
    actions.append(&review);
    actions.append(&clear);
    row.append(&actions);

    let parent_for_create = parent.clone();
    let status_for_create = status.clone();
    let review_for_create = review.clone();
    let clear_for_create = clear.clone();
    create.connect_clicked(
        move |_| match crate::diagnostics_runtime::create_manual_report() {
            Ok(report) => {
                status_for_create.set_text(&report_count_label(
                    crate::diagnostics_runtime::list_reports().map_or(1, |reports| reports.len()),
                ));
                review_for_create.set_sensitive(true);
                clear_for_create.set_sensitive(true);
                show_report_review(&parent_for_create, &report);
            }
            Err(error) => show_notice(
                &parent_for_create,
                &format!("Could not create a local report: {error}"),
            ),
        },
    );

    let parent_for_review = parent.clone();
    review.connect_clicked(move |_| match crate::diagnostics_runtime::list_reports() {
        Ok(reports) if !reports.is_empty() => {
            show_report_review(&parent_for_review, &reports[0]);
        }
        Ok(_) => show_notice(&parent_for_review, "There are no local reports to review."),
        Err(error) => show_notice(
            &parent_for_review,
            &format!("Could not read local reports: {error}"),
        ),
    });

    let parent_for_clear = parent.clone();
    let clear_for_clear = clear.clone();
    clear.connect_clicked(move |_| match crate::diagnostics_runtime::clear_reports() {
        Ok(count) => {
            status.set_text(&report_count_label(0));
            review.set_sensitive(false);
            clear_for_clear.set_sensitive(false);
            show_notice(
                &parent_for_clear,
                &format!("Cleared {count} local report(s)."),
            );
        }
        Err(error) => show_notice(
            &parent_for_clear,
            &format!("Could not clear local reports: {error}"),
        ),
    });
    row.upcast()
}

fn show_report_review(parent: &gtk::Window, report: &zentty_core::DiagnosticReport) {
    let report = match crate::diagnostics_runtime::mark_pending_review(&report.report_id) {
        Ok(report) => report,
        Err(error) => {
            show_notice(parent, &format!("Could not begin local review: {error}"));
            return;
        }
    };
    let (window, close, submit) = build_report_review_window(parent, &report);
    connect_report_review_actions(&window, &close, &submit, &report.report_id);
    window.present();
    eprintln!(
        "zentty-linux: diagnostics report={} state=pending-review",
        report.report_id
    );
}

fn build_report_review_window(
    parent: &gtk::Window,
    report: &zentty_core::DiagnosticReport,
) -> (gtk::Window, gtk::Button, gtk::Button) {
    let window = gtk::Window::builder()
        .title("Review Local Support Report")
        .transient_for(parent)
        .modal(true)
        .default_width(760)
        .default_height(560)
        .build();
    window.set_widget_name("diagnostic-report-review-window");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let explanation = gtk::Label::new(Some(
        "Review the complete redacted payload below. Nothing has been sent.",
    ));
    explanation.set_halign(gtk::Align::Start);
    explanation.set_wrap(true);
    root.append(&explanation);
    let payload = serde_json::to_string_pretty(&report)
        .unwrap_or_else(|error| format!("Could not render report: {error}"));
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(&payload);
    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    view.set_widget_name("diagnostic-report-payload");
    let scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();
    root.append(&scroll);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let close = gtk::Button::with_mnemonic("_Keep Local");
    let submit = gtk::Button::with_mnemonic("_Submit Reviewed Report…");
    submit.set_sensitive(crate::diagnostics_runtime::submission_available());
    submit.set_tooltip_text(Some(crate::diagnostics_runtime::transport_description()));
    actions.append(&close);
    actions.append(&submit);
    root.append(&actions);
    window.set_child(Some(&root));
    (window, close, submit)
}

fn connect_report_review_actions(
    window: &gtk::Window,
    close: &gtk::Button,
    submit: &gtk::Button,
    report_id: &str,
) {
    let submitted = Rc::new(Cell::new(false));
    let report_id = report_id.to_owned();
    let review_window = window.clone();
    close.connect_clicked(move |_| review_window.close());
    let report_id_for_close = report_id.clone();
    let submitted_for_close = Rc::clone(&submitted);
    window.connect_close_request(move |_| {
        if !submitted_for_close.get() {
            let _ = crate::diagnostics_runtime::mark_local(&report_id_for_close);
        }
        gtk::glib::Propagation::Proceed
    });

    let confirmation_parent = window.clone();
    submit.connect_clicked(move |_| {
        eprintln!("zentty-linux: diagnostics submission=confirmation-requested");
        let confirmation = gtk::Window::builder()
            .title("Confirm Diagnostic Submission")
            .modal(true)
            .transient_for(&confirmation_parent)
            .default_width(520)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        let heading = gtk::Label::new(Some("Submit this reviewed report?"));
        heading.add_css_class("title-2");
        heading.set_halign(gtk::Align::Start);
        content.append(&heading);
        let detail = gtk::Label::new(Some(
            "Only the payload currently shown will be sent to the configured Zentty support endpoint.",
        ));
        detail.set_halign(gtk::Align::Start);
        detail.set_wrap(true);
        content.append(&detail);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_mnemonic("_Cancel");
        let confirm = gtk::Button::with_mnemonic("_Submit");
        confirm.add_css_class("destructive-action");
        actions.append(&cancel);
        actions.append(&confirm);
        content.append(&actions);
        confirmation.set_child(Some(&content));
        confirmation.set_default_widget(Some(&cancel));

        let confirmation_for_cancel = confirmation.clone();
        cancel.connect_clicked(move |_| {
            eprintln!("zentty-linux: diagnostics submission=cancelled");
            confirmation_for_cancel.close();
        });
        let report_id = report_id.clone();
        let submitted = Rc::clone(&submitted);
        let review_window = confirmation_parent.clone();
        let confirmation_for_submit = confirmation.clone();
        confirm.connect_clicked(move |_| {
            begin_reviewed_report_submission(
                &report_id,
                &submitted,
                &confirmation_for_submit,
                &review_window,
            );
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let cancel_for_keys = cancel.clone();
        let confirm_for_keys = confirm.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::ALT_MASK)
                && matches!(key, gtk::gdk::Key::s | gtk::gdk::Key::S)
            {
                confirm_for_keys.emit_clicked();
                gtk::glib::Propagation::Stop
            } else if key == gtk::gdk::Key::Escape
                || (modifiers.contains(gtk::gdk::ModifierType::ALT_MASK)
                    && matches!(key, gtk::gdk::Key::c | gtk::gdk::Key::C))
            {
                cancel_for_keys.emit_clicked();
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        confirmation.add_controller(keys);
        confirmation.connect_is_active_notify(|window| {
            if window.is_active() {
                eprintln!("zentty-linux: diagnostics submission-confirmation=active");
            }
        });
        confirmation.present();
    });
}

fn begin_reviewed_report_submission(
    report_id: &str,
    submitted: &Cell<bool>,
    confirmation: &gtk::Window,
    review_window: &gtk::Window,
) {
    if submitted.replace(true) {
        return;
    }
    let report_id = report_id.to_owned();
    confirmation.close();
    review_window.close();
    std::thread::spawn(move || {
        match crate::diagnostics_runtime::submit_reviewed_report(&report_id) {
            Ok(_) => {
                eprintln!("zentty-linux: diagnostics report={report_id} state=sent explicit=true");
            }
            Err(error) => {
                eprintln!(
                    "zentty-linux: diagnostics report={report_id} state=failed detail={error}"
                );
            }
        }
    });
}

fn show_notice(parent: &gtk::Window, detail: &str) {
    gtk::AlertDialog::builder()
        .modal(true)
        .message("Zentty Diagnostics")
        .detail(detail)
        .buttons(["OK"])
        .build()
        .show(Some(parent));
}

fn report_count_label(count: usize) -> String {
    format!("{count} local")
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
    fn local_crash_capture_copy_never_claims_automatic_transmission() {
        assert_eq!(
            PageModel::error_reporting_subtitle(),
            "Capture a bounded redacted crash report locally after restart. Nothing is sent automatically."
        );
    }
}
