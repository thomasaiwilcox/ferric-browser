use browser_config::LoggingConfig;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const LOG_FILE_NAME: &str = "rustbrowser.jsonl";
const MAX_EVENT_BYTES: usize = 512;

pub(crate) struct StructuredLogSink {
    path: PathBuf,
    file: File,
    max_bytes: u64,
    retained_files: usize,
    level: u8,
}

impl StructuredLogSink {
    pub(crate) fn open(state_root: &Path, config: &LoggingConfig) -> io::Result<Self> {
        let directory = state_root.join("logs");
        fs::create_dir_all(&directory)?;
        let path = directory.join(LOG_FILE_NAME);
        let file = open_private_append(&path)?;
        let mut sink = Self {
            path,
            file,
            max_bytes: u64::from(config.max_file_mib).saturating_mul(1024 * 1024),
            retained_files: usize::try_from(config.retained_files).unwrap_or(1).max(1),
            level: level_rank(&config.level),
        };
        if sink.file.metadata()?.len() >= sink.max_bytes {
            sink.rotate()?;
        }
        Ok(sink)
    }

    pub(crate) fn record_action(
        &mut self,
        level: &str,
        action_id: &str,
        operation_id: &str,
        outcome: &str,
        category: Option<&str>,
    ) -> io::Result<()> {
        let Some(level_rank) = level_rank_name(level) else {
            return Ok(());
        };
        if level_rank > self.level {
            return Ok(());
        }
        let safe = |value: &str| {
            value.len() <= MAX_EVENT_BYTES
                && !value.is_empty()
                && !value.chars().any(char::is_control)
        };
        if !safe(action_id) || !safe(operation_id) || !safe(outcome) {
            return Ok(());
        }
        if category.is_some_and(|value| !safe(value)) {
            return Ok(());
        }
        let mut payload = json!({
            "schema": 1,
            "timestamp": unix_timestamp(),
            "level": level,
            "event": "action.audit",
            "action_id": action_id,
            "operation_id": operation_id,
            "outcome": outcome,
            "privacy": "redacted",
        });
        if let Some(category) = category {
            payload["category"] = category.into();
        }
        let mut line = serde_json::to_vec(&payload).map_err(io::Error::other)?;
        line.push(b'\n');
        if self
            .file
            .metadata()?
            .len()
            .saturating_add(line.len() as u64)
            > self.max_bytes
        {
            self.rotate()?;
        }
        self.file.write_all(&line)?;
        self.file.flush()
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.sync_data()?;
        for index in (1..self.retained_files).rev() {
            let source = rotated_path(&self.path, index);
            let target = rotated_path(&self.path, index + 1);
            if source.exists() {
                let _ = fs::remove_file(&target);
                fs::rename(source, target)?;
            }
        }
        let first = rotated_path(&self.path, 1);
        let _ = fs::remove_file(&first);
        fs::rename(&self.path, first)?;
        self.file = open_private_append(&self.path)?;
        Ok(())
    }
}

fn open_private_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), index))
}

fn level_rank(level: &browser_config::LogLevel) -> u8 {
    match level {
        browser_config::LogLevel::Error => 0,
        browser_config::LogLevel::Warn => 1,
        browser_config::LogLevel::Info => 2,
        browser_config::LogLevel::Debug => 3,
    }
}

fn level_rank_name(level: &str) -> Option<u8> {
    match level {
        "error" => Some(0),
        "warn" => Some(1),
        "info" => Some(2),
        "debug" => Some(3),
        _ => None,
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser_config::{LogLevel, LoggingConfig};

    fn config(level: LogLevel, max_file_mib: u32, retained_files: u32) -> LoggingConfig {
        LoggingConfig {
            level,
            max_file_mib,
            retained_files,
        }
    }

    #[test]
    fn action_records_are_redacted_bounded_and_level_filtered() {
        let root = std::env::temp_dir().join(format!("rustbrowser-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut sink = StructuredLogSink::open(&root, &config(LogLevel::Info, 1, 2)).unwrap();
        sink.record_action("debug", "browser.tab.open", "op-1", "accepted", None)
            .unwrap();
        sink.record_action(
            "error",
            "browser.tab.open",
            "op-2",
            "failed",
            Some("stale-target"),
        )
        .unwrap();
        let content = fs::read_to_string(root.join("logs/rustbrowser.jsonl")).unwrap();
        assert!(!content.contains("op-1"));
        assert!(content.contains("op-2"));
        assert!(content.contains("\"privacy\":\"redacted\""));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rotation_keeps_the_configured_number_of_files() {
        let root =
            std::env::temp_dir().join(format!("rustbrowser-log-rotate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut sink = StructuredLogSink::open(&root, &config(LogLevel::Debug, 1, 2)).unwrap();
        sink.max_bytes = 1;
        for index in 0..3 {
            sink.record_action(
                "info",
                "browser.test",
                &format!("op-{index}"),
                "accepted",
                None,
            )
            .unwrap();
        }
        assert!(root.join("logs/rustbrowser.jsonl").exists());
        assert!(root.join("logs/rustbrowser.jsonl.1").exists());
        assert!(root.join("logs/rustbrowser.jsonl.2").exists());
        assert!(!root.join("logs/rustbrowser.jsonl.3").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn opening_an_oversized_log_rotates_before_new_events() {
        let root = std::env::temp_dir().join(format!(
            "rustbrowser-log-open-rotate-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let mut first = StructuredLogSink::open(&root, &config(LogLevel::Debug, 1, 2)).unwrap();
        first.max_bytes = 1;
        first
            .record_action("info", "browser.test", "op-old", "accepted", None)
            .unwrap();
        drop(first);
        let active_path = root.join("logs/rustbrowser.jsonl");
        let mut filler = OpenOptions::new().append(true).open(&active_path).unwrap();
        filler.write_all(&vec![b'x'; 1024 * 1024]).unwrap();
        drop(filler);

        let mut reopened = StructuredLogSink::open(&root, &config(LogLevel::Debug, 1, 2)).unwrap();
        assert!(root.join("logs/rustbrowser.jsonl.1").exists());
        reopened.max_bytes = 1024;
        reopened
            .record_action("info", "browser.test", "op-new", "accepted", None)
            .unwrap();
        let active = fs::read_to_string(root.join("logs/rustbrowser.jsonl")).unwrap();
        assert!(active.contains("op-new"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn log_file_symlinks_are_rejected() {
        let root =
            std::env::temp_dir().join(format!("rustbrowser-log-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let directory = root.join("logs");
        fs::create_dir_all(&directory).unwrap();
        let target = root.join("outside");
        fs::write(&target, b"must not be opened").unwrap();
        std::os::unix::fs::symlink(&target, directory.join(LOG_FILE_NAME)).unwrap();
        assert!(StructuredLogSink::open(&root, &config(LogLevel::Info, 1, 2)).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"must not be opened");
        let _ = fs::remove_dir_all(root);
    }
}
