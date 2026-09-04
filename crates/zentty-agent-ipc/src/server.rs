use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerCommand {
    Set {
        raw_url: String,
        pid: Option<u32>,
        json: bool,
    },
    Clear {
        json: bool,
    },
    List {
        json: bool,
    },
    Open {
        raw_url: Option<String>,
        browser: Option<String>,
        json: bool,
    },
    WatchSet {
        raw_url: String,
        pid: Option<u32>,
        json: bool,
    },
    WatchClear {
        json: bool,
    },
    Watch {
        command: Vec<String>,
    },
}

impl ServerCommand {
    /// Parses the source-compatible `zentty server` argument surface.
    ///
    /// # Errors
    ///
    /// Rejects missing or unknown subcommands, malformed options, invalid
    /// process identifiers, surplus positional values, and empty watch argv.
    pub fn parse(arguments: &[String]) -> Result<Self, ServerIpcError> {
        let (subcommand, trailing) = arguments
            .split_first()
            .ok_or_else(|| ServerIpcError::Invalid("missing server subcommand".to_owned()))?;
        match subcommand.as_str() {
            "set" => parse_set(trailing, false),
            "watch-set" => parse_set(trailing, true),
            "clear" => parse_no_argument(trailing, false, false),
            "list" => parse_no_argument(trailing, true, false),
            "watch-clear" => parse_no_argument(trailing, false, true),
            "open" => parse_open(trailing),
            "watch" => {
                let command = trailing
                    .strip_prefix(&["--".to_owned()])
                    .unwrap_or(trailing)
                    .to_vec();
                if command.is_empty() {
                    Err(ServerIpcError::Invalid(
                        "missing command after tornadotty-cli server watch --".to_owned(),
                    ))
                } else {
                    Ok(Self::Watch { command })
                }
            }
            _ => Err(ServerIpcError::Invalid(format!(
                "unsupported server subcommand: {subcommand}"
            ))),
        }
    }

    #[must_use]
    pub fn route(&self) -> Option<&'static str> {
        match self {
            Self::Set { .. } => Some("server-set"),
            Self::Clear { .. } => Some("server-clear"),
            Self::List { .. } => Some("server-list"),
            Self::Open { .. } => Some("server-open"),
            Self::WatchSet { .. } => Some("server-watch-set"),
            Self::WatchClear { .. } => Some("server-watch-clear"),
            Self::Watch { .. } => None,
        }
    }

    #[must_use]
    pub const fn json(&self) -> bool {
        match self {
            Self::Set { json, .. }
            | Self::Clear { json }
            | Self::List { json }
            | Self::Open { json, .. }
            | Self::WatchSet { json, .. }
            | Self::WatchClear { json } => *json,
            Self::Watch { .. } => false,
        }
    }

    #[must_use]
    pub fn ipc_arguments(&self) -> Vec<String> {
        let mut arguments = match self {
            Self::Set { raw_url, pid, .. } | Self::WatchSet { raw_url, pid, .. } => {
                let mut arguments = vec![raw_url.clone()];
                if let Some(pid) = pid {
                    arguments.extend(["--pid".to_owned(), pid.to_string()]);
                }
                arguments
            }
            Self::Clear { .. } | Self::List { .. } | Self::WatchClear { .. } => Vec::new(),
            Self::Open {
                raw_url, browser, ..
            } => {
                let mut arguments = raw_url.iter().cloned().collect::<Vec<_>>();
                if let Some(browser) = browser {
                    arguments.extend(["--browser".to_owned(), browser.clone()]);
                }
                arguments
            }
            Self::Watch { command } => return command.clone(),
        };
        if self.json() {
            arguments.push("--json".to_owned());
        }
        arguments
    }
}

fn parse_set(arguments: &[String], watch: bool) -> Result<ServerCommand, ServerIpcError> {
    let mut raw_url = None;
    let mut pid = None;
    let mut json = false;
    let mut saw_pid = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--pid" => {
                if saw_pid {
                    return Err(ServerIpcError::Invalid("duplicate option --pid".to_owned()));
                }
                saw_pid = true;
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| ServerIpcError::Invalid("missing value for --pid".to_owned()))?;
                pid = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| ServerIpcError::Invalid(format!("invalid PID '{value}'")))?,
                );
                if pid == Some(0) {
                    return Err(ServerIpcError::Invalid(format!("invalid PID '{value}'")));
                }
                index += 2;
            }
            "--json" => {
                if json {
                    return Err(ServerIpcError::Invalid(
                        "duplicate option --json".to_owned(),
                    ));
                }
                json = true;
                index += 1;
            }
            value if raw_url.is_none() => {
                raw_url = Some(value.to_owned());
                index += 1;
            }
            value => {
                return Err(ServerIpcError::Invalid(format!(
                    "unexpected argument '{value}'"
                )));
            }
        }
    }
    let raw_url =
        raw_url.ok_or_else(|| ServerIpcError::Invalid("missing server URL".to_owned()))?;
    Ok(if watch {
        ServerCommand::WatchSet { raw_url, pid, json }
    } else {
        ServerCommand::Set { raw_url, pid, json }
    })
}

fn parse_no_argument(
    arguments: &[String],
    list: bool,
    watch: bool,
) -> Result<ServerCommand, ServerIpcError> {
    if arguments
        .iter()
        .filter(|argument| *argument == "--json")
        .count()
        > 1
    {
        return Err(ServerIpcError::Invalid(
            "duplicate option --json".to_owned(),
        ));
    }
    if arguments.iter().any(|argument| argument != "--json") {
        return Err(ServerIpcError::Invalid(format!(
            "unexpected argument '{}'",
            arguments
                .iter()
                .find(|argument| *argument != "--json")
                .unwrap()
        )));
    }
    let json = !arguments.is_empty();
    Ok(if list {
        ServerCommand::List { json }
    } else if watch {
        ServerCommand::WatchClear { json }
    } else {
        ServerCommand::Clear { json }
    })
}

fn parse_open(arguments: &[String]) -> Result<ServerCommand, ServerIpcError> {
    let mut raw_url = None;
    let mut browser = None;
    let mut json = false;
    let mut saw_browser = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--browser" => {
                if saw_browser {
                    return Err(ServerIpcError::Invalid(
                        "duplicate option --browser".to_owned(),
                    ));
                }
                saw_browser = true;
                browser = Some(
                    arguments
                        .get(index + 1)
                        .ok_or_else(|| {
                            ServerIpcError::Invalid("missing value for --browser".to_owned())
                        })?
                        .clone(),
                );
                index += 2;
            }
            "--json" => {
                if json {
                    return Err(ServerIpcError::Invalid(
                        "duplicate option --json".to_owned(),
                    ));
                }
                json = true;
                index += 1;
            }
            value if raw_url.is_none() => {
                raw_url = Some(value.to_owned());
                index += 1;
            }
            value => {
                return Err(ServerIpcError::Invalid(format!(
                    "unexpected argument '{value}'"
                )));
            }
        }
    }
    Ok(ServerCommand::Open {
        raw_url,
        browser,
        json,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerIpcRequest {
    subcommand: String,
    arguments: Vec<String>,
}

impl ServerIpcRequest {
    pub const MAX_ARGUMENTS: usize = 16;
    pub const MAX_ARGUMENT_BYTES: usize = 4096;

    /// Constructs one bounded canonical product request.
    ///
    /// # Errors
    ///
    /// Rejects unknown routes, too many arguments, or oversized arguments.
    pub fn new(subcommand: &str, arguments: Vec<String>) -> Result<Self, ServerIpcError> {
        if !matches!(
            subcommand,
            "server-set"
                | "server-clear"
                | "server-list"
                | "server-open"
                | "server-watch-set"
                | "server-watch-clear"
        ) {
            return Err(ServerIpcError::Invalid(format!(
                "unsupported server subcommand: {subcommand}"
            )));
        }
        if arguments.len() > Self::MAX_ARGUMENTS {
            return Err(ServerIpcError::Invalid(
                "too many server command arguments".to_owned(),
            ));
        }
        if arguments
            .iter()
            .any(|argument| argument.len() > Self::MAX_ARGUMENT_BYTES)
        {
            return Err(ServerIpcError::Invalid(
                "server command argument exceeds 4 KiB".to_owned(),
            ));
        }
        Ok(Self {
            subcommand: subcommand.to_owned(),
            arguments,
        })
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerIpcReply {
    stdout: Option<String>,
    error: Option<ServerIpcReplyError>,
}

impl ServerIpcReply {
    pub const MAX_STDOUT_BYTES: usize = 256 * 1024;

    /// Constructs a bounded successful reply.
    ///
    /// # Errors
    ///
    /// Rejects stdout larger than the protocol ceiling.
    pub fn success(stdout: impl Into<String>) -> Result<Self, ServerIpcError> {
        let stdout = stdout.into();
        if stdout.len() > Self::MAX_STDOUT_BYTES {
            return Err(ServerIpcError::Invalid(
                "server reply exceeds 256 KiB".to_owned(),
            ));
        }
        Ok(Self {
            stdout: (!stdout.is_empty()).then_some(stdout),
            error: None,
        })
    }

    /// Constructs a bounded failed reply.
    ///
    /// # Errors
    ///
    /// Rejects empty or oversized error codes and messages.
    pub fn failure(
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ServerIpcError> {
        let code = code.into();
        let message = message.into();
        if code.is_empty() || code.len() > 64 || message.is_empty() || message.len() > 4096 {
            return Err(ServerIpcError::Invalid(
                "invalid server reply error".to_owned(),
            ));
        }
        Ok(Self {
            stdout: None,
            error: Some(ServerIpcReplyError { code, message }),
        })
    }

    #[must_use]
    pub fn stdout(&self) -> Option<&str> {
        self.stdout.as_deref()
    }

    #[must_use]
    pub fn error(&self) -> Option<&ServerIpcReplyError> {
        self.error.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerIpcReplyError {
    code: String,
    message: String,
}

impl ServerIpcReplyError {
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
pub enum ServerIpcError {
    Invalid(String),
}

impl fmt::Display for ServerIpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for ServerIpcError {}

#[cfg(test)]
mod tests {
    use super::{ServerCommand, ServerIpcReply, ServerIpcRequest};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn source_server_commands_parse_to_exact_routes_and_arguments() {
        let set = ServerCommand::parse(&args(&["set", "localhost:5173", "--pid", "123", "--json"]))
            .unwrap();
        assert_eq!(set.route(), Some("server-set"));
        assert_eq!(
            set.ipc_arguments(),
            args(&["localhost:5173", "--pid", "123", "--json"])
        );
        assert_eq!(
            ServerCommand::parse(&args(&["watch", "--", "pnpm", "dev"])).unwrap(),
            ServerCommand::Watch {
                command: args(&["pnpm", "dev"])
            }
        );
        assert!(ServerCommand::parse(&args(&["watch"])).is_err());
        assert!(ServerCommand::parse(&args(&["set", "3000", "extra"])).is_err());
        assert_eq!(
            ServerCommand::parse(&args(&["clear"])).unwrap(),
            ServerCommand::Clear { json: false }
        );
        assert_eq!(
            ServerCommand::parse(&args(&["list", "--json"])).unwrap(),
            ServerCommand::List { json: true }
        );
        for duplicate in [
            args(&["set", "3000", "--pid", "1", "--pid", "2"]),
            args(&["list", "--json", "--json"]),
            args(&["open", "3000", "--browser", "one", "--browser", "two"]),
        ] {
            assert!(
                ServerCommand::parse(&duplicate).is_err(),
                "accepted {duplicate:?}"
            );
        }
    }

    #[test]
    fn protocol_rejects_unknown_routes_and_bounds_arguments_and_replies() {
        assert!(ServerIpcRequest::new("not-server", Vec::new()).is_err());
        assert!(
            ServerIpcRequest::new(
                "server-list",
                vec!["x".to_owned(); ServerIpcRequest::MAX_ARGUMENTS + 1],
            )
            .is_err()
        );
        assert!(
            ServerIpcRequest::new(
                "server-set",
                vec!["x".repeat(ServerIpcRequest::MAX_ARGUMENT_BYTES + 1)],
            )
            .is_err()
        );
        assert!(ServerIpcReply::success("x".repeat(ServerIpcReply::MAX_STDOUT_BYTES)).is_ok());
        assert!(ServerIpcReply::success("x".repeat(ServerIpcReply::MAX_STDOUT_BYTES + 1)).is_err());
    }
}
