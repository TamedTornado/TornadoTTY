use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use tornadotty_test_receipts::{
    ActionName, ActionOutcome, FailureCode, FocusTarget, GeometrySnapshot, LifecycleState,
    MAX_FILE_BYTES, MAX_RECORDS, PaneColumn, ReceiptErrorKind, ReceiptEvent, ReceiptId,
    ReceiptStream, ReceiptWriter, WidgetName, WorklaneTopology,
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tornadotty-receipt-test-{}-{sequence}",
            process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn receipt(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn id(value: &str) -> ReceiptId {
    ReceiptId::new(value).unwrap()
}

fn record(sequence: u64, event: &str) -> String {
    format!(r#"{{"schema_version":1,"sequence":{sequence},"event":{event}}}"#) + "\n"
}

#[test]
fn writer_and_parser_round_trip_every_bounded_event_class() {
    let fixture = Fixture::new();
    let path = fixture.receipt("events.ndjson");
    let mut writer = ReceiptWriter::create(&path).unwrap();
    let events = [
        ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        },
        ReceiptEvent::Lifecycle {
            state: LifecycleState::TerminalReady,
            pane_id: Some(id("pane-1")),
        },
        ReceiptEvent::Topology {
            window_id: id("window-1"),
            worklanes: vec![WorklaneTopology {
                worklane_id: id("worklane-1"),
                pane_ids: vec![id("pane-1")],
                selected_pane_id: id("pane-1"),
            }],
            focused_pane_id: id("pane-1"),
        },
        ReceiptEvent::Focus {
            focus: FocusTarget::Pane {
                pane_id: id("pane-1"),
            },
        },
        ReceiptEvent::Focus {
            focus: FocusTarget::Widget {
                widget: WidgetName::NotificationSoundImport,
            },
        },
        ReceiptEvent::Geometry {
            geometry: GeometrySnapshot::Window {
                window_id: id("window-1"),
                width: 1_000,
                height: 700,
            },
        },
        ReceiptEvent::Geometry {
            geometry: GeometrySnapshot::PaneLayout {
                window_id: id("window-1"),
                worklane_id: id("worklane-1"),
                columns: vec![PaneColumn {
                    column_id: id("column-1"),
                    pane_ids: vec![id("pane-1")],
                }],
            },
        },
        ReceiptEvent::ActionCompletion {
            action: ActionName::SendTestNotification,
            outcome: ActionOutcome::Completed,
            target_id: Some(id("pane-1")),
        },
        ReceiptEvent::Failure {
            code: FailureCode::SettingsAction,
            target_id: Some(id("pane-1")),
        },
        ReceiptEvent::Lifecycle {
            state: LifecycleState::ChildExited,
            pane_id: Some(id("pane-1")),
        },
        ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStopped,
            pane_id: None,
        },
    ];
    for event in &events {
        writer.write(event.clone()).unwrap();
    }
    drop(writer);

    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    let stream = ReceiptStream::parse(&fs::read(path).unwrap()).unwrap();
    assert_eq!(stream.records().len(), events.len());
    assert!(stream.is_complete());
    assert_eq!(stream.records()[7].event, events[7]);
}

#[test]
fn parser_rejects_compatibility_structure_size_and_secret_failures() {
    let started = record(1, r#"{"category":"lifecycle","state":"process_started"}"#);
    let cases = [
        (
            "unknown-version",
            started.replace("\"schema_version\":1", "\"schema_version\":2"),
            ReceiptErrorKind::UnsupportedVersion,
        ),
        (
            "unknown-event",
            record(1, r#"{"category":"invented"}"#),
            ReceiptErrorKind::MalformedRecord,
        ),
        (
            "malformed",
            "{\n".to_owned(),
            ReceiptErrorKind::MalformedRecord,
        ),
        (
            "truncated",
            started.trim_end().to_owned(),
            ReceiptErrorKind::Truncated,
        ),
        (
            "secret-field",
            record(
                1,
                r#"{"category":"lifecycle","state":"process_started","token":"must-not-fit-schema"}"#,
            ),
            ReceiptErrorKind::MalformedRecord,
        ),
        (
            "oversized",
            format!("{}\n", "x".repeat(8_193)),
            ReceiptErrorKind::Oversized,
        ),
    ];
    for (name, bytes, expected) in cases {
        let error = ReceiptStream::parse(bytes.as_bytes()).unwrap_err();
        assert_eq!(error.kind(), expected, "case={name}: {error}");
    }
}

#[test]
fn parser_rejects_duplicate_and_out_of_order_lifecycle() {
    let started = record(1, r#"{"category":"lifecycle","state":"process_started"}"#);
    let ready_one = record(
        2,
        r#"{"category":"lifecycle","state":"terminal_ready","pane_id":"pane-1"}"#,
    );
    let duplicate_ready = record(
        3,
        r#"{"category":"lifecycle","state":"terminal_ready","pane_id":"pane-1"}"#,
    );
    let error = ReceiptStream::parse(format!("{started}{ready_one}{duplicate_ready}").as_bytes())
        .unwrap_err();
    assert_eq!(error.kind(), ReceiptErrorKind::DuplicateEvent);

    let ready_first = record(
        1,
        r#"{"category":"lifecycle","state":"terminal_ready","pane_id":"pane-1"}"#,
    );
    let error = ReceiptStream::parse(ready_first.as_bytes()).unwrap_err();
    assert_eq!(error.kind(), ReceiptErrorKind::OutOfOrder);

    let exited_before_ready = record(
        2,
        r#"{"category":"lifecycle","state":"child_exited","pane_id":"pane-1"}"#,
    );
    let error =
        ReceiptStream::parse(format!("{started}{exited_before_ready}").as_bytes()).unwrap_err();
    assert_eq!(error.kind(), ReceiptErrorKind::OutOfOrder);
}

#[test]
fn writer_rejects_existing_permissive_symlink_and_escape_paths() {
    let fixture = Fixture::new();
    let existing = fixture.receipt("existing.ndjson");
    fs::write(&existing, b"old").unwrap();
    assert_eq!(
        ReceiptWriter::create(&existing).unwrap_err().kind(),
        ReceiptErrorKind::UnsafePath
    );

    let permissive = fixture.receipt("permissive");
    fs::create_dir(&permissive).unwrap();
    fs::set_permissions(&permissive, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        ReceiptWriter::create(&permissive.join("events.ndjson"))
            .unwrap_err()
            .kind(),
        ReceiptErrorKind::UnsafePath
    );

    let linked = fixture.receipt("linked");
    symlink(&fixture.root, &linked).unwrap();
    assert_eq!(
        ReceiptWriter::create(&linked.join("events.ndjson"))
            .unwrap_err()
            .kind(),
        ReceiptErrorKind::UnsafePath
    );

    let escaped = fixture.root.join("nested");
    fs::create_dir(&escaped).unwrap();
    fs::set_permissions(&escaped, fs::Permissions::from_mode(0o700)).unwrap();
    let escaped_path = escaped.join("..").join("escaped.ndjson");
    assert_eq!(
        ReceiptWriter::create(&escaped_path).unwrap_err().kind(),
        ReceiptErrorKind::UnsafePath
    );
}

#[test]
fn identifiers_reject_user_text_and_secret_shaped_values() {
    for invalid in [
        "",
        "pane with spaces",
        "token=secret",
        "../../escape",
        &"x".repeat(97),
    ] {
        assert!(ReceiptId::new(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn receipt_path_test_fixture_is_owner_only() {
    let fixture = Fixture::new();
    let mode = fs::metadata(Path::new(&fixture.root))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn writer_enforces_record_count_before_producing_an_invalid_stream() {
    let fixture = Fixture::new();
    let path = fixture.receipt("record-limit.ndjson");
    let mut writer = ReceiptWriter::create(&path).unwrap();
    for sequence in 0..MAX_RECORDS {
        let event = if sequence == 0 {
            ReceiptEvent::Lifecycle {
                state: LifecycleState::ProcessStarted,
                pane_id: None,
            }
        } else {
            ReceiptEvent::Focus {
                focus: FocusTarget::Pane {
                    pane_id: id("pane-1"),
                },
            }
        };
        writer.write(event).unwrap();
    }
    let error = writer
        .write(ReceiptEvent::Focus {
            focus: FocusTarget::Pane {
                pane_id: id("pane-1"),
            },
        })
        .unwrap_err();
    assert_eq!(error.kind(), ReceiptErrorKind::Oversized);
    drop(writer);
    assert_eq!(
        ReceiptStream::parse(&fs::read(path).unwrap())
            .unwrap()
            .records()
            .len(),
        MAX_RECORDS
    );
}

#[test]
fn writer_enforces_total_bytes_before_producing_an_invalid_stream() {
    let fixture = Fixture::new();
    let path = fixture.receipt("byte-limit.ndjson");
    let mut writer = ReceiptWriter::create(&path).unwrap();
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        })
        .unwrap();
    let columns = (0..128)
        .map(|index| PaneColumn {
            column_id: id(&format!("column-{index:03}")),
            pane_ids: vec![id(&format!("pane-{index:03}"))],
        })
        .collect::<Vec<_>>();
    let event = ReceiptEvent::Geometry {
        geometry: GeometrySnapshot::PaneLayout {
            window_id: id("window-1"),
            worklane_id: id("worklane-1"),
            columns,
        },
    };
    let error = loop {
        match writer.write(event.clone()) {
            Ok(()) => {}
            Err(error) => break error,
        }
    };
    assert_eq!(error.kind(), ReceiptErrorKind::Oversized);
    assert_eq!(error.detail(), "receipt stream exceeds 8 MiB");
    drop(writer);
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() <= MAX_FILE_BYTES);
    ReceiptStream::parse(&bytes).unwrap();
}
