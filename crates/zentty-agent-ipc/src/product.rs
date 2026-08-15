use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductIpcKind {
    Discover,
    Pane,
}

impl ProductIpcKind {
    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Pane => "pane",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProductIpcRequest {
    kind: ProductIpcKind,
    subcommand: String,
    arguments: Vec<String>,
}

impl ProductIpcRequest {
    pub const MAX_ARGUMENTS: usize = 128;
    pub const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
    pub const MAX_TOTAL_ARGUMENT_BYTES: usize = 128 * 1024;

    /// Creates and validates a bounded product request.
    ///
    /// # Errors
    ///
    /// Returns [`ProductIpcError`] for an unsupported route or an argument
    /// count/size above the documented transport ceiling.
    pub fn new(
        kind: ProductIpcKind,
        subcommand: impl Into<String>,
        arguments: Vec<String>,
    ) -> Result<Self, ProductIpcError> {
        let subcommand = subcommand.into();
        let allowed = match kind {
            ProductIpcKind::Discover => [
                "overview",
                "windows",
                "worklanes",
                "panes",
                "panes-current-worklane",
                "select-pane",
            ]
            .as_slice(),
            ProductIpcKind::Pane => [
                "split",
                "grid",
                "focus",
                "pane-rename",
                "close",
                "worklane-color",
                "worklane-rename",
                "zoom",
                "resize",
                "layout",
                "theme",
                "notify",
            ]
            .as_slice(),
        };
        if !allowed.contains(&subcommand.as_str()) {
            return Err(ProductIpcError::InvalidCommand(format!(
                "unsupported {} command {subcommand:?}",
                kind.wire_name()
            )));
        }
        if arguments.len() > Self::MAX_ARGUMENTS {
            return Err(ProductIpcError::InvalidCommand(format!(
                "product command exceeds {} arguments",
                Self::MAX_ARGUMENTS
            )));
        }
        let mut total = 0_usize;
        for argument in &arguments {
            if argument.len() > Self::MAX_ARGUMENT_BYTES {
                return Err(ProductIpcError::InvalidCommand(format!(
                    "product argument exceeds {} bytes",
                    Self::MAX_ARGUMENT_BYTES
                )));
            }
            total = total.saturating_add(argument.len());
        }
        if total > Self::MAX_TOTAL_ARGUMENT_BYTES {
            return Err(ProductIpcError::InvalidCommand(format!(
                "product arguments exceed {} bytes",
                Self::MAX_TOTAL_ARGUMENT_BYTES
            )));
        }
        Ok(Self {
            kind,
            subcommand,
            arguments,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> ProductIpcKind {
        self.kind
    }

    #[must_use]
    pub fn subcommand(&self) -> &str {
        &self.subcommand
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProductIpcReply {
    stdout: Option<String>,
    error: Option<ProductIpcReplyError>,
}

impl ProductIpcReply {
    pub const MAX_STDOUT_BYTES: usize = 256 * 1024;
    pub const MAX_ERROR_BYTES: usize = 4 * 1024;

    /// Creates a bounded successful reply.
    ///
    /// # Errors
    ///
    /// Returns [`ProductIpcError`] when stdout exceeds the protocol ceiling.
    pub fn success(stdout: impl Into<String>) -> Result<Self, ProductIpcError> {
        let stdout = stdout.into();
        if stdout.len() > Self::MAX_STDOUT_BYTES {
            return Err(ProductIpcError::InvalidReply(format!(
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
    /// Returns [`ProductIpcError`] for an invalid code or oversized message.
    pub fn failure(
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ProductIpcError> {
        let code = code.into();
        let message = message.into();
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(ProductIpcError::InvalidReply(
                "product error code is invalid".to_owned(),
            ));
        }
        if message.len() > Self::MAX_ERROR_BYTES {
            return Err(ProductIpcError::InvalidReply(format!(
                "product error exceeds {} bytes",
                Self::MAX_ERROR_BYTES
            )));
        }
        Ok(Self {
            stdout: None,
            error: Some(ProductIpcReplyError { code, message }),
        })
    }

    #[must_use]
    pub fn stdout(&self) -> Option<&str> {
        self.stdout.as_deref()
    }

    #[must_use]
    pub const fn error(&self) -> Option<&ProductIpcReplyError> {
        self.error.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProductIpcReplyError {
    code: String,
    message: String,
}

impl ProductIpcReplyError {
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
pub enum ProductIpcError {
    InvalidCommand(String),
    InvalidReply(String),
}

impl fmt::Display for ProductIpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommand(message) => {
                write!(formatter, "invalid product command: {message}")
            }
            Self::InvalidReply(message) => write!(formatter, "invalid product reply: {message}"),
        }
    }
}

impl std::error::Error for ProductIpcError {}
