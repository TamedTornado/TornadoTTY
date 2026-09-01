use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
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

const AVAILABLE_INTEGRATIONS: [(&str, &str); 8] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("copilot", "GitHub Copilot CLI"),
    ("gemini", "Gemini CLI"),
    ("opencode", "OpenCode"),
    ("pi", "Pi"),
    ("omp", "Oh My Pi"),
    ("small-harness", "Small Harness"),
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

struct State {
    teams: AgentTeamsConfig,
    caffeination: AgentCaffeinationConfig,
    menu_bar: MenuBarConfig,
    integrations: AgentIntegrationsConfig,
    apply: ApplyAgents,
    updating: Cell<bool>,
    status: gtk::Label,
}

#[allow(clippy::too_many_lines)] // Declarative construction of one focused settings page.
pub(crate) fn build(
    teams: AgentTeamsConfig,
    caffeination: AgentCaffeinationConfig,
    menu_bar: MenuBarConfig,
    integrations: AgentIntegrationsConfig,
    available_wrappers: &BTreeSet<String>,
    apply: ApplyAgents,
) -> gtk::Widget {
    let caffeination_available =
        crate::sleep_inhibitor::SleepInhibitorCapability::discover().available();
    let status_item_available = crate::status_notifier::watcher_available();
    eprintln!(
        "zentty-linux: agent-settings loaded teams={} wrappers-available={} source-unavailable={} status-item-available={} caffeination-available={}",
        teams.enabled,
        available_wrappers.len(),
        UNAVAILABLE_PERSISTENT.len(),
        status_item_available,
        caffeination_available,
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

    let state = Rc::new(RefCell::new(State {
        teams,
        caffeination,
        menu_bar,
        integrations,
        apply,
        updating: Cell::new(false),
        status: gtk::Label::new(None),
    }));

    let teams_switch = gtk::Switch::builder()
        .active(teams.enabled)
        .valign(gtk::Align::Center)
        .build();
    teams_switch.set_widget_name("settings-agents-teams");
    instrument_focus(&teams_switch, "teams");
    let behavior = card("Behavior");
    behavior.append(&setting_row(
        "Claude Code agent _teams (experimental)",
        "Expose Tornado TTY's tmux-compatible team environment to newly created panes.",
        &teams_switch,
    ));
    let status_item_switch = gtk::Switch::builder()
        .active(menu_bar.show_status_item)
        .sensitive(status_item_available)
        .valign(gtk::Align::Center)
        .build();
    status_item_switch.set_widget_name("settings-agents-status-item");
    instrument_focus(&status_item_switch, "status-item");
    behavior.append(&setting_row(
        "Show agent status in s_ystem tray",
        if status_item_available {
            "Publish the same in-window fleet through the desktop StatusNotifierItem host."
        } else {
            "Unavailable: no desktop StatusNotifierItem watcher. The in-window fleet remains available."
        },
        &status_item_switch,
    ));
    let caffeination_switch = gtk::Switch::builder()
        .active(caffeination.enabled)
        .sensitive(caffeination_available)
        .valign(gtk::Align::Center)
        .build();
    caffeination_switch.set_widget_name("settings-agents-caffeination");
    instrument_focus(&caffeination_switch, "caffeination");
    behavior.append(&setting_row(
        "Prevent _sleep while agents run",
        if caffeination_available {
            "Block system sleep while a recognized agent is running. The display may still sleep."
        } else {
            "Unavailable: systemd-logind's inhibitor interface was not found. Requested state is retained."
        },
        &caffeination_switch,
    ));
    root.append(&behavior);

    let integrations_box = card("Agent integrations");
    for (id, name) in AVAILABLE_INTEGRATIONS {
        let available = available_wrappers.contains(id);
        let configured = effective_state(&state.borrow().integrations, id, false);
        let toggle = gtk::Switch::builder()
            .active(configured != AgentIntegrationState::Off)
            .sensitive(available)
            .valign(gtk::Align::Center)
            .build();
        toggle.set_widget_name(&format!("settings-agents-integration-{id}"));
        instrument_focus(&toggle, id);
        let detail = if available {
            integration_detail(configured, false)
        } else {
            "Unavailable: the staged wrapper or real agent executable was not found. Requested state is retained."
        };
        eprintln!(
            "zentty-linux: agent-settings integration={id} requested={} observed={} class=ephemeral",
            configured.config_value(),
            if available {
                "wrapper-ready"
            } else {
                "unavailable"
            }
        );
        integrations_box.append(&setting_row(name, detail, &toggle));
        let state = Rc::clone(&state);
        toggle.connect_active_notify(move |toggle| {
            if state.borrow().updating.get() {
                return;
            }
            let requested = if toggle.is_active() {
                AgentIntegrationState::On
            } else {
                AgentIntegrationState::Off
            };
            apply_integration(&state, toggle, id, requested);
        });
    }
    for (id, name) in UNAVAILABLE_PERSISTENT {
        let configured = effective_state(&state.borrow().integrations, id, true);
        eprintln!(
            "zentty-linux: agent-settings integration={id} requested={} observed=unavailable class=persistent consent=required-before-install",
            configured.config_value()
        );
        integrations_box.append(&unavailable_row(
            name,
            configured == AgentIntegrationState::On,
            integration_detail(configured, true),
            &format!("settings-agents-integration-{id}"),
        ));
    }
    root.append(&integrations_box);

    {
        let state = Rc::clone(&state);
        teams_switch.connect_active_notify(move |control| {
            if state.borrow().updating.get() {
                return;
            }
            apply_teams(&state, control);
        });
    }
    {
        let state = Rc::clone(&state);
        status_item_switch.connect_active_notify(move |control| {
            if state.borrow().updating.get() {
                return;
            }
            apply_status_item(&state, control);
        });
    }
    {
        let state = Rc::clone(&state);
        caffeination_switch.connect_active_notify(move |control| {
            if state.borrow().updating.get() {
                return;
            }
            apply_caffeination(&state, control);
        });
    }

    let status = state.borrow().status.clone();
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&root)
        .build();
    scroll.update_property(&[gtk::accessible::Property::Label("Agents Settings")]);
    scroll.upcast()
}

fn apply_status_item(state: &Rc<RefCell<State>>, control: &gtk::Switch) {
    let (teams, caffeination, accepted, integrations, apply) = {
        let state = state.borrow();
        (
            state.teams,
            state.caffeination,
            state.menu_bar,
            state.integrations.clone(),
            Rc::clone(&state.apply),
        )
    };
    let requested = MenuBarConfig {
        show_status_item: control.is_active(),
    };
    if !has_changed(&accepted, &requested) {
        return;
    }
    match apply(teams, caffeination, requested, integrations) {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.menu_bar = requested;
            state.status.set_text("");
            eprintln!(
                "zentty-linux: agent-settings control=status-item enabled={} result=applied",
                requested.show_status_item
            );
        }
        Err(error) => {
            rollback_switch(state, control, accepted.show_status_item);
            report_error(state, "status-item", &error);
        }
    }
}

fn effective_state(
    integrations: &AgentIntegrationsConfig,
    id: &str,
    persistent: bool,
) -> AgentIntegrationState {
    integrations
        .states
        .get(id)
        .copied()
        .unwrap_or(if persistent {
            AgentIntegrationState::Ask
        } else {
            AgentIntegrationState::On
        })
}

fn integration_detail(state: AgentIntegrationState, persistent: bool) -> &'static str {
    match (persistent, state) {
        (true, AgentIntegrationState::Ask) => {
            "Unavailable on Linux. Consent will be required before any future hook installation. Requested state: ask."
        }
        (true, AgentIntegrationState::On) => {
            "Unavailable on Linux; requested enabled, but no installed hooks are claimed."
        }
        (true, AgentIntegrationState::Off) => {
            "Unavailable on Linux; requested disabled and no installed hooks are claimed."
        }
        (false, AgentIntegrationState::On | AgentIntegrationState::Ask) => {
            "Built in and enabled for newly created panes through Tornado TTY's authenticated wrapper."
        }
        (false, AgentIntegrationState::Off) => {
            "Built in but disabled; new panes use the agent executable directly."
        }
    }
}

fn has_changed<T: PartialEq>(accepted: &T, requested: &T) -> bool {
    accepted != requested
}

fn apply_teams(state: &Rc<RefCell<State>>, control: &gtk::Switch) {
    let (accepted, caffeination, menu_bar, integrations, apply) = {
        let state = state.borrow();
        (
            state.teams,
            state.caffeination,
            state.menu_bar,
            state.integrations.clone(),
            Rc::clone(&state.apply),
        )
    };
    let requested = AgentTeamsConfig {
        enabled: control.is_active(),
    };
    if !has_changed(&accepted, &requested) {
        return;
    }
    match apply(requested, caffeination, menu_bar, integrations) {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.teams = requested;
            state.status.set_text("");
            eprintln!("zentty-linux: agent-settings control=teams result=applied");
        }
        Err(error) => {
            rollback_switch(state, control, accepted.enabled);
            report_error(state, "teams", &error);
        }
    }
}

fn apply_caffeination(state: &Rc<RefCell<State>>, control: &gtk::Switch) {
    let (teams, accepted, menu_bar, integrations, apply) = {
        let state = state.borrow();
        (
            state.teams,
            state.caffeination,
            state.menu_bar,
            state.integrations.clone(),
            Rc::clone(&state.apply),
        )
    };
    let requested = AgentCaffeinationConfig {
        enabled: control.is_active(),
    };
    if !has_changed(&accepted, &requested) {
        return;
    }
    match apply(teams, requested, menu_bar, integrations) {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.caffeination = requested;
            state.status.set_text("");
            eprintln!(
                "zentty-linux: agent-settings control=caffeination enabled={} result=applied",
                requested.enabled
            );
        }
        Err(error) => {
            rollback_switch(state, control, accepted.enabled);
            report_error(state, "caffeination", &error);
        }
    }
}

fn apply_integration(
    state: &Rc<RefCell<State>>,
    control: &gtk::Switch,
    id: &str,
    requested: AgentIntegrationState,
) {
    let (teams, caffeination, menu_bar, mut integrations, accepted, apply) = {
        let state = state.borrow();
        (
            state.teams,
            state.caffeination,
            state.menu_bar,
            state.integrations.clone(),
            effective_state(&state.integrations, id, false),
            Rc::clone(&state.apply),
        )
    };
    if !has_changed(&accepted, &requested) {
        return;
    }
    integrations.states.insert(id.to_owned(), requested);
    match apply(teams, caffeination, menu_bar, integrations.clone()) {
        Ok(()) => {
            let mut state = state.borrow_mut();
            state.integrations = integrations;
            state.status.set_text("");
            eprintln!(
                "zentty-linux: agent-settings control=integration id={id} requested={} observed=wrapper-configured result=applied",
                requested.config_value()
            );
        }
        Err(error) => {
            rollback_switch(state, control, accepted != AgentIntegrationState::Off);
            report_error(state, id, &error);
        }
    }
}

fn rollback_switch(state: &Rc<RefCell<State>>, control: &gtk::Switch, accepted: bool) {
    state.borrow().updating.set(true);
    control.set_active(accepted);
    state.borrow().updating.set(false);
}

fn report_error(state: &Rc<RefCell<State>>, control: &str, error: &str) {
    state
        .borrow()
        .status
        .set_text(&format!("Could not save Agents settings: {error}"));
    eprintln!("zentty-linux: agent-settings control={control} result=error detail={error}");
}

fn unavailable_row(title: &str, configured: bool, reason: &str, widget_name: &str) -> gtk::Widget {
    let control = gtk::Switch::builder()
        .active(configured)
        .sensitive(false)
        .valign(gtk::Align::Center)
        .build();
    control.set_widget_name(widget_name);
    control.update_property(&[gtk::accessible::Property::Description(reason)]);
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
    title.set_use_underline(true);
    title.set_mnemonic_widget(Some(control));
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

fn instrument_focus(control: &impl IsA<gtk::Widget>, name: &str) {
    let focus = gtk::EventControllerFocus::new();
    let name = name.to_owned();
    focus.connect_enter(move |_| {
        eprintln!("zentty-linux: agent-settings focus={name}");
    });
    control.add_controller(focus);
}

#[cfg(test)]
mod tests {
    use super::{
        AVAILABLE_INTEGRATIONS, UNAVAILABLE_PERSISTENT, effective_state, has_changed,
        integration_detail,
    };
    use std::collections::BTreeMap;
    use zentty_core::{AgentIntegrationState, AgentIntegrationsConfig};

    #[test]
    fn source_agent_inventory_never_disappears_silently() {
        assert_eq!(
            AVAILABLE_INTEGRATIONS.map(|(id, _)| id),
            [
                "claude",
                "codex",
                "copilot",
                "gemini",
                "opencode",
                "pi",
                "omp",
                "small-harness"
            ]
        );
        assert!(UNAVAILABLE_PERSISTENT.iter().any(|(id, _)| *id == "amp"));
    }

    #[test]
    fn unset_integration_state_uses_the_source_class_default() {
        let integrations = AgentIntegrationsConfig {
            states: BTreeMap::new(),
            grandfathered_v1: false,
        };

        assert_eq!(
            effective_state(&integrations, "amp", true),
            AgentIntegrationState::Ask
        );
        assert_eq!(
            effective_state(&integrations, "codex", false),
            AgentIntegrationState::On
        );
    }

    #[test]
    fn stored_state_always_overrides_the_class_default() {
        let integrations = AgentIntegrationsConfig {
            states: BTreeMap::from([
                ("amp".to_owned(), AgentIntegrationState::On),
                ("codex".to_owned(), AgentIntegrationState::Off),
            ]),
            grandfathered_v1: false,
        };

        assert_eq!(
            effective_state(&integrations, "amp", true),
            AgentIntegrationState::On
        );
        assert_eq!(
            effective_state(&integrations, "codex", false),
            AgentIntegrationState::Off
        );
    }

    #[test]
    fn persistent_requested_on_never_claims_unobserved_hooks_are_installed() {
        let detail = integration_detail(AgentIntegrationState::On, true);

        assert!(detail.contains("no installed hooks are claimed"));
        assert!(!detail.contains("Built in"));
    }

    #[test]
    fn ephemeral_state_describes_the_actual_new_pane_effect() {
        assert!(
            integration_detail(AgentIntegrationState::On, false).contains("authenticated wrapper")
        );
        assert!(
            integration_detail(AgentIntegrationState::Off, false)
                .contains("agent executable directly")
        );
    }

    #[test]
    fn idempotent_notifications_do_not_repeat_a_settings_write() {
        assert!(!has_changed(&true, &true));
        assert!(has_changed(&true, &false));
        assert!(!has_changed(
            &AgentIntegrationState::On,
            &AgentIntegrationState::On
        ));
        assert!(has_changed(
            &AgentIntegrationState::On,
            &AgentIntegrationState::Off
        ));
    }
}
