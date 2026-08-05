#![forbid(unsafe_code)]

//! Pure compatibility logic for Zentty's Claude agent-team `tmux` facade.
//!
//! This crate does not own sockets, product state, terminals, or PTYs.

mod command;
mod format;
mod invocation;
mod send_keys;
mod store;
mod target;

pub use command::{Command, CommandError};
pub use format::FormatRenderer;
pub use invocation::{Invocation, InvocationError};
pub use send_keys::SendKeys;
pub use store::{TeamAnchor, TeamStore, TeamTransition};
pub use target::PaneTarget;
