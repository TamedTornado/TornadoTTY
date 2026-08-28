use gtk::gio;
use gtk::prelude::*;
use zentty_core::ClipboardConfig;
use zentty_ghostty::{Error, GhosttySurface};

use crate::source_ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CopyCompanion {
    Clean,
    Raw,
}

impl CopyCompanion {
    const fn for_config(config: ClipboardConfig) -> Self {
        if config.always_clean_copies {
            Self::Raw
        } else {
            Self::Clean
        }
    }

    const fn menu_item(self) -> (&'static str, &'static str) {
        match self {
            Self::Clean => (source_ui::CLEAN_COPY, "workspace.clean-copy"),
            Self::Raw => (source_ui::COPY_RAW, "workspace.copy-raw"),
        }
    }
}

pub(super) fn install(surface: &GhosttySurface) -> Result<gio::Menu, Error> {
    let menu = gio::Menu::new();
    refresh(&menu, ClipboardConfig::default());
    surface.set_context_menu_model(menu.upcast_ref())?;
    Ok(menu)
}

pub(super) fn refresh(menu: &gio::Menu, config: ClipboardConfig) {
    menu.remove_all();

    let clipboard = gio::Menu::new();
    clipboard.append(Some(source_ui::COPY), Some("win.copy"));
    let (label, action) = CopyCompanion::for_config(config).menu_item();
    clipboard.append(Some(label), Some(action));
    clipboard.append(Some("Paste"), Some("win.paste"));
    clipboard.append(
        Some("Notify on Next Command Finish"),
        Some("surface.notify-on-next-command-finish"),
    );
    menu.append_section(None, &clipboard);

    let terminal = gio::Menu::new();
    terminal.append(Some("Clear"), Some("win.clear"));
    terminal.append(Some("Reset"), Some("win.reset"));
    menu.append_section(None, &terminal);

    let layout = gio::Menu::new();
    layout.append_submenu(Some("Split"), &split_menu());
    layout.append_submenu(Some("Tab"), &tab_menu());
    layout.append_submenu(Some("Window"), &window_menu());
    menu.append_section(None, &layout);

    let configuration = gio::Menu::new();
    configuration.append_submenu(Some("Config"), &config_menu());
    menu.append_section(None, &configuration);
}

fn split_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Change Title…"), Some("surface.prompt-title"));
    menu.append(Some("Split Up"), Some("split-tree.new-split::up"));
    menu.append(Some("Split Down"), Some("split-tree.new-split::down"));
    menu.append(Some("Split Left"), Some("split-tree.new-split::left"));
    menu.append(Some("Split Right"), Some("split-tree.new-split::right"));
    menu.append(Some("Close Split"), Some("split-tree.close-split"));
    menu
}

fn tab_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Change Tab Title…"), Some("tab.prompt-tab-title"));
    menu.append(Some("New Tab"), Some("win.new-tab"));
    menu.append(Some("Close Tab"), Some("tab.close::this"));
    menu
}

fn window_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("Change Window Title…"),
        Some("win.prompt-window-title"),
    );
    menu.append(Some("New Window"), Some("win.new-window"));
    menu.append(Some("Close Window"), Some("win.close"));
    menu
}

fn config_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("Open Configuration in OS Editor"),
        Some("app.open-config::os-open"),
    );
    menu.append(
        Some("Open Configuration in New Window"),
        Some("app.open-config::new-window"),
    );
    menu.append(Some("Reload Configuration"), Some("app.reload-config"));
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_section_labels(menu: &gio::Menu) -> Vec<String> {
        let section = menu
            .item_link(0, gio::MENU_LINK_SECTION.as_str())
            .expect("first menu section");
        (0..section.n_items())
            .map(|index| {
                section
                    .item_attribute_value(index, gio::MENU_ATTRIBUTE_LABEL.as_str(), None)
                    .and_then(|value| value.get::<String>())
                    .expect("string menu label")
            })
            .collect()
    }

    #[test]
    fn normal_copy_offers_clean_companion() {
        assert_eq!(
            CopyCompanion::for_config(ClipboardConfig::default()).menu_item(),
            (source_ui::CLEAN_COPY, "workspace.clean-copy")
        );

        let menu = gio::Menu::new();
        refresh(&menu, ClipboardConfig::default());
        assert_eq!(
            first_section_labels(&menu),
            [
                "Copy",
                "Clean Copy",
                "Paste",
                "Notify on Next Command Finish"
            ]
        );
    }

    #[test]
    fn automatic_clean_copy_offers_raw_escape_hatch() {
        let config = ClipboardConfig {
            always_clean_copies: true,
            ..ClipboardConfig::default()
        };
        assert_eq!(
            CopyCompanion::for_config(config).menu_item(),
            (source_ui::COPY_RAW, "workspace.copy-raw")
        );

        let menu = gio::Menu::new();
        refresh(&menu, config);
        assert_eq!(
            first_section_labels(&menu),
            ["Copy", "Copy Raw", "Paste", "Notify on Next Command Finish"]
        );
    }
}
