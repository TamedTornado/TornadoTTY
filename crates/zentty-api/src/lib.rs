#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Current language-neutral application API version.
pub const APPLICATION_API_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationScope {
    Discover,
    Pane,
}

/// Canonical application target derived by authentication. This is service
/// context, not a client routing claim.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationTarget {
    pub window_id: String,
    pub worklane_id: String,
    pub pane_id: String,
}

impl ApplicationTarget {
    #[must_use]
    pub fn new(
        window_id: impl Into<String>,
        worklane_id: impl Into<String>,
        pane_id: impl Into<String>,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            worklane_id: worklane_id.into(),
            pane_id: pane_id.into(),
        }
    }
}

/// Authority scope established by the transport before service dispatch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationAuthority {
    Pane,
    Instance,
}

impl ApplicationScope {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Pane => "pane",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationOperation {
    Overview,
    Windows,
    Worklanes,
    Panes,
    PanesCurrentWorklane,
    SelectPane,
    Split,
    Grid,
    Focus,
    PaneRename,
    Close,
    WorklaneColor,
    WorklaneRename,
    Zoom,
    Resize,
    Layout,
    Theme,
    Notify,
    ShellSignal,
}

impl ApplicationOperation {
    pub const ALL: [Self; 19] = [
        Self::Overview,
        Self::Windows,
        Self::Worklanes,
        Self::Panes,
        Self::PanesCurrentWorklane,
        Self::SelectPane,
        Self::Split,
        Self::Grid,
        Self::Focus,
        Self::PaneRename,
        Self::Close,
        Self::WorklaneColor,
        Self::WorklaneRename,
        Self::Zoom,
        Self::Resize,
        Self::Layout,
        Self::Theme,
        Self::Notify,
        Self::ShellSignal,
    ];

    #[must_use]
    pub const fn scope(self) -> ApplicationScope {
        match self {
            Self::Overview
            | Self::Windows
            | Self::Worklanes
            | Self::Panes
            | Self::PanesCurrentWorklane
            | Self::SelectPane => ApplicationScope::Discover,
            Self::Split
            | Self::Grid
            | Self::Focus
            | Self::PaneRename
            | Self::Close
            | Self::WorklaneColor
            | Self::WorklaneRename
            | Self::Zoom
            | Self::Resize
            | Self::Layout
            | Self::Theme
            | Self::Notify
            | Self::ShellSignal => ApplicationScope::Pane,
        }
    }

    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Windows => "windows",
            Self::Worklanes => "worklanes",
            Self::Panes => "panes",
            Self::PanesCurrentWorklane => "panes-current-worklane",
            Self::SelectPane => "select-pane",
            Self::Split => "split",
            Self::Grid => "grid",
            Self::Focus => "focus",
            Self::PaneRename => "pane-rename",
            Self::Close => "close",
            Self::WorklaneColor => "worklane-color",
            Self::WorklaneRename => "worklane-rename",
            Self::Zoom => "zoom",
            Self::Resize => "resize",
            Self::Layout => "layout",
            Self::Theme => "theme",
            Self::Notify => "notify",
            Self::ShellSignal => "shell-signal",
        }
    }

    /// Resolves one closed application operation from its transport identity.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationApiError`] when the scope/name pair is not an API
    /// operation.
    pub fn from_wire(scope: ApplicationScope, name: &str) -> Result<Self, ApplicationApiError> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.scope() == scope && operation.wire_name() == name)
            .ok_or_else(|| {
                ApplicationApiError::InvalidCommand(format!(
                    "unsupported {} operation {name:?}",
                    scope.wire_name()
                ))
            })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationRequest {
    operation: ApplicationOperation,
    arguments: Vec<String>,
}

impl ApplicationRequest {
    pub const MAX_ARGUMENTS: usize = 128;
    pub const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
    pub const MAX_TOTAL_ARGUMENT_BYTES: usize = 128 * 1024;

    /// Creates and validates a bounded product request.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationApiError`] for an unsupported route or an argument
    /// count/size above the documented transport ceiling.
    pub fn new(
        kind: ApplicationScope,
        subcommand: impl Into<String>,
        arguments: Vec<String>,
    ) -> Result<Self, ApplicationApiError> {
        let subcommand = subcommand.into();
        let operation = ApplicationOperation::from_wire(kind, &subcommand)?;
        if arguments.len() > Self::MAX_ARGUMENTS {
            return Err(ApplicationApiError::InvalidCommand(format!(
                "product command exceeds {} arguments",
                Self::MAX_ARGUMENTS
            )));
        }
        let mut total = 0_usize;
        for argument in &arguments {
            if argument.len() > Self::MAX_ARGUMENT_BYTES {
                return Err(ApplicationApiError::InvalidCommand(format!(
                    "product argument exceeds {} bytes",
                    Self::MAX_ARGUMENT_BYTES
                )));
            }
            total = total.saturating_add(argument.len());
        }
        if total > Self::MAX_TOTAL_ARGUMENT_BYTES {
            return Err(ApplicationApiError::InvalidCommand(format!(
                "product arguments exceed {} bytes",
                Self::MAX_TOTAL_ARGUMENT_BYTES
            )));
        }
        Ok(Self {
            operation,
            arguments,
        })
    }

    #[must_use]
    pub const fn operation(&self) -> ApplicationOperation {
        self.operation
    }

    #[must_use]
    pub const fn kind(&self) -> ApplicationScope {
        self.operation.scope()
    }

    #[must_use]
    pub fn subcommand(&self) -> &str {
        self.operation.wire_name()
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

pub type ProductIpcKind = ApplicationScope;
pub type ProductIpcRequest = ApplicationRequest;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationReply {
    result: Option<ApplicationResult>,
    error: Option<ApplicationReplyError>,
}

impl ApplicationReply {
    pub const MAX_RESULT_BYTES: usize = 256 * 1024;
    pub const MAX_ERROR_BYTES: usize = 4 * 1024;

    /// Creates a bounded successful structured result.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationApiError`] when the serialized result exceeds the
    /// protocol ceiling.
    pub fn success(result: ApplicationResult) -> Result<Self, ApplicationApiError> {
        let bytes = serde_json::to_vec(&result)
            .map_err(|error| ApplicationApiError::InvalidReply(error.to_string()))?;
        if bytes.len() > Self::MAX_RESULT_BYTES {
            return Err(ApplicationApiError::InvalidReply(format!(
                "application result exceeds {} bytes",
                Self::MAX_RESULT_BYTES
            )));
        }
        Ok(Self {
            result: Some(result),
            error: None,
        })
    }

    /// Creates a bounded machine-readable failure reply.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationApiError`] for an invalid code or oversized message.
    pub fn failure(
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ApplicationApiError> {
        let code = code.into();
        let message = message.into();
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(ApplicationApiError::InvalidReply(
                "product error code is invalid".to_owned(),
            ));
        }
        if message.len() > Self::MAX_ERROR_BYTES {
            return Err(ApplicationApiError::InvalidReply(format!(
                "product error exceeds {} bytes",
                Self::MAX_ERROR_BYTES
            )));
        }
        Ok(Self {
            result: None,
            error: Some(ApplicationReplyError {
                category: ApplicationErrorCategory::from_code(&code),
                code,
                message,
            }),
        })
    }

    #[must_use]
    pub const fn result(&self) -> Option<&ApplicationResult> {
        self.result.as_ref()
    }

    #[must_use]
    pub const fn error(&self) -> Option<&ApplicationReplyError> {
        self.error.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationResultKind {
    Empty,
    Discovery,
    Selection,
    Topology,
    Theme,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationResult {
    kind: ApplicationResultKind,
    value: serde_json::Value,
}

impl ApplicationResult {
    #[must_use]
    pub const fn new(kind: ApplicationResultKind, value: serde_json::Value) -> Self {
        Self { kind, value }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            kind: ApplicationResultKind::Empty,
            value: serde_json::Value::Null,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ApplicationResultKind {
        self.kind
    }

    #[must_use]
    pub const fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationReplyError {
    category: ApplicationErrorCategory,
    code: String,
    message: String,
}

impl ApplicationReplyError {
    #[must_use]
    pub const fn category(&self) -> ApplicationErrorCategory {
        self.category
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationErrorCategory {
    InvalidArguments,
    UnsupportedOperation,
    UnsupportedVersion,
    AuthorizationFailure,
    StaleTarget,
    StaleInstance,
    RetryableInstanceReplacement,
    ProductUnavailable,
    ProductRejection,
    PermanentTransportFailure,
}

impl ApplicationErrorCategory {
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "invalid_command" | "invalid_request" => Self::InvalidArguments,
            "unsupported" | "unsupported_command" => Self::UnsupportedOperation,
            "unsupported_version" => Self::UnsupportedVersion,
            "authorization_failed" | "unauthorized_target" => Self::AuthorizationFailure,
            "stale_target" => Self::StaleTarget,
            "stale_instance" => Self::StaleInstance,
            "instance_replaced" => Self::RetryableInstanceReplacement,
            "application_unavailable" => Self::ProductUnavailable,
            "permanent_transport_failure" => Self::PermanentTransportFailure,
            _ => Self::ProductRejection,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationApiError {
    InvalidCommand(String),
    InvalidReply(String),
}

impl fmt::Display for ApplicationApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommand(message) => {
                write!(formatter, "invalid application request: {message}")
            }
            Self::InvalidReply(message) => write!(formatter, "invalid product reply: {message}"),
        }
    }
}

impl std::error::Error for ApplicationApiError {}

pub type ProductIpcReply = ApplicationReply;
pub type ProductIpcReplyError = ApplicationReplyError;
pub type ProductIpcError = ApplicationApiError;

#[cfg(test)]
mod tests {
    use super::{
        APPLICATION_API_VERSION, ApplicationAuthority, ApplicationErrorCategory,
        ApplicationOperation, ApplicationReply, ApplicationRequest, ApplicationResult,
        ApplicationResultKind, ApplicationScope, ApplicationTarget,
    };
    use std::collections::HashSet;

    #[derive(serde::Deserialize)]
    struct Inventory {
        api_version: u32,
        operations: Vec<InventoryOperation>,
    }

    #[derive(serde::Deserialize)]
    struct InventoryOperation {
        id: String,
        scope: ApplicationScope,
        wire_name: String,
    }

    #[test]
    fn operation_registry_is_closed_unique_and_round_trips_wire_identity() {
        assert_eq!(APPLICATION_API_VERSION, 1);
        assert_eq!(ApplicationOperation::ALL.len(), 19);
        let mut identities = HashSet::new();
        for operation in ApplicationOperation::ALL {
            let identity = (operation.scope(), operation.wire_name());
            assert!(identities.insert(identity));
            assert_eq!(
                ApplicationOperation::from_wire(identity.0, identity.1).unwrap(),
                operation
            );
        }
        assert_eq!(
            identities
                .iter()
                .filter(|(scope, _)| *scope == ApplicationScope::Discover)
                .count(),
            6
        );
        assert_eq!(
            identities
                .iter()
                .filter(|(scope, _)| *scope == ApplicationScope::Pane)
                .count(),
            13
        );
    }

    #[test]
    fn request_derives_scope_and_name_from_the_closed_operation() {
        let request = ApplicationRequest::new(
            ApplicationScope::Pane,
            "split",
            vec!["right".to_owned(), "--equal".to_owned()],
        )
        .unwrap();
        assert_eq!(request.operation(), ApplicationOperation::Split);
        assert_eq!(request.kind(), ApplicationScope::Pane);
        assert_eq!(request.subcommand(), "split");
        assert_eq!(request.arguments(), ["right", "--equal"]);

        assert!(ApplicationRequest::new(ApplicationScope::Discover, "split", Vec::new()).is_err());
        assert!(ApplicationRequest::new(ApplicationScope::Pane, "panes", Vec::new()).is_err());
    }

    #[test]
    fn canonical_target_and_authority_are_language_neutral_service_context() {
        let target = ApplicationTarget::new("window-1", "worklane-2", "pane-3");
        assert_eq!(
            serde_json::to_value(&target).unwrap(),
            serde_json::json!({
                "windowId":"window-1",
                "worklaneId":"worklane-2",
                "paneId":"pane-3"
            })
        );
        assert_eq!(
            serde_json::to_value(ApplicationAuthority::Pane).unwrap(),
            serde_json::json!("pane")
        );
        assert_eq!(
            serde_json::to_value(ApplicationAuthority::Instance).unwrap(),
            serde_json::json!("instance")
        );
    }

    #[test]
    fn machine_inventory_matches_the_compiled_operation_registry_exactly() {
        let inventory: Inventory = serde_json::from_str(include_str!(
            "../../../docs/architecture/zentty-application-api-v1.json"
        ))
        .unwrap();
        assert_eq!(inventory.api_version, APPLICATION_API_VERSION);
        let documented = inventory
            .operations
            .into_iter()
            .map(|operation| (operation.id, operation.scope, operation.wire_name))
            .collect::<HashSet<_>>();
        let compiled = ApplicationOperation::ALL
            .into_iter()
            .map(|operation| {
                (
                    operation.wire_name().to_owned(),
                    operation.scope(),
                    operation.wire_name().to_owned(),
                )
            })
            .collect::<HashSet<_>>();
        assert_eq!(documented, compiled);
    }

    #[test]
    fn reply_errors_have_a_stable_machine_category_independent_of_prose() {
        for (code, expected) in [
            (
                "invalid_request",
                ApplicationErrorCategory::InvalidArguments,
            ),
            (
                "unsupported_command",
                ApplicationErrorCategory::UnsupportedOperation,
            ),
            (
                "unsupported_version",
                ApplicationErrorCategory::UnsupportedVersion,
            ),
            (
                "unauthorized_target",
                ApplicationErrorCategory::AuthorizationFailure,
            ),
            ("stale_target", ApplicationErrorCategory::StaleTarget),
            ("stale_instance", ApplicationErrorCategory::StaleInstance),
            (
                "instance_replaced",
                ApplicationErrorCategory::RetryableInstanceReplacement,
            ),
            (
                "application_unavailable",
                ApplicationErrorCategory::ProductUnavailable,
            ),
            ("grid_failed", ApplicationErrorCategory::ProductRejection),
            (
                "permanent_transport_failure",
                ApplicationErrorCategory::PermanentTransportFailure,
            ),
        ] {
            let reply = ApplicationReply::failure(code, "prose may change").unwrap();
            assert_eq!(reply.error().unwrap().category(), expected, "{code}");
        }
    }

    #[test]
    fn successful_replies_carry_one_closed_structured_result() {
        let result = ApplicationResult::new(
            ApplicationResultKind::Topology,
            serde_json::json!({"createdPaneIDs":["pane-2"]}),
        );
        let reply = ApplicationReply::success(result.clone()).unwrap();
        assert_eq!(reply.result(), Some(&result));
        assert!(reply.error().is_none());
        assert_eq!(
            serde_json::to_value(reply).unwrap(),
            serde_json::json!({
                "result": {
                    "kind": "topology",
                    "value": {"createdPaneIDs":["pane-2"]}
                },
                "error": null
            })
        );
        let empty_theme = ApplicationResult::new(
            ApplicationResultKind::Theme,
            serde_json::Value::String(String::new()),
        );
        let overhead = serde_json::to_vec(&empty_theme).unwrap().len();
        let boundary = ApplicationResult::new(
            ApplicationResultKind::Theme,
            serde_json::Value::String("x".repeat(ApplicationReply::MAX_RESULT_BYTES - overhead)),
        );
        assert_eq!(
            serde_json::to_vec(&boundary).unwrap().len(),
            ApplicationReply::MAX_RESULT_BYTES
        );
        assert!(ApplicationReply::success(boundary).is_ok());
        assert!(
            ApplicationReply::success(ApplicationResult::new(
                ApplicationResultKind::Theme,
                serde_json::Value::String(
                    "x".repeat(ApplicationReply::MAX_RESULT_BYTES - overhead + 1)
                ),
            ))
            .is_err()
        );
    }

    #[test]
    fn reply_error_code_and_message_boundaries_are_exact() {
        assert!(ApplicationReply::failure("a".repeat(64), "x".repeat(4096)).is_ok());
        assert!(ApplicationReply::failure("", "valid message").is_err());
        assert!(ApplicationReply::failure("a".repeat(65), "valid message").is_err());
        assert!(ApplicationReply::failure("Uppercase", "valid message").is_err());
        assert!(ApplicationReply::failure("valid_code", "x".repeat(4097)).is_err());
    }
}
