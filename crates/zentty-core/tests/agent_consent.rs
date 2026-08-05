use zentty_core::{
    AgentIntegrationClass, AgentIntegrationGate, AgentIntegrationState, resolve_integration_gate,
};

#[test]
fn ephemeral_adapters_are_on_by_default_and_never_install_persistent_hooks() {
    assert_eq!(
        resolve_integration_gate(AgentIntegrationClass::Ephemeral, None, false),
        AgentIntegrationGate::Proceed
    );
    assert_eq!(
        resolve_integration_gate(
            AgentIntegrationClass::Ephemeral,
            Some(AgentIntegrationState::Ask),
            false,
        ),
        AgentIntegrationGate::Proceed
    );
    assert_eq!(
        resolve_integration_gate(
            AgentIntegrationClass::Ephemeral,
            Some(AgentIntegrationState::Off),
            false,
        ),
        AgentIntegrationGate::Off
    );
}

#[test]
fn persistent_hooks_require_consent_and_restores_degrade_without_consuming_it() {
    assert_eq!(
        resolve_integration_gate(AgentIntegrationClass::Persistent, None, false),
        AgentIntegrationGate::NeedsConsent
    );
    assert_eq!(
        resolve_integration_gate(AgentIntegrationClass::Persistent, None, true),
        AgentIntegrationGate::SuppressedByRestore
    );
    assert_eq!(
        resolve_integration_gate(
            AgentIntegrationClass::Persistent,
            Some(AgentIntegrationState::On),
            true,
        ),
        AgentIntegrationGate::Proceed
    );
}
