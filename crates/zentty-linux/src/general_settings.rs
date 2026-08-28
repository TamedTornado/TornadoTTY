use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{
    ClipboardConfig, CommandFlattenAggressiveness, ConfirmationsConfig, RestoreConfig,
};

pub(crate) type ApplyGeneral = Rc<dyn Fn(GeneralSettings)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralSettings {
    pub(crate) confirmations: ConfirmationsConfig,
    pub(crate) restore: RestoreConfig,
    pub(crate) clipboard: ClipboardConfig,
}

impl GeneralSettings {
    fn bool_value(self, id: &str) -> Option<bool> {
        Some(match id {
            "confirm-close-pane" => self.confirmations.confirm_before_closing_pane,
            "confirm-close-window" => self.confirmations.confirm_before_closing_window,
            "confirm-quit" => self.confirmations.confirm_before_quitting,
            "restore-workspace" => self.restore.restore_workspace_on_launch,
            "start-restored-background" => self.restore.start_restored_sessions_in_background,
            "always-clean-copies" => self.clipboard.always_clean_copies,
            "flatten-multiline" => self.clipboard.clean_options.flatten_multi_line_commands,
            "preserve-blank-lines" => {
                self.clipboard
                    .clean_options
                    .preserve_blank_lines_when_flattening
            }
            "remove-box-drawing" => self.clipboard.clean_options.remove_box_drawing,
            "flatten-slash-commands" => {
                self.clipboard
                    .clean_options
                    .flatten_slash_command_selections
            }
            "strip-url-tracking" => self.clipboard.clean_options.strip_url_tracking_parameters,
            "quote-paths" => self.clipboard.clean_options.quote_paths_with_spaces,
            "show-copy-markdown" => self.clipboard.show_copy_markdown_command,
            _ => return None,
        })
    }

    fn set_bool(&mut self, id: &str, value: bool) -> bool {
        match id {
            "confirm-close-pane" => self.confirmations.confirm_before_closing_pane = value,
            "confirm-close-window" => self.confirmations.confirm_before_closing_window = value,
            "confirm-quit" => self.confirmations.confirm_before_quitting = value,
            "restore-workspace" => self.restore.restore_workspace_on_launch = value,
            "start-restored-background" => {
                self.restore.start_restored_sessions_in_background = value;
            }
            "always-clean-copies" => self.clipboard.always_clean_copies = value,
            "flatten-multiline" => {
                self.clipboard.clean_options.flatten_multi_line_commands = value;
            }
            "preserve-blank-lines" => {
                self.clipboard
                    .clean_options
                    .preserve_blank_lines_when_flattening = value;
            }
            "remove-box-drawing" => self.clipboard.clean_options.remove_box_drawing = value,
            "flatten-slash-commands" => {
                self.clipboard
                    .clean_options
                    .flatten_slash_command_selections = value;
            }
            "strip-url-tracking" => {
                self.clipboard.clean_options.strip_url_tracking_parameters = value;
            }
            "quote-paths" => self.clipboard.clean_options.quote_paths_with_spaces = value,
            "show-copy-markdown" => self.clipboard.show_copy_markdown_command = value,
            _ => return false,
        }
        true
    }

    fn update_bool(&mut self, id: &str, value: bool) -> Option<Self> {
        self.set_bool(id, value).then_some(*self)
    }
}

fn selected_for_aggressiveness(value: CommandFlattenAggressiveness) -> u32 {
    match value {
        CommandFlattenAggressiveness::Low => 0,
        CommandFlattenAggressiveness::Normal => 1,
        CommandFlattenAggressiveness::High => 2,
    }
}

fn aggressiveness_for_selected(selected: u32) -> Option<CommandFlattenAggressiveness> {
    match selected {
        0 => Some(CommandFlattenAggressiveness::Low),
        1 => Some(CommandFlattenAggressiveness::Normal),
        2 => Some(CommandFlattenAggressiveness::High),
        _ => None,
    }
}

struct BoolSpec {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
}

const LIFECYCLE: &[BoolSpec] = &[
    BoolSpec {
        id: "confirm-close-pane",
        title: "Confirm before closing",
        subtitle: "Show a confirmation dialog when closing a pane.",
    },
    BoolSpec {
        id: "confirm-close-window",
        title: "Confirm before closing window",
        subtitle: "Show a confirmation dialog when closing a window with running processes.",
    },
    BoolSpec {
        id: "confirm-quit",
        title: "Confirm before quitting",
        subtitle: "Show a confirmation dialog when quitting Zentty.",
    },
    BoolSpec {
        id: "restore-workspace",
        title: "Restore worklanes on next launch",
        subtitle: "Reopen windows, pane layout, and saved working directories after quitting.",
    },
    BoolSpec {
        id: "start-restored-background",
        title: "Start restored sessions in background",
        subtitle: "Initialize panes in every restored worklane at launch, even before you visit them.",
    },
];

const CLIPBOARD: &[BoolSpec] = &[
    BoolSpec {
        id: "always-clean-copies",
        title: "Always clean copied content",
        subtitle: "When you copy from the terminal, run the clean-copy pipeline automatically (whitespace, prompts, URLs, and more).",
    },
    BoolSpec {
        id: "flatten-multiline",
        title: "Flatten multi-line commands",
        subtitle: "Join wrapped shell commands and continuations into a single line when you clean copy.",
    },
    BoolSpec {
        id: "preserve-blank-lines",
        title: "Preserve blank lines when flattening",
        subtitle: "Keep intentional blank lines inside a flattened command block.",
    },
    BoolSpec {
        id: "remove-box-drawing",
        title: "Remove box-drawing characters",
        subtitle: "Strip terminal table and box-drawing glyphs during cleaning.",
    },
    BoolSpec {
        id: "flatten-slash-commands",
        title: "Flatten slash-command selections",
        subtitle: "Treat agent slash-command decorations like wrapped commands when cleaning.",
    },
    BoolSpec {
        id: "strip-url-tracking",
        title: "Strip URL tracking parameters",
        subtitle: "Remove common tracking query parameters from URLs in copied text.",
    },
    BoolSpec {
        id: "quote-paths",
        title: "Quote paths with spaces",
        subtitle: "Wrap filesystem paths that contain spaces in quotes when cleaning.",
    },
    BoolSpec {
        id: "show-copy-markdown",
        title: "Show Copy as Markdown command",
        subtitle: "Include Copy as Markdown for selection-based Markdown reformatting.",
    },
];

pub(crate) fn build(initial: GeneralSettings, apply: &ApplyGeneral) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.append(&section_heading(
        "General",
        "Confirmations, restore, and clipboard",
    ));
    let state = Rc::new(RefCell::new(initial));
    content.append(&settings_group(LIFECYCLE, &state, apply));
    content.append(&settings_group(&CLIPBOARD[..2], &state, apply));
    content.append(&aggressiveness_row(&state, apply));
    content.append(&settings_group(&CLIPBOARD[2..], &state, apply));
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&content));
    scroll.upcast()
}

fn section_heading(title: &str, subtitle: &str) -> gtk::Widget {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    labels.append(&title);
    labels.append(&subtitle);
    labels.upcast()
}

fn settings_group(
    specs: &[BoolSpec],
    state: &Rc<RefCell<GeneralSettings>>,
    apply: &ApplyGeneral,
) -> gtk::Widget {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
    group.add_css_class("card");
    for spec in specs {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(12);
        row.set_margin_bottom(12);
        row.set_margin_start(14);
        row.set_margin_end(14);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let title = gtk::Label::new(Some(spec.title));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("heading");
        let subtitle = gtk::Label::new(Some(spec.subtitle));
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_wrap(true);
        subtitle.add_css_class("dim-label");
        labels.append(&title);
        labels.append(&subtitle);
        let toggle = gtk::Switch::new();
        toggle.set_widget_name(&format!("general-{}", spec.id));
        toggle.set_valign(gtk::Align::Center);
        toggle.set_active(state.borrow().bool_value(spec.id).unwrap_or(false));
        let id = spec.id;
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(move |_| {
            eprintln!("zentty-linux: general-settings focus={id}");
        });
        toggle.add_controller(focus);
        let state = Rc::clone(state);
        let apply = Rc::clone(apply);
        toggle.connect_active_notify(move |toggle| {
            let snapshot = {
                let mut state = state.borrow_mut();
                let Some(snapshot) = state.update_bool(id, toggle.is_active()) else {
                    return;
                };
                snapshot
            };
            eprintln!(
                "zentty-linux: general-settings action={id} value={}",
                toggle.is_active()
            );
            apply(snapshot);
        });
        row.append(&labels);
        row.append(&toggle);
        group.append(&row);
    }
    group.upcast()
}

fn aggressiveness_row(state: &Rc<RefCell<GeneralSettings>>, apply: &ApplyGeneral) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_start(14);
    row.set_margin_end(14);
    let label = gtk::Label::new(Some("Command flatten aggressiveness"));
    label.set_hexpand(true);
    label.set_halign(gtk::Align::Start);
    label.add_css_class("heading");
    let dropdown = gtk::DropDown::from_strings(&["Low", "Normal", "High"]);
    dropdown.set_widget_name("general-command-flatten-aggressiveness");
    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter(move |_| {
        eprintln!("zentty-linux: general-settings focus=command-flatten-aggressiveness");
    });
    dropdown.add_controller(focus);
    dropdown.set_selected(selected_for_aggressiveness(
        state
            .borrow()
            .clipboard
            .clean_options
            .command_flatten_aggressiveness,
    ));
    let state = Rc::clone(state);
    let apply = Rc::clone(apply);
    dropdown.connect_selected_notify(move |dropdown| {
        let Some(value) = aggressiveness_for_selected(dropdown.selected()) else {
            return;
        };
        let snapshot = {
            let mut state = state.borrow_mut();
            state.clipboard.clean_options.command_flatten_aggressiveness = value;
            *state
        };
        eprintln!(
            "zentty-linux: general-settings action=command-flatten-aggressiveness value={}",
            value.config_value()
        );
        apply(snapshot);
    });
    row.append(&label);
    row.append(&dropdown);
    row.upcast()
}

#[cfg(test)]
mod tests {
    use super::{
        CLIPBOARD, GeneralSettings, LIFECYCLE, aggressiveness_for_selected,
        selected_for_aggressiveness,
    };
    use zentty_core::{
        ClipboardConfig, CommandFlattenAggressiveness, ConfirmationsConfig, RestoreConfig,
    };

    #[test]
    fn every_source_boolean_has_one_independently_mutable_control() {
        let mut settings = GeneralSettings {
            confirmations: ConfirmationsConfig::default(),
            restore: RestoreConfig::default(),
            clipboard: ClipboardConfig::default(),
        };
        let ids = LIFECYCLE
            .iter()
            .chain(CLIPBOARD)
            .map(|spec| spec.id)
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 13);
        for id in ids {
            let before = settings.bool_value(id).unwrap();
            assert!(settings.set_bool(id, !before));
            assert_eq!(settings.bool_value(id), Some(!before));
            assert!(settings.set_bool(id, before));
        }
        assert!(!settings.set_bool("unknown", true));
        assert_eq!(settings.bool_value("unknown"), None);
    }

    #[test]
    fn widget_boundary_conversions_are_total_for_every_source_value() {
        for (selected, value) in [
            (0, CommandFlattenAggressiveness::Low),
            (1, CommandFlattenAggressiveness::Normal),
            (2, CommandFlattenAggressiveness::High),
        ] {
            assert_eq!(selected_for_aggressiveness(value), selected);
            assert_eq!(aggressiveness_for_selected(selected), Some(value));
        }
        assert_eq!(aggressiveness_for_selected(3), None);

        let mut settings = GeneralSettings {
            confirmations: ConfirmationsConfig::default(),
            restore: RestoreConfig::default(),
            clipboard: ClipboardConfig::default(),
        };
        assert!(
            !settings
                .update_bool("confirm-close-pane", false)
                .unwrap()
                .confirmations
                .confirm_before_closing_pane
        );
        assert_eq!(settings.update_bool("unknown", true), None);
    }
}
