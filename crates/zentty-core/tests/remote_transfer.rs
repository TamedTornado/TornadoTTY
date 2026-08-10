use zentty_core::{
    MAX_REMOTE_FILE_BYTES, MAX_REMOTE_IMAGE_BYTES, RemoteTransferFailure, RemoteTransferMethod,
    RemoteTransferPrerequisites, RemoteUploadPath, RemoteUploadPathError, SshDestination,
    escape_remote_path_for_shell, parse_ssh_destination, scp_connection_arguments,
    ssh_connection_arguments,
};

#[test]
fn source_limits_and_path_shapes_are_exact() {
    assert_eq!(MAX_REMOTE_IMAGE_BYTES, 10 * 1024 * 1024);
    assert_eq!(MAX_REMOTE_FILE_BYTES, 500 * 1024 * 1024);
    let image = RemoteUploadPath::for_image("jpeg", 1_700_000_000, "1234abcd").unwrap();
    assert_eq!(
        image.final_path(),
        "/tmp/zentty-paste-1700000000-1234abcd.jpeg"
    );
    assert_eq!(
        image.partial_path(),
        "/tmp/zentty-paste-1700000000-1234abcd.jpeg.partial-1234abcd"
    );
}

#[test]
fn source_filename_sanitization_preserves_safe_identity() {
    for (source, expected) in [
        ("Quarterly Report.pdf", "Quarterly-Report.pdf"),
        ("résumé 🚀.png", "r-sum.png"),
        ("???bad***name.zip", "bad-name.zip"),
        (".env", "env"),
        (".env.local", "env.local"),
        ("name.", "name"),
        (".", "file"),
        ("a..b", "a.b"),
        ("", "file"),
    ] {
        let path = RemoteUploadPath::for_file(source, 1_700_000_000, "1234abcd").unwrap();
        assert_eq!(
            path.final_path(),
            format!("/tmp/zentty-paste-1700000000-1234abcd-{expected}")
        );
    }
    assert_eq!(
        RemoteUploadPath::for_file("a", 1, "bad"),
        Err(RemoteUploadPathError::InvalidNonce)
    );
    let long_name = format!("{}.{}", "a".repeat(120), "b".repeat(30));
    let long_path = RemoteUploadPath::for_file(&long_name, 1, "1234abcd").unwrap();
    let uploaded_name = long_path.final_path().rsplit_once('-').unwrap().1;
    assert_eq!(uploaded_name.len(), 128);
    assert!(uploaded_name.ends_with(&format!(".{}", "b".repeat(30))));
}

#[test]
fn fallback_is_limited_to_missing_transport_capability() {
    assert_eq!(
        RemoteTransferPrerequisites {
            local_scp_available: true,
            remote_sftp_available: true,
        }
        .preferred_method(),
        RemoteTransferMethod::Scp
    );
    assert_eq!(
        RemoteTransferPrerequisites {
            local_scp_available: false,
            remote_sftp_available: true,
        }
        .preferred_method(),
        RemoteTransferMethod::SshStream
    );
    for failure in [
        RemoteTransferFailure::LocalScpMissing,
        RemoteTransferFailure::SftpSubsystemUnavailable,
    ] {
        assert!(failure.permits_stream_fallback());
    }
    for failure in [
        RemoteTransferFailure::Authentication,
        RemoteTransferFailure::HostVerification,
        RemoteTransferFailure::HostUnreachable,
        RemoteTransferFailure::PermissionDenied,
        RemoteTransferFailure::DiskFull,
        RemoteTransferFailure::Timeout,
        RemoteTransferFailure::Cancelled,
        RemoteTransferFailure::IntegrityMismatch,
        RemoteTransferFailure::DestinationCollision,
        RemoteTransferFailure::Ambiguous,
    ] {
        assert!(!failure.permits_stream_fallback(), "{failure:?}");
    }
}

#[test]
fn inserted_paths_are_shell_safe() {
    assert_eq!(
        escape_remote_path_for_shell("/tmp/zentty-paste-a_b-1.png"),
        "/tmp/zentty-paste-a_b-1.png"
    );
    assert_eq!(
        escape_remote_path_for_shell("/tmp/a b'c"),
        "'/tmp/a b'\\''c'"
    );
}

#[test]
fn ssh_and_scp_reuse_only_connection_options_with_exact_flag_translation() {
    let destination = parse_ssh_destination(&[
        "ssh",
        "-4",
        "-a",
        "-B",
        "eth0",
        "-b",
        "127.0.0.2",
        "-I",
        "/token.so",
        "-i",
        "/key with spaces",
        "-o",
        "BatchMode=no",
        "-p",
        "2222",
        "deploy@example.test",
    ])
    .unwrap();
    assert_eq!(
        ssh_connection_arguments(&destination),
        [
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-4",
            "-a",
            "-B",
            "eth0",
            "-b",
            "127.0.0.2",
            "-I",
            "/token.so",
            "-i",
            "/key with spaces",
            "-p",
            "2222",
        ]
    );
    assert_eq!(
        scp_connection_arguments(&destination),
        [
            "-B",
            "-o",
            "ConnectTimeout=10",
            "-4",
            "-o",
            "BindInterface=eth0",
            "-o",
            "BindAddress=127.0.0.2",
            "-o",
            "PKCS11Provider=/token.so",
            "-i",
            "/key with spaces",
            "-P",
            "2222",
        ]
    );
    assert_eq!(
        SshDestination::new("host", None, "host", None).connection_options(),
        &[]
    );
}
