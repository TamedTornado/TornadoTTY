#![forbid(unsafe_code)]

mod event;
pub mod input;
mod parser;
pub mod scenario;
pub mod session;
mod writer;

use std::fmt;

pub use event::{
    ActionName, ActionOutcome, FocusTarget, GeometrySnapshot, LifecycleState, PaneColumn,
    ReceiptEvent, ReceiptId, WorklaneTopology,
};
pub use event::{FailureCode, WidgetName};
pub use parser::{ReceiptRecord, ReceiptStream};
pub use writer::{
    RECEIPT_ENVIRONMENT_VARIABLE, ReceiptWriter, emit, finish, initialize_from_environment,
};

pub const SCHEMA_VERSION: u8 = 1;
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RECORD_BYTES: usize = 8 * 1024;
pub const MAX_RECORDS: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptErrorKind {
    AlreadyInitialized,
    DuplicateEvent,
    Io,
    InvalidEvent,
    MalformedRecord,
    OutOfOrder,
    Oversized,
    Truncated,
    UnsafePath,
    UnsupportedVersion,
}

#[derive(Debug)]
pub struct ReceiptError {
    kind: ReceiptErrorKind,
    detail: String,
}

impl ReceiptError {
    #[must_use]
    pub fn invalid_event(detail: impl Into<String>) -> Self {
        error(ReceiptErrorKind::InvalidEvent, detail)
    }

    #[must_use]
    pub fn kind(&self) -> ReceiptErrorKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", error_label(self.kind), self.detail)
    }
}

impl std::error::Error for ReceiptError {}

pub type Result<T> = std::result::Result<T, ReceiptError>;

pub(crate) fn error(kind: ReceiptErrorKind, detail: impl Into<String>) -> ReceiptError {
    ReceiptError {
        kind,
        detail: detail.into(),
    }
}

const fn error_label(kind: ReceiptErrorKind) -> &'static str {
    match kind {
        ReceiptErrorKind::AlreadyInitialized => "already initialized",
        ReceiptErrorKind::DuplicateEvent => "duplicate event",
        ReceiptErrorKind::Io => "I/O error",
        ReceiptErrorKind::InvalidEvent => "invalid event",
        ReceiptErrorKind::MalformedRecord => "malformed record",
        ReceiptErrorKind::OutOfOrder => "out-of-order event",
        ReceiptErrorKind::Oversized => "size limit exceeded",
        ReceiptErrorKind::Truncated => "truncated record",
        ReceiptErrorKind::UnsafePath => "unsafe receipt path",
        ReceiptErrorKind::UnsupportedVersion => "unsupported receipt version",
    }
}
