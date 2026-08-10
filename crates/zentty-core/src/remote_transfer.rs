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
