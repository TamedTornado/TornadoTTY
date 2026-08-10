use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use sha2::{Digest, Sha256};
use zentty_core::{CleanCopyOptions, clean_copy, is_likely_markdown, reformat_markdown};

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
        let transformed = match style {
            CopyStyle::Clean => clean_copy(&raw, CleanCopyOptions::default()).text,
            CopyStyle::Markdown if is_likely_markdown(&raw) => reformat_markdown(&raw),
            CopyStyle::Default | CopyStyle::Raw | CopyStyle::Markdown => raw.clone(),
        };
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
