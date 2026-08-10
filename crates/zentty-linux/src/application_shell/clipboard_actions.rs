use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use sha2::{Digest, Sha256};
use zentty_core::{ClipboardConfig, clean_copy, is_likely_markdown, reformat_markdown};

use super::ApplicationShell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CopyStyle {
    Default,
    Raw,
    Clean,
    Markdown,
}

impl CopyStyle {
    pub(super) const fn action_name(self) -> &'static str {
        match self {
            Self::Default => "copy",
            Self::Raw => "copy-raw",
            Self::Clean => "clean-copy",
            Self::Markdown => "copy-as-markdown",
        }
    }
}

impl ApplicationShell {
    pub(super) fn copy_focused_selection(shell: &Rc<RefCell<Self>>, style: CopyStyle) {
        let (pane_id, raw) = {
            let shell = shell.borrow();
            let Some(pane_id) = shell.state.focused_pane_id() else {
                eprintln!(
                    "zentty-linux: action={} error=no-focused-pane",
                    style.action_name()
                );
                return;
            };
            let Some(surface) = shell.pane_runtime.surface(pane_id) else {
                eprintln!(
                    "zentty-linux: action={} pane={pane_id} error=no-live-surface",
                    style.action_name()
                );
                return;
            };
            let raw = match surface.read_selection() {
                Ok(raw) => raw,
                Err(error) => {
                    eprintln!(
                        "zentty-linux: action={} pane={pane_id} error=selection-unavailable detail={error}",
                        style.action_name()
                    );
                    return;
                }
            };
            (pane_id.to_owned(), raw)
        };
        shell.borrow().focus_selected_surface();
        let clipboard = shell.borrow().config.clipboard;
        let transformed = transform_selection(&raw, style, clipboard);
        let modified = transformed != raw;
        gtk::prelude::WidgetExt::display(&shell.borrow().window)
            .clipboard()
            .set_text(&transformed);
        let digest = Sha256::digest(transformed.as_bytes());
        eprintln!(
            "zentty-linux: action={} pane={pane_id} bytes={} modified={modified} sha256={digest:x}",
            style.action_name(),
            transformed.len()
        );
    }
}

fn resolved_style(style: CopyStyle, clipboard: ClipboardConfig) -> CopyStyle {
    if style == CopyStyle::Default && clipboard.always_clean_copies {
        CopyStyle::Clean
    } else {
        style
    }
}

fn transform_selection(raw: &str, style: CopyStyle, clipboard: ClipboardConfig) -> String {
    match resolved_style(style, clipboard) {
        CopyStyle::Clean => clean_copy(raw, clipboard.clean_options).text,
        CopyStyle::Markdown if is_likely_markdown(raw) => reformat_markdown(raw),
        CopyStyle::Default | CopyStyle::Raw | CopyStyle::Markdown => raw.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{CopyStyle, resolved_style, transform_selection};
    use zentty_core::{ClipboardConfig, reformat_markdown};

    #[test]
    fn automatic_clean_copy_changes_only_default_copy() {
        let mut clipboard = ClipboardConfig::default();
        for style in [
            CopyStyle::Default,
            CopyStyle::Raw,
            CopyStyle::Clean,
            CopyStyle::Markdown,
        ] {
            assert_eq!(resolved_style(style, clipboard), style);
        }
        clipboard.always_clean_copies = true;
        assert_eq!(
            resolved_style(CopyStyle::Default, clipboard),
            CopyStyle::Clean
        );
        assert_eq!(resolved_style(CopyStyle::Raw, clipboard), CopyStyle::Raw);
        assert_eq!(
            resolved_style(CopyStyle::Clean, clipboard),
            CopyStyle::Clean
        );
        assert_eq!(
            resolved_style(CopyStyle::Markdown, clipboard),
            CopyStyle::Markdown
        );
    }

    #[test]
    fn action_names_are_stable_product_receipts() {
        assert_eq!(CopyStyle::Default.action_name(), "copy");
        assert_eq!(CopyStyle::Raw.action_name(), "copy-raw");
        assert_eq!(CopyStyle::Clean.action_name(), "clean-copy");
        assert_eq!(CopyStyle::Markdown.action_name(), "copy-as-markdown");
    }

    #[test]
    fn transformation_policy_covers_automatic_raw_and_markdown_paths() {
        let raw = "https://example.test/path?utm_source=terminal";
        let clipboard = ClipboardConfig {
            always_clean_copies: true,
            ..ClipboardConfig::default()
        };
        assert_eq!(
            transform_selection(raw, CopyStyle::Default, clipboard),
            "https://example.test/path"
        );
        assert_eq!(transform_selection(raw, CopyStyle::Raw, clipboard), raw);

        let markdown = "## Heading\n\nwrapped\nbody";
        assert_eq!(
            transform_selection(markdown, CopyStyle::Markdown, clipboard),
            reformat_markdown(markdown)
        );
        let prose = "ordinary\nwrapped prose";
        assert_eq!(
            transform_selection(prose, CopyStyle::Markdown, clipboard),
            prose
        );
    }
}
