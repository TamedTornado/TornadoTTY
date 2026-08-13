#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationState {
    Ask,
    On,
    Off,
}

impl AgentIntegrationState {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub(crate) fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "ask" => Some(Self::Ask),
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationClass {
    Persistent,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationGate {
    Proceed,
    Off,
    SuppressedByRestore,
    NeedsConsent,
}

#[must_use]
pub fn resolve_integration_gate(
    class: AgentIntegrationClass,
    stored_state: Option<AgentIntegrationState>,
    is_restore: bool,
) -> AgentIntegrationGate {
    let effective = stored_state.unwrap_or(match class {
        AgentIntegrationClass::Persistent => AgentIntegrationState::Ask,
        AgentIntegrationClass::Ephemeral => AgentIntegrationState::On,
    });
    match effective {
        AgentIntegrationState::On => AgentIntegrationGate::Proceed,
        AgentIntegrationState::Off => AgentIntegrationGate::Off,
        AgentIntegrationState::Ask if class == AgentIntegrationClass::Ephemeral => {
            AgentIntegrationGate::Proceed
        }
        AgentIntegrationState::Ask if is_restore => AgentIntegrationGate::SuppressedByRestore,
        AgentIntegrationState::Ask => AgentIntegrationGate::NeedsConsent,
    }
}
