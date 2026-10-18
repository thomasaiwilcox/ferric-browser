//! Prepare complete files on one bounded worker; only the current UI may publish.

use super::download_files::{StagedDownload, cleanup_staged_download};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
};
use uuid::Uuid;

pub(super) const CAPACITY: usize = 1_000;

#[derive(Debug)]
pub(super) enum SaveError {
    Cancelled,
    Source(io::Error),
    Destination(io::Error),
    Copy(io::Error),
    Sync(io::Error),
    Publish(io::Error),
    DestinationChanged,
    Unavailable,
}

impl SaveError {
    pub(super) fn failure(&self) -> ferric_browser_application::DownloadFailure {
        use ferric_browser_application::{DownloadFailure, DownloadSaveFailure};
        let kind = match self {
            Self::DestinationChanged => DownloadSaveFailure::DestinationChanged,
            Self::Source(_) => DownloadSaveFailure::SourceUnavailable,
            Self::Publish(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                DownloadSaveFailure::AlreadyExists
            }
            Self::Destination(error)
            | Self::Copy(error)
            | Self::Sync(error)
            | Self::Publish(error) => match error.kind() {
                io::ErrorKind::PermissionDenied => DownloadSaveFailure::PermissionDenied,
                io::ErrorKind::StorageFull => DownloadSaveFailure::StorageFull,
                _ => DownloadSaveFailure::Unavailable,
            },
            Self::Cancelled | Self::Unavailable => DownloadSaveFailure::Unavailable,
        };
        DownloadFailure::Save(kind)
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Saving was cancelled."),
            Self::Source(error) => write!(f, "Cannot read the downloaded file: {error}"),
            Self::Destination(error) => write!(f, "Cannot prepare the save folder: {error}"),
            Self::Copy(error) => write!(f, "Cannot write the downloaded file: {error}"),
            Self::Sync(error) => write!(f, "Cannot finish writing the downloaded file: {error}"),
            Self::Publish(error) => write!(f, "Cannot publish the downloaded file: {error}"),
            Self::DestinationChanged => {
                f.write_str("The save folder changed before the file was ready.")
            }
            Self::Unavailable => f.write_str("File saving is unavailable."),
        }
    }
}

struct Work {
    id: String,
    session: Uuid,
    staged: StagedDownload,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Work {
    fn drop(&mut self) {
        cleanup_staged_download(&self.staged);
    }
}

pub(super) struct Completion {
    pub(super) id: String,
    pub(super) session: Uuid,
    pub(super) result: Result<PreparedDownload, SaveError>,
}

/// All requests/results are bounded by the admitted-job count. Dropping this
/// owner revokes every job and disconnects it without joining a blocked fsync.
/// The detached worker can touch only owned staging/temporary files; it has no
/// publication method, application state or Qt reference.
pub(super) struct DownloadFinalizer {
    sender: Sender<Work>,
    results: Receiver<Completion>,
    jobs: BTreeMap<String, Arc<AtomicBool>>,
}

impl DownloadFinalizer {
    pub(super) fn spawn() -> io::Result<Self> {
        let (sender, requests) = mpsc::channel::<Work>();
        let (responses, results) = mpsc::sync_channel(8);
        thread::Builder::new()
            .name("download-file-save".into())
            .spawn(move || {
                while let Ok(work) = requests.recv() {
                    let result = prepare(&work.staged, &work.cancelled);
                    let completion = Completion {
                        id: work.id.clone(),
                        session: work.session,
                        result,
                    };
                    // Work releases its source on this worker even if its UI left.
                    drop(work);
                    if responses.send(completion).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender,
            results,
            jobs: BTreeMap::new(),
        })
    }

    pub(super) fn len(&self) -> usize {
        self.jobs.len()
    }
    pub(super) fn contains(&self, id: &str) -> bool {
        self.jobs.contains_key(id)
    }
    pub(super) fn cancel(&mut self, id: &str) -> bool {
        self.jobs
            .get(id)
            .is_some_and(|cancelled| !cancelled.swap(true, Ordering::AcqRel))
    }
    pub(super) fn submit(
        &mut self,
        id: String,
        session: Uuid,
        staged: StagedDownload,
    ) -> Result<(), SaveError> {
        // Destination acceptance reserves a slot across staged + finishing jobs.
        if self.jobs.len() >= CAPACITY || self.jobs.contains_key(&id) {
            cleanup_staged_download(&staged);
            return Err(SaveError::Unavailable);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let work = Work {
            id: id.clone(),
            session,
            staged,
            cancelled: Arc::clone(&cancelled),
        };
        self.sender.send(work).map_err(|_| SaveError::Unavailable)?;
        self.jobs.insert(id, cancelled);
        Ok(())
    }
    pub(super) fn poll(&mut self) -> Vec<Completion> {
        let mut ready = Vec::new();
        // Bound publication work per GUI turn as well as worker memory.
        for _ in 0..16 {
            match self.results.try_recv() {
                Ok(mut completion) => {
                    if self
                        .jobs
                        .remove(&completion.id)
                        .is_none_or(|flag| flag.load(Ordering::Acquire))
                    {
                        completion.result = Err(SaveError::Cancelled);
                    }
                    ready.push(completion);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    for (id, _) in std::mem::take(&mut self.jobs) {
                        ready.push(Completion {
                            id,
                            session: Uuid::nil(),
                            result: Err(SaveError::Unavailable),
                        });
                    }
                    break;
                }
            }
        }
        ready
    }
}

impl Drop for DownloadFinalizer {
    fn drop(&mut self) {
        for flag in self.jobs.values() {
            flag.store(true, Ordering::Release);
        }
    }
}

pub(super) struct PreparedDownload {
    directory: File,
    temporary_name: CString,
    final_name: CString,
    destination: PathBuf,
}

impl PreparedDownload {
    pub(super) fn destination(&self) -> &Path {
        &self.destination
    }

    pub(super) fn publish(self) -> Result<(), SaveError> {
        let parent = self
            .destination
            .parent()
            .ok_or(SaveError::DestinationChanged)?;
        let current = fs::symlink_metadata(parent).map_err(SaveError::Destination)?;
        let captured = self.directory.metadata().map_err(SaveError::Destination)?;
        if current.file_type().is_symlink()
            || current.dev() != captured.dev()
            || current.ino() != captured.ino()
        {
            return Err(SaveError::DestinationChanged);
        }
        publish_exclusive(&self.directory, &self.temporary_name, &self.final_name)
            .map_err(SaveError::Publish)
    }
}

impl Drop for PreparedDownload {
    fn drop(&mut self) {
        remove_owned_temporary(&self.directory, &self.temporary_name);
    }
}

fn interrupted(cancelled: &AtomicBool) -> Result<(), SaveError> {
    if cancelled.load(Ordering::Acquire) {
        Err(SaveError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn prepare(
    staged: &StagedDownload,
    cancelled: &AtomicBool,
) -> Result<PreparedDownload, SaveError> {
    interrupted(cancelled)?;
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(staged.staging_path())
        .map_err(SaveError::Source)?;
    if !source.metadata().map_err(SaveError::Source)?.is_file() {
        return Err(SaveError::Source(io::Error::other(
            "Downloaded data is not a regular file.",
        )));
    }
    let destination = staged.final_path().to_owned();
    let parent = destination.parent().ok_or(SaveError::DestinationChanged)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(parent)
        .map_err(SaveError::Destination)?;
    let final_name = CString::new(
        destination
            .file_name()
            .ok_or(SaveError::DestinationChanged)?
            .as_bytes(),
    )
    .map_err(|_| SaveError::DestinationChanged)?;
    let temporary_name = CString::new(format!(".ferric-download-{}.partial", Uuid::new_v4()))
        .expect("UUID filename has no NUL");
    let mut output =
        create_owned_temporary(&directory, &temporary_name).map_err(SaveError::Destination)?;
    let prepared = PreparedDownload {
        directory,
        temporary_name,
        final_name,
        destination,
    };
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        interrupted(cancelled)?;
        let count = match source.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result.map_err(SaveError::Source)?,
        };
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(SaveError::Copy)?;
    }
    output.sync_all().map_err(SaveError::Sync)?;
    interrupted(cancelled)?;
    Ok(prepared)
}

#[allow(unsafe_code)]
fn create_owned_temporary(directory: &File, name: &CString) -> io::Result<File> {
    // SAFETY: Both arguments remain live; name is NUL-terminated. Exclusive
    // creation plus O_NOFOLLOW creates a new regular file under the held dirfd.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a fresh, owned descriptor; exactly one File owns it.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[allow(unsafe_code)]
fn remove_owned_temporary(directory: &File, name: &CString) {
    // SAFETY: The live dirfd and owned, NUL-terminated basename are valid.
    // This never removes a chosen final path; successful rename removes this name.
    unsafe {
        libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0);
    }
}

#[allow(unsafe_code)]
fn publish_exclusive(directory: &File, source: &CString, destination: &CString) -> io::Result<()> {
    // SAFETY: Both basenames are NUL-terminated and the directory File owns the
    // live fd. RENAME_NOREPLACE cannot replace a file/symlink created after admission.
    let result = unsafe {
        libc::renameat2(
            directory.as_raw_fd(),
            source.as_ptr(),
            directory.as_raw_fd(),
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
#[path = "download_finalization_tests.rs"]
mod tests;
