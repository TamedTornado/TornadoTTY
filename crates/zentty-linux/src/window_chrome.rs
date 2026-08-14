use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use std::cell::RefCell;
use zentty_core::{OpenWithCatalog, OpenWithTargetKind, SidebarWorklaneSummary};

use crate::source_ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChromeControlSpec {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    enabled: bool,
}

const CHROME_CONTROLS: [ChromeControlSpec; 6] = [
    ChromeControlSpec {
        id: "toggle-sidebar",
        label: source_ui::TOGGLE_SIDEBAR,
        icon: "sidebar-show-symbolic",
        enabled: true,
    },
    ChromeControlSpec {
        id: "arrange-panes",
        label: source_ui::ARRANGE_PANES,
        icon: "view-grid-symbolic",
        enabled: true,
    },
    ChromeControlSpec {
        id: "back",
        label: source_ui::NAVIGATE_BACK,
        icon: "go-previous-symbolic",
        enabled: false,
    },
    ChromeControlSpec {
        id: "forward",
        label: source_ui::NAVIGATE_FORWARD,
        icon: "go-next-symbolic",
        enabled: false,
    },
    ChromeControlSpec {
        id: "notifications",
        label: source_ui::NOTIFICATIONS,
        icon: "preferences-system-notifications-symbolic",
        enabled: true,
    },
    ChromeControlSpec {
        id: "agent-status",
        label: source_ui::AGENT_STATUS,
        icon: "system-run-symbolic",
        enabled: true,
    },
];

pub(crate) struct WindowChrome {
    root: gtk::CenterBox,
    context: gtk::Label,
    project_icon: gtk::Picture,
    project: gtk::Box,
    branch: gtk::Button,
    pull_request: gtk::Button,
    refresh_review: gtk::Button,
    back: gtk::Button,
    forward: gtk::Button,
    notifications: gtk::MenuButton,
    notification_badge: gtk::Label,
    rendered_attention: RefCell<Vec<zentty_core::AttentionItem>>,
    fleet: gtk::MenuButton,
    fleet_indicator: gtk::Box,
    rendered_fleet: RefCell<Vec<zentty_core::FleetPaneSnapshot>>,
    open_with_primary: gtk::Button,
    open_with_menu: gtk::MenuButton,
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

        let (back, forward) = navigation_controls();
        leading.append(&back);
        leading.append(&forward);

        let (notifications, notification_badge) = notification_control();
        notifications.set_margin_start(4);
        leading.append(&notifications);
        let (fleet, fleet_indicator) = fleet_control();
        leading.append(&fleet);

        let context = gtk::Label::new(None);
        context.add_css_class("zentty-window-context");
        context.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        context.set_max_width_chars(64);

        let center = gtk::Box::new(gtk::Orientation::Vertical, 0);
        center.set_halign(gtk::Align::Center);
        let context_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        context_row.set_halign(gtk::Align::Center);
        let project_icon = crate::project_icon_view::picture("zentty-chrome-project-icon", 18);
        context_row.append(&project_icon);
        context_row.append(&context);
        center.append(&context_row);
        let project = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        project.set_halign(gtk::Align::Center);
        project.set_widget_name("zentty-project-context");
        let branch = gtk::Button::new();
        branch.set_has_frame(false);
        branch.add_css_class("project-context-branch");
        branch.set_action_name(Some("workspace.open-branch-remote"));
        branch.set_tooltip_text(Some("Open branch on remote"));
        let pull_request = gtk::Button::new();
        pull_request.set_has_frame(false);
        pull_request.add_css_class("review-chip");
        pull_request.add_css_class("review-chip-info");
        pull_request.set_action_name(Some("workspace.open-pull-request"));
        pull_request.set_tooltip_text(Some("Open pull request"));
        let refresh_review = gtk::Button::new();
        refresh_review.set_has_frame(false);
        refresh_review.set_icon_name("view-refresh-symbolic");
        refresh_review.set_action_name(Some("workspace.refresh-review-status"));
        refresh_review.set_tooltip_text(Some("Refresh Git and pull-request status"));
        refresh_review.set_accessible_role(gtk::AccessibleRole::Button);
        refresh_review.update_property(&[gtk::accessible::Property::Label(
            "Refresh Git and pull-request status",
        )]);
        project.append(&branch);
        project.append(&pull_request);
        project.append(&refresh_review);
        project.set_visible(false);
        center.append(&project);

        let trailing = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        trailing.add_css_class("open-with-control");
        let open_with_primary = gtk::Button::new();
        open_with_primary.add_css_class("open-with-primary");
        open_with_primary.set_icon_name("folder-open-symbolic");
        open_with_primary.set_action_name(Some("workspace.open-with-primary"));
        open_with_primary.set_sensitive(false);
        let open_with_menu = gtk::MenuButton::new();
        open_with_menu.add_css_class("open-with-menu");
        open_with_menu.set_icon_name("pan-down-symbolic");
        open_with_menu.set_tooltip_text(Some(source_ui::SHOW_OPEN_WITH_MENU));
        open_with_menu.update_property(&[gtk::accessible::Property::Label(
            source_ui::SHOW_OPEN_WITH_MENU,
        )]);
        open_with_menu.set_sensitive(false);
        trailing.append(&open_with_primary);
        trailing.append(&open_with_menu);

        root.set_start_widget(Some(&leading));
        root.set_center_widget(Some(&center));
        root.set_end_widget(Some(&trailing));
        log_chrome_controls();
        Self {
            root,
            context,
            project_icon,
            project,
            branch,
            pull_request,
            refresh_review,
            back,
            forward,
            notifications,
            notification_badge,
            rendered_attention: RefCell::new(Vec::new()),
            fleet,
            fleet_indicator,
            rendered_fleet: RefCell::new(Vec::new()),
            open_with_primary,
            open_with_menu,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::CenterBox {
        &self.root
    }

    pub(crate) fn configure_open_with(&self, catalog: &OpenWithCatalog) {
        if let Some(primary) = &catalog.primary {
            self.open_with_primary
                .set_icon_name(open_with_icon(primary.kind));
            let label = format!("Open focused pane in {}", primary.name);
            self.open_with_primary.set_tooltip_text(Some(&label));
            self.open_with_primary
                .update_property(&[gtk::accessible::Property::Label(&label)]);
        } else {
            self.open_with_primary
                .set_tooltip_text(Some("Open With unavailable"));
        }
        let popover = gtk::Popover::new();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        list.set_margin_top(6);
        list.set_margin_bottom(6);
        list.set_margin_start(6);
        list.set_margin_end(6);
        let heading = gtk::Label::new(Some(source_ui::OPEN_WITH));
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        heading.set_margin_start(6);
        list.append(&heading);
        for target in &catalog.enabled {
            let button = gtk::Button::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let icon = gtk::Image::from_icon_name(open_with_icon(target.kind));
            let label = gtk::Label::new(Some(&target.name));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            content.append(&icon);
            content.append(&label);
            button.set_child(Some(&content));
            button.set_halign(gtk::Align::Fill);
            button.set_action_name(Some("workspace.open-with-target"));
            button.set_action_target_value(Some(&target.id.to_variant()));
            let menu = popover.clone();
            button.connect_clicked(move |_| menu.popdown());
            list.append(&button);
        }
        popover.set_child(Some(&list));
        self.open_with_menu.set_popover(Some(&popover));
        self.open_with_primary
            .set_sensitive(catalog.primary.is_some());
        self.open_with_menu
            .set_sensitive(!catalog.enabled.is_empty());
    }

    pub(crate) fn set_open_with_context_available(&self, available: bool) {
        self.open_with_primary.set_sensitive(available);
        self.open_with_menu.set_sensitive(available);
    }

    pub(crate) fn render_attention(&self, items: &[zentty_core::AttentionItem]) {
        if self.rendered_attention.borrow().as_slice() == items {
            return;
        }
        let count = items.iter().filter(|item| !item.is_resolved()).count();
        crate::attention_inbox::update_badge(&self.notification_badge, count);
        self.notifications
            .set_popover(Some(&crate::attention_inbox::popover(items)));
        self.notifications.set_sensitive(true);
        self.notifications.set_tooltip_text(Some(&if count == 0 {
            "Notification Inbox".to_owned()
        } else {
            format!("Notification Inbox · {count} unresolved")
        }));
        self.rendered_attention.replace(items.to_vec());
    }

    pub(crate) fn render_fleet(&self, snapshots: &[zentty_core::FleetPaneSnapshot]) {
        if self.rendered_fleet.borrow().as_slice() == snapshots {
            return;
        }
        let summary = zentty_core::FleetSummary::from_snapshots(snapshots);
        crate::agent_fleet::update_indicator(&self.fleet_indicator, summary);
        self.fleet
            .set_popover(Some(&crate::agent_fleet::popover(snapshots)));
        self.fleet.set_tooltip_text(Some(&summary.header()));
        self.fleet
            .update_property(&[gtk::accessible::Property::Label(
                &summary.accessibility_label(),
            )]);
        self.rendered_fleet.replace(snapshots.to_vec());
    }

    pub(crate) fn show_fleet(&self) {
        self.fleet.popup();
    }

    pub(crate) fn connect_fleet_closed(&self, callback: impl Fn() + 'static) {
        self.fleet.connect_active_notify(move |button| {
            if !button.is_active() {
                callback();
            }
        });
    }

    pub(crate) fn fleet_snapshot(&self) -> Vec<zentty_core::FleetPaneSnapshot> {
        self.rendered_fleet.borrow().clone()
    }

    pub(crate) fn dismiss_status_popovers(&self) {
        self.notifications.popdown();
        self.fleet.popdown();
    }

    pub(crate) fn render(
        &self,
        summaries: &[SidebarWorklaneSummary],
        can_navigate_back: bool,
        can_navigate_forward: bool,
    ) {
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
        let focused_pane = summaries
            .iter()
            .find(|summary| summary.is_active)
            .and_then(|summary| summary.pane_rows.iter().find(|pane| pane.is_focused));
        crate::project_icon_view::configure(
            &self.project_icon,
            focused_pane.and_then(|pane| pane.project_icon_path.as_deref()),
            "window-chrome",
        );
        let project_context = focused_pane.and_then(|pane| pane.project_context.as_ref());
        if let Some(project_context) = project_context {
            let reference = project_context.reference.display();
            self.branch.set_label(&if project_context.dirty {
                format!("{reference}  ●")
            } else {
                reference
            });
            let branch_enabled =
                project_context.reference.branch().is_some() && project_context.remote.is_some();
            self.branch.set_sensitive(branch_enabled);
            self.branch
                .update_property(&[gtk::accessible::Property::Label(&format!(
                    "{} branch {} on remote",
                    if branch_enabled { "Open" } else { "View" },
                    project_context.reference.display()
                ))]);
            if let Some(review) = &project_context.review {
                self.pull_request
                    .set_label(&format!("PR #{}", review.pull_request.number));
                self.pull_request
                    .set_sensitive(review.pull_request.url.is_some());
                self.pull_request.set_visible(true);
            } else {
                self.pull_request.set_visible(false);
            }
            self.project
                .set_tooltip_text(project_context.review_error.as_deref());
            self.project.set_visible(true);
            self.refresh_review.set_sensitive(true);
        } else {
            self.project.set_visible(false);
        }
        self.back.set_sensitive(can_navigate_back);
        self.forward.set_sensitive(can_navigate_forward);
    }
}

fn log_chrome_controls() {
    eprintln!(
        "zentty-linux: chrome-controls={}",
        CHROME_CONTROLS
            .iter()
            .map(|control| control.id)
            .collect::<Vec<_>>()
            .join(",")
    );
}

fn open_with_icon(kind: OpenWithTargetKind) -> &'static str {
    match kind {
        OpenWithTargetKind::Editor => "accessories-text-editor-symbolic",
        OpenWithTargetKind::FileManager => "folder-open-symbolic",
        OpenWithTargetKind::Terminal => "utilities-terminal-symbolic",
    }
}

fn notification_control() -> (gtk::MenuButton, gtk::Label) {
    let button = gtk::MenuButton::new();
    configure_menu_button(&button, CHROME_CONTROLS[4]);
    let (content, badge) = crate::attention_inbox::button_content(0);
    button.set_child(Some(&content));
    button.set_popover(Some(&crate::attention_inbox::popover(&[])));
    button.connect_active_notify(|button| {
        eprintln!(
            "zentty-linux: chrome-popover=attention-inbox state={}",
            if button.is_active() { "open" } else { "closed" }
        );
    });
    (button, badge)
}

fn fleet_control() -> (gtk::MenuButton, gtk::Box) {
    let button = gtk::MenuButton::new();
    configure_menu_button(&button, CHROME_CONTROLS[5]);
    let (content, indicator) = crate::agent_fleet::button_content();
    button.set_child(Some(&content));
    button.set_popover(Some(&crate::agent_fleet::popover(&[])));
    button.connect_active_notify(|button| {
        eprintln!(
            "zentty-linux: chrome-popover=agent-fleet state={}",
            if button.is_active() { "open" } else { "closed" }
        );
    });
    (button, indicator)
}

fn navigation_controls() -> (gtk::Button, gtk::Button) {
    let back = icon_button(CHROME_CONTROLS[2]);
    back.set_action_name(Some("workspace.navigate-back"));
    let forward = icon_button(CHROME_CONTROLS[3]);
    forward.set_action_name(Some("workspace.navigate-forward"));
    (back, forward)
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

type ArrangeAction = (&'static str, &'static str, &'static str);

const CREATE_ACTIONS: [ArrangeAction; 4] = [
    (
        source_ui::SPLIT_RIGHT,
        "go-next-symbolic",
        "split-pane-right",
    ),
    (
        source_ui::ADD_PANE_RIGHT,
        "application-add-symbolic",
        "add-pane-right",
    ),
    (
        source_ui::ADD_PANE_LEFT,
        "go-previous-symbolic",
        "add-pane-left",
    ),
    (
        source_ui::NEW_PANE_BELOW,
        "go-down-symbolic",
        "split-pane-below",
    ),
];

const WIDTH_ACTIONS: [ArrangeAction; 6] = [
    (
        source_ui::ARRANGE_WIDTH_FULL,
        "view-fullscreen-symbolic",
        "arrange-width-full",
    ),
    (
        source_ui::ARRANGE_WIDTH_HALF,
        "view-dual-symbolic",
        "arrange-width-half",
    ),
    (
        source_ui::ARRANGE_WIDTH_THIRDS,
        "view-grid-symbolic",
        "arrange-width-thirds",
    ),
    (
        source_ui::ARRANGE_WIDTH_QUARTERS,
        "view-grid-symbolic",
        "arrange-width-quarters",
    ),
    (
        source_ui::ARRANGE_GOLDEN_WIDE,
        "zoom-in-symbolic",
        "arrange-golden-wide",
    ),
    (
        source_ui::ARRANGE_GOLDEN_NARROW,
        "zoom-out-symbolic",
        "arrange-golden-narrow",
    ),
];

const HEIGHT_ACTIONS: [ArrangeAction; 6] = [
    (
        source_ui::ARRANGE_HEIGHT_FULL,
        "view-fullscreen-symbolic",
        "arrange-height-full",
    ),
    (
        source_ui::ARRANGE_HEIGHT_TWO,
        "view-list-symbolic",
        "arrange-height-two",
    ),
    (
        source_ui::ARRANGE_HEIGHT_THREE,
        "view-list-symbolic",
        "arrange-height-three",
    ),
    (
        source_ui::ARRANGE_HEIGHT_FOUR,
        "view-list-symbolic",
        "arrange-height-four",
    ),
    (
        source_ui::ARRANGE_GOLDEN_TALL,
        "zoom-in-symbolic",
        "arrange-golden-tall",
    ),
    (
        source_ui::ARRANGE_GOLDEN_SHORT,
        "zoom-out-symbolic",
        "arrange-golden-short",
    ),
];

const DEFAULT_ACTIONS: [ArrangeAction; 1] = [(
    source_ui::RESET_PANE_LAYOUT,
    "edit-undo-symbolic",
    "reset-pane-layout",
)];

fn arrange_panes_popover() -> gtk::Popover {
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    append_action_section(&menu, "Create", &CREATE_ACTIONS, &popover);
    append_action_section(&menu, "Column width", &WIDTH_ACTIONS, &popover);
    append_action_section(&menu, "Panes per column", &HEIGHT_ACTIONS, &popover);
    append_action_section(&menu, "Defaults", &DEFAULT_ACTIONS, &popover);
    popover.set_child(Some(&menu));
    popover
}

fn append_action_section(
    menu: &gtk::Box,
    heading: &str,
    actions: &[(&'static str, &'static str, &'static str)],
    popover: &gtk::Popover,
) {
    let heading = gtk::Label::new(Some(heading));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    heading.set_margin_top(4);
    heading.set_margin_start(4);
    menu.append(&heading);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    for (index, (label, icon, action)) in actions.iter().copied().enumerate() {
        let button = arrange_action_button(label, icon, action, popover);
        let column = i32::try_from(index % 2).expect("two-column index fits i32");
        let row = i32::try_from(index / 2).expect("small menu index fits i32");
        grid.attach(&button, column, row, 1, 1);
    }
    menu.append(&grid);
}

fn arrange_action_button(
    label: &'static str,
    icon: &'static str,
    action: &'static str,
    popover: &gtk::Popover,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_hexpand(true);
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
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
    button
}

#[cfg(test)]
mod tests {
    use super::CHROME_CONTROLS;
    use crate::source_ui;

    #[test]
    fn leading_chrome_matches_the_source_control_order_and_availability() {
        assert_eq!(
            CHROME_CONTROLS.map(|control| (control.id, control.label, control.enabled)),
            [
                ("toggle-sidebar", source_ui::TOGGLE_SIDEBAR, true),
                ("arrange-panes", source_ui::ARRANGE_PANES, true),
                ("back", source_ui::NAVIGATE_BACK, false),
                ("forward", source_ui::NAVIGATE_FORWARD, false),
                ("notifications", source_ui::NOTIFICATIONS, true),
                ("agent-status", source_ui::AGENT_STATUS, true),
            ]
        );
        assert!(
            CHROME_CONTROLS
                .iter()
                .all(|control| !control.icon.is_empty())
        );
    }
}
