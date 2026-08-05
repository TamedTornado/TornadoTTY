use crate::{Command, CommandError};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxCompatRequest {
    command: Command,
    arguments: Vec<String>,
    standard_input: Option<String>,
}

impl TmuxCompatRequest {
    pub const VERSION: u32 = 1;
    pub const MAX_ARGUMENTS: usize = 256;
    pub const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
    pub const MAX_ARGUMENT_TOTAL_BYTES: usize = 64 * 1024;
    pub const MAX_STANDARD_INPUT_BYTES: usize = 256 * 1024;

    /// Validates a decoded IPC payload and canonicalizes its command.
    ///
    /// # Errors
    ///
    /// Rejects unsupported versions or commands and any bounded field that
    /// exceeds the compatibility protocol limits.
    pub fn new(
        version: u32,
        subcommand: &str,
        arguments: Vec<String>,
        standard_input: Option<String>,
    ) -> Result<Self, ProtocolError> {
        if version != Self::VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }
        if arguments.len() > Self::MAX_ARGUMENTS {
            return Err(ProtocolError::LimitExceeded("argument count"));
        }
        if arguments
            .iter()
            .any(|argument| argument.len() > Self::MAX_ARGUMENT_BYTES)
        {
            return Err(ProtocolError::LimitExceeded("argument bytes"));
        }
        if arguments.iter().map(String::len).sum::<usize>() > Self::MAX_ARGUMENT_TOTAL_BYTES {
            return Err(ProtocolError::LimitExceeded("total argument bytes"));
        }
        if standard_input
            .as_ref()
            .is_some_and(|input| input.len() > Self::MAX_STANDARD_INPUT_BYTES)
        {
            return Err(ProtocolError::LimitExceeded("standard input bytes"));
        }
        Ok(Self {
            command: Command::parse(subcommand)?,
            arguments,
            standard_input,
        })
    }

    #[must_use]
    pub fn command(&self) -> Command {
        self.command
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    #[must_use]
    pub fn standard_input(&self) -> Option<&str> {
        self.standard_input.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxCompatReply {
    payload: ReplyPayload,
}

impl TmuxCompatReply {
    pub const VERSION: u32 = 1;
    pub const MAX_STDOUT_BYTES: usize = 256 * 1024;
    pub const MAX_ERROR_CODE_BYTES: usize = 64;
    pub const MAX_ERROR_MESSAGE_BYTES: usize = 16 * 1024;

    /// Constructs a bounded successful response.
    ///
    /// # Errors
    ///
    /// Rejects stdout larger than the compatibility response limit.
    pub fn success(stdout: String) -> Result<Self, ProtocolError> {
        if stdout.len() > Self::MAX_STDOUT_BYTES {
            return Err(ProtocolError::LimitExceeded("stdout bytes"));
        }
        Ok(Self {
            payload: ReplyPayload::Success(stdout),
        })
    }

    /// Constructs a bounded failed response.
    ///
    /// # Errors
    ///
    /// Rejects oversized or empty diagnostic fields.
    pub fn failure(
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ProtocolError> {
        let code = code.into();
        let message = message.into();
        if code.is_empty() || code.len() > Self::MAX_ERROR_CODE_BYTES {
            return Err(ProtocolError::LimitExceeded("error code bytes"));
        }
        if message.is_empty() || message.len() > Self::MAX_ERROR_MESSAGE_BYTES {
            return Err(ProtocolError::LimitExceeded("error message bytes"));
        }
        Ok(Self {
            payload: ReplyPayload::Failure(TmuxCompatReplyError { code, message }),
        })
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self.payload, ReplyPayload::Success(_))
    }

    #[must_use]
    pub fn stdout(&self) -> Option<&str> {
        match &self.payload {
            ReplyPayload::Success(stdout) => Some(stdout),
            ReplyPayload::Failure(_) => None,
        }
    }

    #[must_use]
    pub fn error(&self) -> Option<&TmuxCompatReplyError> {
        match &self.payload {
            ReplyPayload::Success(_) => None,
            ReplyPayload::Failure(error) => Some(error),
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> u8 {
        u8::from(!self.is_ok())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReplyPayload {
    Success(String),
    Failure(TmuxCompatReplyError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxCompatReplyError {
    code: String,
    message: String,
}

impl TmuxCompatReplyError {
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
pub enum ProtocolError {
    UnsupportedVersion(u32),
    UnsupportedCommand(CommandError),
    LimitExceeded(&'static str),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported tmux compatibility protocol version: {version}"
                )
            }
            Self::UnsupportedCommand(error) => error.fmt(formatter),
            Self::LimitExceeded(field) => {
                write!(formatter, "tmux compatibility {field} limit exceeded")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<CommandError> for ProtocolError {
    fn from(error: CommandError) -> Self {
        Self::UnsupportedCommand(error)
    }
}
