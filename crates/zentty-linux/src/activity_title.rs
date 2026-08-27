use gtk::prelude::*;

const STABLE_CHILD: &str = "stable";
const ACTIVITY_CHILD: &str = "activity";

pub(crate) fn widget(name: &str, text: &str) -> gtk::Stack {
    let stack = gtk::Stack::new();
    stack.set_widget_name(&format!("{name}-presentation"));
    stack.set_hexpand(true);

    let stable = gtk::Label::new(Some(text));
    stable.set_widget_name(name);
    stable.set_xalign(0.0);
    stable.set_hexpand(true);
    stable.set_ellipsize(gtk::pango::EllipsizeMode::End);
    stack.add_named(&stable, Some(STABLE_CHILD));

    let activity = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let prefix = activity_label(&format!("{name}-activity-prefix"));
    let spinner = activity_label(&format!("{name}-activity-spinner"));
    spinner.set_width_chars(1);
    spinner.set_max_width_chars(1);
    let suffix = activity_label(&format!("{name}-activity-suffix"));
    suffix.set_hexpand(true);
    suffix.set_ellipsize(gtk::pango::EllipsizeMode::End);
    activity.append(&prefix);
    activity.append(&spinner);
    activity.append(&suffix);
    stack.add_named(&activity, Some(ACTIVITY_CHILD));
    stack.set_visible_child_name(STABLE_CHILD);
    stack
}

pub(crate) fn show_stable(root: &gtk::Widget, name: &str, text: &str) -> bool {
    let Some(stack) = find_named_widget(root, &format!("{name}-presentation"))
        .and_then(|widget| widget.downcast::<gtk::Stack>().ok())
    else {
        return false;
    };
    let Some(label) = find_named_label(stack.upcast_ref(), name) else {
        return false;
    };
    set_text_if_changed(&label, text);
    show_child_if_changed(&stack, STABLE_CHILD);
    true
}

pub(crate) fn show_activity(root: &gtk::Widget, name: &str, text: &str) -> bool {
    let Some(range) = zentty_core::codex_activity_spinner_range(text) else {
        return show_stable(root, name, text);
    };
    let Some(stack) = find_named_widget(root, &format!("{name}-presentation"))
        .and_then(|widget| widget.downcast::<gtk::Stack>().ok())
    else {
        return false;
    };
    let Some(prefix) = find_named_label(stack.upcast_ref(), &format!("{name}-activity-prefix"))
    else {
        return false;
    };
    let Some(spinner) = find_named_label(stack.upcast_ref(), &format!("{name}-activity-spinner"))
    else {
        return false;
    };
    let Some(suffix) = find_named_label(stack.upcast_ref(), &format!("{name}-activity-suffix"))
    else {
        return false;
    };
    set_text_if_changed(&prefix, &text[..range.start]);
    set_text_if_changed(&spinner, &text[range.clone()]);
    set_text_if_changed(&suffix, &text[range.end..]);
    show_child_if_changed(&stack, ACTIVITY_CHILD);
    true
}

fn activity_label(name: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_widget_name(name);
    label.set_xalign(0.0);
    label
}

fn set_text_if_changed(label: &gtk::Label, text: &str) {
    if label.text().as_str() != text {
        label.set_text(text);
    }
}

fn show_child_if_changed(stack: &gtk::Stack, name: &str) {
    if stack.visible_child_name().as_deref() != Some(name) {
        stack.set_visible_child_name(name);
    }
}

fn find_named_label(root: &gtk::Widget, name: &str) -> Option<gtk::Label> {
    find_named_widget(root, name)?.downcast::<gtk::Label>().ok()
}

fn find_named_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_named_widget(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{show_activity, show_stable, widget};
    use gtk::prelude::*;

    #[test]
    fn activity_changes_only_the_fixed_width_spinner_label() {
        gtk::init().expect("GTK");
        let title = widget("pane-title", "Working · Bro | Tasks 6/7");
        assert!(show_activity(
            title.upcast_ref(),
            "pane-title",
            "Working ⠧ Bro | Tasks 6/7"
        ));
        let spinner = super::find_named_label(title.upcast_ref(), "pane-title-activity-spinner")
            .expect("spinner");
        let prefix = super::find_named_label(title.upcast_ref(), "pane-title-activity-prefix")
            .expect("prefix");
        let suffix = super::find_named_label(title.upcast_ref(), "pane-title-activity-suffix")
            .expect("suffix");
        assert_eq!(prefix.text(), "Working ");
        assert_eq!(spinner.text(), "⠧");
        assert_eq!(spinner.width_chars(), 1);
        assert_eq!(suffix.text(), " Bro | Tasks 6/7");

        assert!(show_stable(title.upcast_ref(), "pane-title", "Ready · Bro"));
        let stable = super::find_named_label(title.upcast_ref(), "pane-title").expect("stable");
        assert_eq!(stable.text(), "Ready · Bro");
        assert_eq!(title.visible_child_name().as_deref(), Some("stable"));
    }
}
