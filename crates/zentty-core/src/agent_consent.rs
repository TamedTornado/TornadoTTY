#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationState {
    Ask,
    On,
    Off,
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
