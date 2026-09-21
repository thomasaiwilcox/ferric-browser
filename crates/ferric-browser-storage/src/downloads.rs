//! Safe destination and lifecycle primitives for browser downloads.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

const MAX_FILENAME_BYTES: usize = 240;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionPolicy {
    Ask,
    AutoRename,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadPathError {
    InvalidDirectory(PathBuf),
    Collision(PathBuf),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for DownloadPathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDirectory(path) => {
                write!(
                    formatter,
                    "download directory is not a real directory: {}",
                    path.display()
                )
            }
            Self::Collision(path) => write!(
                formatter,
                "download destination already exists: {}",
                path.display()
            ),
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for DownloadPathError {}

/// Converts an untrusted server-provided filename into a bounded basename.
#[must_use]
pub fn sanitize_download_filename(input: &str) -> String {
    let mut output = input
        .chars()
        .filter_map(|character| {
            if character == '/' || character == '\\' {
                Some('_')
            } else if character == '\0' || character.is_control() {
                None
            } else {
                Some(character)
            }
        })
        .collect::<String>();
    output = output.trim().trim_matches('.').to_owned();
    if output.is_empty() {
        output = "download".into();
    }
    if is_reserved_name(&output) {
        output.insert(0, '_');
    }
    truncate_preserving_extension(&output)
}

/// Chooses a non-overwriting destination under a user-selected directory.
/// The directory and all existing candidates are checked without following a
/// symlink at the final path.
///
/// # Errors
///
/// Returns an error when the directory is not safe, the name collides under
/// the requested policy, or a filesystem operation fails.
pub fn choose_download_path(
    directory: impl AsRef<Path>,
    suggested_name: &str,
    policy: CollisionPolicy,
) -> Result<PathBuf, DownloadPathError> {
    let directory = directory.as_ref();
    let metadata = fs::symlink_metadata(directory).map_err(|error| path_io(directory, &error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(DownloadPathError::InvalidDirectory(directory.to_owned()));
    }
    let filename = sanitize_download_filename(suggested_name);
    let first = directory.join(&filename);
    if !path_exists_without_following(&first)? {
        return Ok(first);
    }
    if policy == CollisionPolicy::Ask {
        return Err(DownloadPathError::Collision(first));
    }
    let (stem, extension) = split_extension(&filename);
    for number in 1..=10_000_u32 {
        let candidate_name = format!("{stem} ({number}){extension}");
        let candidate = directory.join(candidate_name);
        if !path_exists_without_following(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(DownloadPathError::Collision(first))
}

/// Creates a destination without following or overwriting an attacker-created
/// path between selection and the engine's write phase.
///
/// # Errors
///
/// Returns an error when the path already exists, is not writable, or cannot
/// be opened with exclusive creation.
pub fn create_download_file(path: impl AsRef<Path>) -> Result<File, DownloadPathError> {
    let path = path.as_ref();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| path_io(path, &error))
}

fn path_exists_without_following(path: &Path) -> Result<bool, DownloadPathError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(path_io(path, &error)),
    }
}

fn split_extension(filename: &str) -> (&str, &str) {
    filename
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map_or((filename, ""), |(stem, _extension)| {
            (stem, &filename[stem.len()..])
        })
}

fn truncate_preserving_extension(filename: &str) -> String {
    if filename.len() <= MAX_FILENAME_BYTES {
        return filename.into();
    }
    let (stem, extension) = split_extension(filename);
    if extension.len() >= MAX_FILENAME_BYTES {
        return truncate_to_bytes(extension, MAX_FILENAME_BYTES);
    }
    let available = MAX_FILENAME_BYTES.saturating_sub(extension.len());
    let mut truncated = truncate_to_bytes(stem, available);
    if truncated.is_empty() {
        truncated = truncate_to_bytes("download", available);
    }
    format!("{truncated}{extension}")
}

fn truncate_to_bytes(value: &str, limit: usize) -> String {
    value
        .chars()
        .scan(0usize, |length, character| {
            let next_length = *length + character.len_utf8();
            if next_length > limit {
                None
            } else {
                *length = next_length;
                Some(character)
            }
        })
        .collect()
}

fn is_reserved_name(filename: &str) -> bool {
    let stem = filename.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_lowercase().as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    )
}

fn path_io(path: &Path, error: &io::Error) -> DownloadPathError {
    DownloadPathError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_untrusted_names_and_preserves_extension() {
        let name = format!("../{}report/{}.pdf", "x".repeat(260), "draft");
        let sanitized = sanitize_download_filename(&name);
        assert!(!sanitized.contains('/'));
        assert!(!sanitized.contains('\\'));
        assert!(
            Path::new(&sanitized)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        );
        assert!(sanitized.len() <= MAX_FILENAME_BYTES);
        assert_eq!(sanitize_download_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_download_filename(".."), "download");
        assert!(
            sanitize_download_filename(&format!("{}é", "a".repeat(239))).len()
                <= MAX_FILENAME_BYTES
        );
        assert!(
            sanitize_download_filename(&format!("a.{}", "x".repeat(300))).len()
                <= MAX_FILENAME_BYTES
        );
    }

    #[test]
    fn collision_policy_never_selects_existing_or_symlink_paths() {
        let directory = std::env::temp_dir().join(format!(
            "ferric-browser-downloads-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).expect("directory");
        let first = directory.join("report.pdf");
        File::create(&first).expect("existing file");
        let renamed = choose_download_path(&directory, "report.pdf", CollisionPolicy::AutoRename)
            .expect("renamed destination");
        assert_eq!(
            renamed.file_name().and_then(|name| name.to_str()),
            Some("report (1).pdf")
        );
        assert_eq!(
            choose_download_path(&directory, "report.pdf", CollisionPolicy::Ask),
            Err(DownloadPathError::Collision(first))
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn create_is_exclusive_and_uses_private_mode() {
        let directory = std::env::temp_dir().join(format!(
            "ferric-browser-download-file-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).expect("directory");
        let path = directory.join("download.bin");
        drop(create_download_file(&path).expect("create"));
        assert!(matches!(
            create_download_file(&path),
            Err(DownloadPathError::Io { .. })
        ));
        let _ = fs::remove_dir_all(directory);
    }
}
