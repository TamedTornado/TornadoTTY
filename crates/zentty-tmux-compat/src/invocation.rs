use crate::{Command, CommandError};
use std::fmt;

const GLOBAL_VALUE_OPTIONS: [&str; 5] = ["-S", "-L", "-T", "-f", "-c"];
const GLOBAL_BOOLEAN_OPTIONS: [&str; 13] = [
    "-2", "-C", "-CC", "-D", "-d", "-l", "-N", "-P", "-q", "-u", "-U", "-v", "-V",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    pub command: Command,
    pub arguments: Vec<String>,
}

impl Invocation {
    /// Parses global tmux options and canonicalizes the following command.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing command, a global option without its
    /// required value, or an unsupported command.
    pub fn parse(arguments: &[String]) -> Result<Self, InvocationError> {
        let mut arguments = arguments.iter();
        while let Some(argument) = arguments.next() {
            let argument = argument.as_str();
            if GLOBAL_VALUE_OPTIONS.contains(&argument) {
                if arguments.next().is_none() {
                    return Err(InvocationError::MissingGlobalOptionValue(
                        argument.to_owned(),
                    ));
                }
                continue;
            }
            if GLOBAL_BOOLEAN_OPTIONS.contains(&argument) {
                continue;
            }
            return Ok(Self {
                command: Command::parse(argument)?,
                arguments: arguments.cloned().collect(),
            });
        }
        Err(InvocationError::MissingCommand)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvocationError {
    MissingCommand,
    MissingGlobalOptionValue(String),
    UnsupportedCommand(CommandError),
}

impl fmt::Display for InvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => formatter.write_str("tmux compatibility command is required"),
            Self::MissingGlobalOptionValue(option) => {
                write!(formatter, "tmux global option requires a value: {option}")
            }
            Self::UnsupportedCommand(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InvocationError {}

impl From<CommandError> for InvocationError {
    fn from(error: CommandError) -> Self {
        Self::UnsupportedCommand(error)
    }
}
