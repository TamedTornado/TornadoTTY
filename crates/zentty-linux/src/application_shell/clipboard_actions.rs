use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use sha2::{Digest, Sha256};
use zentty_core::{
    ClipboardConfig, clean_copy_with_columns, is_likely_markdown, reformat_markdown,
};

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
        let (pane_id, raw, columns) = {
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
            let columns = surface
                .cell_size()
                .ok()
                .and_then(|cell| terminal_columns(surface.widget().width(), cell.width));
            (pane_id.to_owned(), raw, columns)
        };
        shell.borrow().focus_selected_surface();
        let clipboard = shell.borrow().config.clipboard;
        let Some(transformed) = prepared_payload(&raw, style, clipboard, columns) else {
            eprintln!(
                "zentty-linux: action={} pane={pane_id} error=selection-empty",
                style.action_name()
            );
            return;
        };
        let modified = transformed != raw;
        let provider = gtk::gdk::ContentProvider::for_value(&transformed.to_value());
        let platform_clipboard =
            gtk::prelude::WidgetExt::display(&shell.borrow().window).clipboard();
        if let Err(error) = platform_clipboard.set_content(Some(&provider)) {
            eprintln!(
                "zentty-linux: action={} pane={pane_id} error=clipboard-write-failed detail={error}",
                style.action_name()
            );
            return;
        }
        let digest = Sha256::digest(transformed.as_bytes());
        eprintln!(
            "zentty-linux: action={} pane={pane_id} bytes={} modified={modified} columns={} sha256={digest:x}",
            style.action_name(),
            transformed.len(),
            columns.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
        );
    }
}

fn terminal_columns(widget_width: i32, cell_width: f64) -> Option<usize> {
    if widget_width <= 0 || !cell_width.is_finite() || cell_width < 1.0 {
        return None;
    }
    let width = f64::from(widget_width);
    let mut used = cell_width;
    let mut columns = 0_usize;
    while used <= width {
        columns = columns.checked_add(1)?;
        used += cell_width;
    }
    (columns > 0).then_some(columns)
}

fn prepared_payload(
    raw: &str,
    style: CopyStyle,
    clipboard: ClipboardConfig,
    columns: Option<usize>,
) -> Option<String> {
    (!raw.is_empty()).then(|| transform_selection(raw, style, clipboard, columns))
}

fn resolved_style(style: CopyStyle, clipboard: ClipboardConfig) -> CopyStyle {
    if style == CopyStyle::Default && clipboard.always_clean_copies {
        CopyStyle::Clean
    } else {
        style
    }
}

fn transform_selection(
    raw: &str,
    style: CopyStyle,
    clipboard: ClipboardConfig,
    columns: Option<usize>,
) -> String {
    match resolved_style(style, clipboard) {
        CopyStyle::Clean => clean_copy_with_columns(raw, clipboard.clean_options, columns).text,
        CopyStyle::Markdown if is_likely_markdown(raw) => reformat_markdown(raw),
        CopyStyle::Default | CopyStyle::Raw | CopyStyle::Markdown => raw.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CopyStyle, prepared_payload, resolved_style, terminal_columns, transform_selection,
    };
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
            transform_selection(raw, CopyStyle::Default, clipboard, None),
            "https://example.test/path"
        );
        assert_eq!(
            transform_selection(raw, CopyStyle::Raw, clipboard, None),
            raw
        );

        let markdown = "## Heading\n\nwrapped\nbody";
        assert_eq!(
            transform_selection(markdown, CopyStyle::Markdown, clipboard, None),
            reformat_markdown(markdown)
        );
        let prose = "ordinary\nwrapped prose";
        assert_eq!(
            transform_selection(prose, CopyStyle::Markdown, clipboard, None),
            prose
        );
    }

    #[test]
    fn empty_selection_never_replaces_an_existing_clipboard_owner() {
        assert_eq!(
            prepared_payload("", CopyStyle::Default, ClipboardConfig::default(), None),
            None
        );
        assert_eq!(
            prepared_payload(" ", CopyStyle::Raw, ClipboardConfig::default(), None),
            Some(" ".to_owned())
        );
    }

    #[test]
    fn terminal_columns_use_live_widget_and_cell_metrics() {
        assert_eq!(terminal_columns(800, 8.0), Some(100));
        assert_eq!(terminal_columns(799, 8.0), Some(99));
        assert_eq!(terminal_columns(0, 8.0), None);
        assert_eq!(terminal_columns(800, 0.0), None);
        assert_eq!(terminal_columns(800, 0.5), None);
        assert_eq!(terminal_columns(800, f64::NAN), None);
    }
}
