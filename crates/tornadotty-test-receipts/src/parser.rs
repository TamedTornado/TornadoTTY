use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    LifecycleState, MAX_FILE_BYTES, MAX_RECORD_BYTES, MAX_RECORDS, ReceiptErrorKind, ReceiptEvent,
    ReceiptId, Result, SCHEMA_VERSION, error,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptRecord {
    pub schema_version: u8,
    pub sequence: u64,
    pub event: ReceiptEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptStream {
    records: Vec<ReceiptRecord>,
    complete: bool,
}

impl ReceiptStream {
    /// Parses and validates a complete sequence of newline-delimited records.
    /// A running product may omit `process_stopped`, which is reflected by
    /// [`Self::is_complete`].
    ///
    /// # Errors
    ///
    /// Returns an error for size violations, malformed or truncated records,
    /// unsupported versions, invalid events, or lifecycle/order violations.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(error(
                ReceiptErrorKind::Oversized,
                "receipt stream exceeds 8 MiB",
            ));
        }
        if bytes.is_empty() {
            return Err(error(
                ReceiptErrorKind::Truncated,
                "receipt stream contains no complete records",
            ));
        }
        if !bytes.ends_with(b"\n") {
            return Err(error(
                ReceiptErrorKind::Truncated,
                "receipt stream does not end at a record boundary",
            ));
        }
        let mut records = Vec::new();
        let mut ready_panes = BTreeSet::<ReceiptId>::new();
        let mut exited_panes = BTreeSet::<ReceiptId>::new();
        let mut stopped = false;
        for (index, line) in bytes[..bytes.len() - 1]
            .split(|byte| *byte == b'\n')
            .enumerate()
        {
            if line.is_empty() {
                return Err(error(
                    ReceiptErrorKind::MalformedRecord,
                    format!("record {} is empty", index + 1),
                ));
            }
            if line.len() > MAX_RECORD_BYTES {
                return Err(error(
                    ReceiptErrorKind::Oversized,
                    format!("record {} exceeds 8 KiB", index + 1),
                ));
            }
            if records.len() == MAX_RECORDS {
                return Err(error(
                    ReceiptErrorKind::Oversized,
                    "receipt stream exceeds 8192 records",
                ));
            }
            let record: ReceiptRecord = serde_json::from_slice(line).map_err(|cause| {
                error(
                    ReceiptErrorKind::MalformedRecord,
                    format!("record {} is not valid schema JSON: {cause}", index + 1),
                )
            })?;
            if record.schema_version != SCHEMA_VERSION {
                return Err(error(
                    ReceiptErrorKind::UnsupportedVersion,
                    format!(
                        "record {} uses version {}",
                        index + 1,
                        record.schema_version
                    ),
                ));
            }
            let expected_sequence = u64::try_from(index + 1).map_err(|_| {
                error(
                    ReceiptErrorKind::Oversized,
                    "receipt record index cannot be represented",
                )
            })?;
            if record.sequence != expected_sequence {
                return Err(error(
                    ReceiptErrorKind::OutOfOrder,
                    format!(
                        "record {} has sequence {}, expected {expected_sequence}",
                        index + 1,
                        record.sequence
                    ),
                ));
            }
            record.event.validate()?;
            validate_order(&record, index, stopped, &mut ready_panes, &mut exited_panes)?;
            if matches!(
                record.event,
                ReceiptEvent::Lifecycle {
                    state: LifecycleState::ProcessStopped,
                    ..
                }
            ) {
                stopped = true;
            }
            records.push(record);
        }
        Ok(Self {
            records,
            complete: stopped,
        })
    }

    #[must_use]
    pub fn records(&self) -> &[ReceiptRecord] {
        &self.records
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

fn validate_order(
    record: &ReceiptRecord,
    index: usize,
    stopped: bool,
    ready_panes: &mut BTreeSet<ReceiptId>,
    exited_panes: &mut BTreeSet<ReceiptId>,
) -> Result<()> {
    if stopped {
        return Err(error(
            ReceiptErrorKind::OutOfOrder,
            "an event follows process_stopped",
        ));
    }
    if index == 0 {
        if !matches!(
            record.event,
            ReceiptEvent::Lifecycle {
                state: LifecycleState::ProcessStarted,
                pane_id: None
            }
        ) {
            return Err(error(
                ReceiptErrorKind::OutOfOrder,
                "the first event must be process_started",
            ));
        }
        return Ok(());
    }
    match &record.event {
        ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            ..
        } => Err(error(
            ReceiptErrorKind::DuplicateEvent,
            "process_started may appear only once",
        )),
        ReceiptEvent::Lifecycle {
            state: LifecycleState::TerminalReady,
            pane_id: Some(pane_id),
        } => {
            if ready_panes.insert(pane_id.clone()) {
                Ok(())
            } else {
                Err(error(
                    ReceiptErrorKind::DuplicateEvent,
                    format!("terminal_ready repeated for {}", pane_id.as_str()),
                ))
            }
        }
        ReceiptEvent::Lifecycle {
            state: LifecycleState::ChildExited,
            pane_id: Some(pane_id),
        } => {
            if !ready_panes.contains(pane_id) {
                return Err(error(
                    ReceiptErrorKind::OutOfOrder,
                    format!(
                        "child_exited precedes terminal_ready for {}",
                        pane_id.as_str()
                    ),
                ));
            }
            if exited_panes.insert(pane_id.clone()) {
                Ok(())
            } else {
                Err(error(
                    ReceiptErrorKind::DuplicateEvent,
                    format!("child_exited repeated for {}", pane_id.as_str()),
                ))
            }
        }
        _ => Ok(()),
    }
}
