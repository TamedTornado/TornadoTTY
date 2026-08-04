use gtk::prelude::*;
use zentty_core::SidebarWorklaneSummary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChromeControlSpec {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    enabled: bool,
}

const CHROME_CONTROLS: [ChromeControlSpec; 5] = [
    ChromeControlSpec {
        id: "toggle-sidebar",
        label: "Toggle sidebar",
        icon: "sidebar-show-symbolic",
        enabled: true,
    },
    ChromeControlSpec {
        id: "arrange-panes",
        label: "Arrange panes",
        icon: "view-grid-symbolic",
        enabled: true,
    },
    ChromeControlSpec {
        id: "back",
        label: "Go back",
        icon: "go-previous-symbolic",
        enabled: false,
    },
    ChromeControlSpec {
        id: "forward",
        label: "Go forward",
        icon: "go-next-symbolic",
        enabled: false,
    },
    ChromeControlSpec {
        id: "notifications",
        label: "Notifications",
        icon: "preferences-system-notifications-symbolic",
        enabled: false,
    },
];

pub(crate) struct WindowChrome {
    root: gtk::CenterBox,
    context: gtk::Label,
}

impl WindowChrome {
    pub(crate) fn new() -> Self {
        let root = gtk::CenterBox::new();
        root.add_css_class("zentty-window-chrome");

        let leading = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let toggle = icon_button(CHROME_CONTROLS[0]);
        toggle.set_action_name(Some("workspace.toggle-sidebar"));
        leading.append(&toggle);

        let arrange = gtk::MenuButton::new();
        configure_menu_button(&arrange, CHROME_CONTROLS[1]);
        arrange.set_popover(Some(&arrange_panes_popover()));
        arrange.connect_active_notify(|button| {
            eprintln!(
                "zentty-linux: chrome-popover=arrange-panes state={}",
                if button.is_active() { "open" } else { "closed" }
            );
        });
        leading.append(&arrange);

        let back = icon_button(CHROME_CONTROLS[2]);
        leading.append(&back);
        let forward = icon_button(CHROME_CONTROLS[3]);
        leading.append(&forward);

        let notifications = icon_button(CHROME_CONTROLS[4]);
        notifications.set_margin_start(4);
        leading.append(&notifications);

        let context = gtk::Label::new(None);
        context.add_css_class("zentty-window-context");
        context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        context.set_max_width_chars(64);

        root.set_start_widget(Some(&leading));
        root.set_center_widget(Some(&context));
        eprintln!(
            "zentty-linux: chrome-controls={}",
            CHROME_CONTROLS
                .iter()
                .map(|control| control.id)
                .collect::<Vec<_>>()
                .join(",")
        );
        Self { root, context }
    }

    pub(crate) fn widget(&self) -> &gtk::CenterBox {
        &self.root
    }

    pub(crate) fn render(&self, summaries: &[SidebarWorklaneSummary]) {
        let text = summaries
            .iter()
            .find(|summary| summary.is_active)
            .map(|summary| {
                let lane = summary
                    .top_label
                    .as_deref()
                    .unwrap_or(summary.primary_text.as_str());
                let pane = summary
                    .pane_rows
                    .iter()
                    .find(|pane| pane.is_focused)
                    .map(|pane| pane.primary_text.as_str());
                pane.filter(|pane| *pane != lane)
                    .map_or_else(|| lane.to_owned(), |pane| format!("{lane}  ·  {pane}"))
            })
            .unwrap_or_default();
        self.context.set_text(&text);
        self.context
            .update_property(&[gtk::accessible::Property::Label(text.as_str())]);
    }
}

fn icon_button(spec: ChromeControlSpec) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("zentty-chrome-icon");
    button.set_icon_name(spec.icon);
    button.set_tooltip_text(Some(spec.label));
    button.set_accessible_role(gtk::AccessibleRole::Button);
    button.update_property(&[gtk::accessible::Property::Label(spec.label)]);
    button.set_sensitive(spec.enabled);
    button
}

fn configure_menu_button(button: &gtk::MenuButton, spec: ChromeControlSpec) {
    button.add_css_class("zentty-chrome-icon");
    button.set_icon_name(spec.icon);
    button.set_tooltip_text(Some(spec.label));
    button.set_accessible_role(gtk::AccessibleRole::Button);
    button.update_property(&[gtk::accessible::Property::Label(spec.label)]);
    button.set_sensitive(spec.enabled);
}

fn arrange_panes_popover() -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    for (label, icon, action) in [
        ("New Pane Right", "go-next-symbolic", "split-pane-right"),
        ("New Pane Below", "go-down-symbolic", "split-pane-below"),
    ] {
        let button = gtk::Button::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&gtk::Image::from_icon_name(icon));
        let text = gtk::Label::new(Some(label));
        text.set_xalign(0.0);
        text.set_hexpand(true);
        content.append(&text);
        button.set_child(Some(&content));
        button.set_action_name(Some(&format!("workspace.{action}")));
        let menu_popover = popover.clone();
        button.connect_clicked(move |_| menu_popover.popdown());
        menu.append(&button);
    }
    popover.set_child(Some(&menu));
    popover
}

#[cfg(test)]
mod tests {
    use super::CHROME_CONTROLS;

    #[test]
    fn leading_chrome_matches_the_source_control_order_and_availability() {
        assert_eq!(
            CHROME_CONTROLS.map(|control| (control.id, control.label, control.enabled)),
            [
                ("toggle-sidebar", "Toggle sidebar", true),
                ("arrange-panes", "Arrange panes", true),
                ("back", "Go back", false),
                ("forward", "Go forward", false),
                ("notifications", "Notifications", false),
            ]
        );
        assert!(
            CHROME_CONTROLS
                .iter()
                .all(|control| !control.icon.is_empty())
        );
    }
}
