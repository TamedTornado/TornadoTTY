use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use zentty_core::{
    AgentCaffeinationConfig, AgentIntegrationState, AgentIntegrationsConfig, AgentTeamsConfig,
    MenuBarConfig,
};

pub(crate) type ApplyAgents = Rc<
    dyn Fn(
        AgentTeamsConfig,
        AgentCaffeinationConfig,
        MenuBarConfig,
        AgentIntegrationsConfig,
    ) -> Result<(), String>,
>;

const AVAILABLE_INTEGRATIONS: [(&str, &str); 3] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("gemini", "Gemini CLI"),
];
const UNAVAILABLE_PERSISTENT: [(&str, &str); 8] = [
    ("amp", "Amp"),
    ("cursor", "Cursor"),
    ("droid", "Droid"),
    ("grok", "Grok"),
    ("agy", "Agy"),
    ("hermes", "Hermes"),
    ("vibe", "Vibe"),
    ("kimi", "Kimi"),
];
const UNAVAILABLE_EPHEMERAL: [(&str, &str); 5] = [
    ("copilot", "GitHub Copilot CLI"),
    ("opencode", "OpenCode"),
    ("pi", "Pi"),
    ("omp", "Oh My Posh"),
    ("small-harness", "Small Harness"),
];

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings page.
pub(crate) fn build(
    teams: AgentTeamsConfig,
    caffeination: AgentCaffeinationConfig,
    menu_bar: MenuBarConfig,
    integrations: AgentIntegrationsConfig,
    apply: ApplyAgents,
) -> gtk::Widget {
    eprintln!(
        "zentty-linux: agent-settings loaded teams={} wrappers-available={} source-unavailable={} status-item-available=false caffeination-available=false",
        teams.enabled,
        AVAILABLE_INTEGRATIONS.len(),
        UNAVAILABLE_PERSISTENT.len() + UNAVAILABLE_EPHEMERAL.len(),
    );
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(28);
    root.set_margin_bottom(28);
    root.set_margin_start(30);
    root.set_margin_end(30);
    let title = gtk::Label::new(Some("Agents"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    root.append(&title);
    let subtitle = gtk::Label::new(Some(
        "Configure coding-agent integrations and Claude Code agent teams.",
    ));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.add_css_class("dim-label");
    root.append(&subtitle);

    let teams_switch = gtk::Switch::builder()
        .active(teams.enabled)
        .valign(gtk::Align::Center)
        .build();
    teams_switch.set_widget_name("settings-agents-teams");
    let behavior = card("Behavior");
    behavior.append(&setting_row(
        "Claude Code agent teams (experimental)",
        "Expose Zentty's tmux-compatible team environment to newly created panes.",
        &teams_switch,
    ));
    behavior.append(&unavailable_row(
        "Show agent status in menu bar",
        menu_bar.show_status_item,
        "Unavailable on Linux until a reviewed status-item backend is implemented.",
        "settings-agents-status-item",
    ));
    behavior.append(&unavailable_row(
        "Prevent sleep while agents run",
        caffeination.enabled,
        "Unavailable on Linux until the desktop sleep-inhibitor lifecycle is qualified.",
        "settings-agents-caffeination",
    ));
    root.append(&behavior);

    let integrations_box = card("Agent integrations");
    let state = Rc::new(RefCell::new(integrations));
    for (id, name) in AVAILABLE_INTEGRATIONS {
        let toggle = gtk::Switch::builder()
            .active(state.borrow().states.get(id) != Some(&AgentIntegrationState::Off))
            .valign(gtk::Align::Center)
            .build();
        toggle.set_widget_name(&format!("settings-agents-integration-{id}"));
        integrations_box.append(&setting_row(
            name,
            "Use Zentty's authenticated wrapper for newly created panes.",
            &toggle,
        ));
        let state = Rc::clone(&state);
        let apply = Rc::clone(&apply);
        toggle.connect_active_notify(move |toggle| {
            state.borrow_mut().states.insert(
                id.to_owned(),
                if toggle.is_active() {
                    AgentIntegrationState::On
                } else {
                    AgentIntegrationState::Off
                },
            );
            apply_current(&apply, teams, caffeination, menu_bar, &state.borrow());
        });
    }
    for (id, name) in UNAVAILABLE_PERSISTENT
        .into_iter()
        .chain(UNAVAILABLE_EPHEMERAL)
    {
        integrations_box.append(&unavailable_row(
            name,
            false,
            "Not yet available in the Linux wrapper inventory.",
            &format!("settings-agents-integration-{id}"),
        ));
    }
    root.append(&integrations_box);

    {
        let state = Rc::clone(&state);
        teams_switch.connect_active_notify(move |control| {
            apply_current(
                &apply,
                AgentTeamsConfig {
                    enabled: control.is_active(),
                },
                caffeination,
                menu_bar,
                &state.borrow(),
            );
        });
    }

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label("Agents Settings")]);
    scroll.upcast()
}

fn apply_current(
    apply: &ApplyAgents,
    teams: AgentTeamsConfig,
    caffeination: AgentCaffeinationConfig,
    menu_bar: MenuBarConfig,
    integrations: &AgentIntegrationsConfig,
) {
    if let Err(error) = apply(teams, caffeination, menu_bar, integrations.clone()) {
        eprintln!("zentty-linux: agent-settings result=error detail={error}");
    }
}

fn unavailable_row(title: &str, configured: bool, reason: &str, widget_name: &str) -> gtk::Widget {
    let control = gtk::Switch::builder()
        .active(configured)
        .sensitive(false)
        .valign(gtk::Align::Center)
        .build();
    control.set_widget_name(widget_name);
    setting_row(title, reason, &control)
}

fn card(title: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("card");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    card.append(&heading);
    card
}

fn setting_row(title: &str, subtitle: &str, control: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);
    row.append(control);
    row.upcast()
}

#[cfg(test)]
mod tests {
    use super::{AVAILABLE_INTEGRATIONS, UNAVAILABLE_EPHEMERAL, UNAVAILABLE_PERSISTENT};

    #[test]
    fn source_agent_inventory_never_disappears_silently() {
        assert_eq!(
            AVAILABLE_INTEGRATIONS.map(|(id, _)| id),
            ["claude", "codex", "gemini"]
        );
        assert!(UNAVAILABLE_PERSISTENT.iter().any(|(id, _)| *id == "amp"));
        assert!(
            UNAVAILABLE_EPHEMERAL
                .iter()
                .any(|(id, _)| *id == "opencode")
        );
    }
}
