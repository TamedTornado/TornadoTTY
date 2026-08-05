#![forbid(unsafe_code)]

//! Pure compatibility logic for Zentty's Claude agent-team `tmux` facade.
//!
//! This crate does not own sockets, product state, terminals, or PTYs.

mod arguments;
mod command;
mod format;
mod invocation;
mod send_keys;
mod store;
mod target;

pub use arguments::ParsedArguments;
pub use command::{Command, CommandError};
pub use format::FormatRenderer;
pub use invocation::{Invocation, InvocationError};
pub use send_keys::SendKeys;
pub use store::{StoreError, TeamAnchor, TeamStore, TeamTransition};
pub use target::PaneTarget;
