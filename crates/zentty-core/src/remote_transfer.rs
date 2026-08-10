use crate::{SshConnectionOption, SshDestination};

pub const MAX_REMOTE_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_REMOTE_FILE_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTransferMethod {
    Scp,
    SshStream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteTransferPrerequisites {
    pub local_scp_available: bool,
    pub remote_sftp_available: bool,
}

impl RemoteTransferPrerequisites {
    #[must_use]
    pub fn preferred_method(self) -> RemoteTransferMethod {
        if self.local_scp_available && self.remote_sftp_available {
            RemoteTransferMethod::Scp
        } else {
            RemoteTransferMethod::SshStream
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTransferFailure {
    LocalScpMissing,
    SftpSubsystemUnavailable,
    Authentication,
    HostVerification,
    HostUnreachable,
    PermissionDenied,
    DiskFull,
    Timeout,
    Cancelled,
    IntegrityMismatch,
    DestinationCollision,
    Ambiguous,
}

impl RemoteTransferFailure {
    #[must_use]
    pub fn permits_stream_fallback(self) -> bool {
        matches!(self, Self::LocalScpMissing | Self::SftpSubsystemUnavailable)
    }
}

#[must_use]
pub fn ssh_connection_arguments(destination: &SshDestination) -> Vec<String> {
    let mut arguments = vec![
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        "ConnectTimeout=10".to_owned(),
    ];
    append_connection_options(&mut arguments, destination, false);
    if let Some(port) = destination.port {
        arguments.extend(["-p".to_owned(), port.to_string()]);
    }
    arguments
}

#[must_use]
pub fn scp_connection_arguments(destination: &SshDestination) -> Vec<String> {
    let mut arguments = vec![
        "-B".to_owned(),
        "-o".to_owned(),
        "ConnectTimeout=10".to_owned(),
    ];
    append_connection_options(&mut arguments, destination, true);
    if let Some(port) = destination.port {
        arguments.extend(["-P".to_owned(), port.to_string()]);
    }
    arguments
}

fn append_connection_options(
    arguments: &mut Vec<String>,
    destination: &SshDestination,
    for_scp: bool,
) {
    for option in destination.connection_options() {
        match option {
            SshConnectionOption::Flag(flag) if !for_scp || flag != "-a" => {
                arguments.push(flag.clone());
            }
            SshConnectionOption::Flag(_) => {}
            SshConnectionOption::Value { flag, value } if !for_scp => {
                arguments.extend([flag.clone(), value.clone()]);
            }
            SshConnectionOption::Value { flag, value } => match flag.as_str() {
                "-B" => arguments.extend(["-o".to_owned(), format!("BindInterface={value}")]),
                "-b" => arguments.extend(["-o".to_owned(), format!("BindAddress={value}")]),
                "-I" => arguments.extend(["-o".to_owned(), format!("PKCS11Provider={value}")]),
                _ => arguments.extend([flag.clone(), value.clone()]),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteUploadPath {
    final_path: String,
    partial_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteUploadPathError {
    InvalidNonce,
}

impl RemoteUploadPath {
    /// Builds the randomized source-compatible destination for clipboard image data.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteUploadPathError::InvalidNonce`] unless `nonce` is exactly
    /// eight ASCII hexadecimal characters.
    pub fn for_image(
        file_extension: &str,
        unix_timestamp: u64,
        nonce: &str,
    ) -> Result<Self, RemoteUploadPathError> {
        let extension = sanitize_component(file_extension, 16);
        let extension = if extension.is_empty() {
            "png"
        } else {
            &extension
        };
        Self::new(
            &format!("zentty-paste-{unix_timestamp}-{nonce}.{extension}"),
            nonce,
        )
    }

    /// Builds the randomized source-compatible destination for one local file.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteUploadPathError::InvalidNonce`] unless `nonce` is exactly
    /// eight ASCII hexadecimal characters.
    pub fn for_file(
        original_filename: &str,
        unix_timestamp: u64,
        nonce: &str,
    ) -> Result<Self, RemoteUploadPathError> {
        let filename = sanitized_filename(original_filename);
        Self::new(
            &format!("zentty-paste-{unix_timestamp}-{nonce}-{filename}"),
            nonce,
        )
    }

    fn new(filename: &str, nonce: &str) -> Result<Self, RemoteUploadPathError> {
        if nonce.len() != 8 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(RemoteUploadPathError::InvalidNonce);
        }
        let final_path = format!("/tmp/{filename}");
        let partial_path = format!("{final_path}.partial-{nonce}");
        Ok(Self {
            final_path,
            partial_path,
        })
    }

    /// Replaces the source-compatible short partial suffix with a
    /// transport-strength 128-bit nonce while preserving the visible final
    /// path.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteUploadPathError::InvalidNonce`] unless `nonce` is
    /// exactly 32 lowercase ASCII hexadecimal characters.
    pub fn with_transport_nonce(&self, nonce: &str) -> Result<Self, RemoteUploadPathError> {
        if nonce.len() != 32
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(RemoteUploadPathError::InvalidNonce);
        }
        Ok(Self {
            final_path: self.final_path.clone(),
            partial_path: format!("{}.partial-{nonce}", self.final_path),
        })
    }

    #[must_use]
    pub fn final_path(&self) -> &str {
        &self.final_path
    }

    #[must_use]
    pub fn partial_path(&self) -> &str {
        &self.partial_path
    }
}

#[must_use]
pub fn escape_remote_path_for_shell(path: &str) -> String {
    if path
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    {
        return path.to_owned();
    }
    format!("'{}'", path.replace('\'', "'\\''"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteVerificationPlanError {
    InvalidSha256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteVerificationPlan {
    upload_path: RemoteUploadPath,
    expected_bytes: u64,
    expected_sha256: String,
}

impl RemoteVerificationPlan {
    const INTEGRITY_MISMATCH: u8 = 70;
    const DESTINATION_COLLISION: u8 = 71;
    const MISSING_CHECKSUM_TOOL: u8 = 72;

    /// Creates the remote integrity and publication plan.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteVerificationPlanError::InvalidSha256`] unless the
    /// digest is exactly 64 lowercase ASCII hexadecimal characters. This
    /// makes every interpolated script value structural data rather than
    /// executable shell input.
    pub fn new(
        upload_path: RemoteUploadPath,
        expected_bytes: u64,
        expected_sha256: &str,
    ) -> Result<Self, RemoteVerificationPlanError> {
        if expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(RemoteVerificationPlanError::InvalidSha256);
        }
        Ok(Self {
            upload_path,
            expected_bytes,
            expected_sha256: expected_sha256.to_owned(),
        })
    }

    #[must_use]
    pub fn upload_path(&self) -> &RemoteUploadPath {
        &self.upload_path
    }

    #[must_use]
    pub const fn remote_exit_code_for_integrity_mismatch(&self) -> u8 {
        Self::INTEGRITY_MISMATCH
    }

    #[must_use]
    pub const fn remote_exit_code_for_destination_collision(&self) -> u8 {
        Self::DESTINATION_COLLISION
    }

    #[must_use]
    pub const fn remote_exit_code_for_missing_checksum_tool(&self) -> u8 {
        Self::MISSING_CHECKSUM_TOOL
    }

    #[must_use]
    pub fn script(&self) -> String {
        let partial = escape_remote_path_for_shell(self.upload_path.partial_path());
        let final_path = escape_remote_path_for_shell(self.upload_path.final_path());
        format!(
            "set -eu; p={partial}; f={final_path}; cleanup() {{ rm -f \"$p\"; }}; \
             trap cleanup EXIT HUP INT TERM; [ ! -L \"$p\" ] || exit {}; \
             [ \"$(wc -c < \"$p\" | tr -d ' ')\" = {} ] || exit {}; \
             if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum -- \"$p\"); h=${{h%% *}}; \
             elif command -v shasum >/dev/null 2>&1; then h=$(shasum -a 256 -- \"$p\"); h=${{h%% *}}; \
             else exit {}; fi; [ \"$h\" = {} ] || exit {}; \
             [ ! -e \"$f\" ] && [ ! -L \"$f\" ] || exit {}; \
             ln \"$p\" \"$f\" || exit {}; rm -f \"$p\"; trap - EXIT HUP INT TERM",
            Self::INTEGRITY_MISMATCH,
            self.expected_bytes,
            Self::INTEGRITY_MISMATCH,
            Self::MISSING_CHECKSUM_TOOL,
            self.expected_sha256,
            Self::INTEGRITY_MISMATCH,
            Self::DESTINATION_COLLISION,
            Self::DESTINATION_COLLISION,
        )
    }
}

fn sanitized_filename(original: &str) -> String {
    let original = if original.is_empty() {
        "file"
    } else {
        original
    };
    let (stem, extension) = split_filename(original);
    let stem = sanitize_component(stem, 128);
    let stem = if stem.is_empty() { "file" } else { &stem };
    let Some(extension) = extension else {
        return stem.to_owned();
    };
    let extension = sanitize_component(extension, 64);
    if extension.is_empty() {
        return stem.to_owned();
    }
    let stem_limit = 128usize.saturating_sub(extension.len() + 1).max(1);
    format!("{}.{}", truncate(stem, stem_limit), extension)
}

fn split_filename(filename: &str) -> (&str, Option<&str>) {
    match filename.rsplit_once('.') {
        Some(("", _)) => (filename, None),
        Some((stem, extension)) => (stem, Some(extension)),
        _ => (filename, None),
    }
}

fn sanitize_component(value: &str, limit: usize) -> String {
    let mut sanitized = String::new();
    let mut previous_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_') {
            sanitized.push(character);
            previous_dash = false;
        } else if !previous_dash {
            sanitized.push('-');
            previous_dash = true;
        }
    }
    let sanitized = sanitized.trim_matches(['.', '-']);
    truncate(sanitized, limit).to_owned()
}

fn truncate(value: &str, limit: usize) -> &str {
    &value[..value.len().min(limit)]
}
