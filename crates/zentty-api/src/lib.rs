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
    stdout: Option<String>,
    error: Option<ApplicationReplyError>,
}

impl ApplicationReply {
    pub const MAX_STDOUT_BYTES: usize = 256 * 1024;
    pub const MAX_ERROR_BYTES: usize = 4 * 1024;

    /// Creates a bounded successful reply.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationApiError`] when stdout exceeds the protocol ceiling.
    pub fn success(stdout: impl Into<String>) -> Result<Self, ApplicationApiError> {
        let stdout = stdout.into();
        if stdout.len() > Self::MAX_STDOUT_BYTES {
            return Err(ApplicationApiError::InvalidReply(format!(
                "product output exceeds {} bytes",
                Self::MAX_STDOUT_BYTES
            )));
        }
        Ok(Self {
            stdout: Some(stdout),
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
            stdout: None,
            error: Some(ApplicationReplyError { code, message }),
        })
    }

    #[must_use]
    pub fn stdout(&self) -> Option<&str> {
        self.stdout.as_deref()
    }

    #[must_use]
    pub const fn error(&self) -> Option<&ApplicationReplyError> {
        self.error.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationReplyError {
    code: String,
    message: String,
}

impl ApplicationReplyError {
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
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
        APPLICATION_API_VERSION, ApplicationOperation, ApplicationRequest, ApplicationScope,
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
}
