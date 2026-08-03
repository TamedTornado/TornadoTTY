#![deny(unsafe_code)]

use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AsyncBackend {
    #[default]
    Default,
    Epoll,
    IoUring,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    NotImplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => {
                formatter.write_str("the safe Ghostty adapter is not implemented")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Main-thread runtime placeholder used to establish the safe public boundary.
///
/// The `Rc` marker deliberately makes this type neither `Send` nor `Sync`.
pub struct GhosttyRuntime {
    _main_thread: Rc<()>,
}

impl GhosttyRuntime {
    /// Creates the runtime before GTK initialization.
    ///
    /// This intentionally remains a semantic red until the real FFI ownership
    /// implementation is added behind this safe boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotImplemented`] until the real adapter lands.
    pub fn new(_backend: AsyncBackend) -> Result<Self, Error> {
        Err(Error::NotImplemented)
    }
}
