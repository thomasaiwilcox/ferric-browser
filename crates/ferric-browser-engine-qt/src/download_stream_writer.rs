//! One bounded writer for browser-owned fetch streams; no network or UI objects.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
};

pub(super) const CAPACITY: usize = 8;
pub(super) const CHUNK_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub(super) enum Phase {
    // Stable values consumed by the QML transport adapter.
    Reserved = 0,
    Opening = 1,
    Ready = 2,
    Writing = 3,
    Closing = 4,
    Complete = 5,
    Cancelled = 6,
    Failed = 7,
}

struct Slot {
    phase: Phase,
    bytes: i64,
    cancelled: Arc<AtomicBool>,
    pending_open: Option<PathBuf>,
    error: Option<io::ErrorKind>,
}
struct StreamFile {
    file: Option<File>,
    path: PathBuf,
    bytes: i64,
    cancelled: Arc<AtomicBool>,
    keep: bool,
}
impl Drop for StreamFile {
    fn drop(&mut self) {
        self.file.take();
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}
enum Command {
    Open {
        id: String,
        path: PathBuf,
        cancelled: Arc<AtomicBool>,
    },
    Write {
        id: String,
        bytes: Vec<u8>,
    },
    Finish {
        id: String,
    },
    Wake,
}
struct Event {
    id: String,
    phase: Phase,
    bytes: i64,
    error: Option<io::ErrorKind>,
}

pub(super) struct StreamDownloadWriter {
    sender: SyncSender<Command>,
    events: Receiver<Event>,
    slots: BTreeMap<String, Slot>,
}
impl StreamDownloadWriter {
    pub(super) fn spawn(wake: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        let (sender, commands) = mpsc::sync_channel::<Command>(CAPACITY);
        let (events, receiver) = mpsc::sync_channel(CAPACITY * 2);
        thread::Builder::new()
            .name("download-stream-write".into())
            .spawn(move || {
                let mut files = BTreeMap::<String, StreamFile>::new();
                while let Ok(command) = commands.recv() {
                    let cancelled: Vec<_> = files
                        .iter()
                        .filter(|(_, file)| file.cancelled.load(Ordering::Acquire))
                        .map(|(id, _)| id.clone())
                        .collect();
                    for id in cancelled {
                        files.remove(&id);
                    }
                    let event = match command {
                        Command::Open {
                            id,
                            path,
                            cancelled,
                        } => {
                            if cancelled.load(Ordering::Acquire) {
                                Event {
                                    id,
                                    phase: Phase::Cancelled,
                                    bytes: 0,
                                    error: None,
                                }
                            } else {
                                match OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .mode(0o600)
                                    .custom_flags(libc::O_NOFOLLOW)
                                    .open(&path)
                                {
                                    Ok(file) => {
                                        files.insert(
                                            id.clone(),
                                            StreamFile {
                                                file: Some(file),
                                                path,
                                                bytes: 0,
                                                cancelled,
                                                keep: false,
                                            },
                                        );
                                        Event {
                                            id,
                                            phase: Phase::Ready,
                                            bytes: 0,
                                            error: None,
                                        }
                                    }
                                    Err(error) => Event {
                                        id,
                                        phase: Phase::Failed,
                                        bytes: 0,
                                        error: Some(error.kind()),
                                    },
                                }
                            }
                        }
                        Command::Write { id, bytes } => {
                            let result = files
                                .get_mut(&id)
                                .ok_or(io::ErrorKind::Interrupted)
                                .and_then(|stream| {
                                    if stream.cancelled.load(Ordering::Acquire) {
                                        return Err(io::ErrorKind::Interrupted);
                                    }
                                    let next_bytes = stream
                                        .bytes
                                        .checked_add(
                                            i64::try_from(bytes.len())
                                                .map_err(|_| io::ErrorKind::InvalidData)?,
                                        )
                                        .ok_or(io::ErrorKind::InvalidData)?;
                                    stream
                                        .file
                                        .as_mut()
                                        .ok_or(io::ErrorKind::NotFound)?
                                        .write_all(&bytes)
                                        .map_err(|error| error.kind())?;
                                    stream.bytes = next_bytes;
                                    Ok(next_bytes)
                                });
                            match result {
                                Ok(bytes) => Event {
                                    id,
                                    phase: Phase::Ready,
                                    bytes,
                                    error: None,
                                },
                                Err(error) => {
                                    files.remove(&id);
                                    Event {
                                        id,
                                        phase: Phase::Failed,
                                        bytes: 0,
                                        error: Some(error),
                                    }
                                }
                            }
                        }
                        Command::Finish { id } => {
                            if let Some(mut stream) = files.remove(&id) {
                                stream.file.take();
                                stream.keep = !stream.cancelled.load(Ordering::Acquire);
                                Event {
                                    id,
                                    phase: if stream.keep {
                                        Phase::Complete
                                    } else {
                                        Phase::Cancelled
                                    },
                                    bytes: stream.bytes,
                                    error: None,
                                }
                            } else {
                                Event {
                                    id,
                                    phase: Phase::Cancelled,
                                    bytes: 0,
                                    error: None,
                                }
                            }
                        }
                        Command::Wake => continue,
                    };
                    if events.send(event).is_err() {
                        break;
                    }
                    wake();
                }
                // Every still-open file belongs to an abandoned stream; Drop removes it.
            })?;
        Ok(Self {
            sender,
            events: receiver,
            slots: BTreeMap::new(),
        })
    }
    pub(super) fn available(&self) -> bool {
        self.slots.len() < CAPACITY
    }
    pub(super) fn reserve(&mut self, id: String) -> bool {
        if !self.available() || self.slots.contains_key(&id) {
            return false;
        }
        self.slots.insert(
            id,
            Slot {
                phase: Phase::Reserved,
                bytes: 0,
                cancelled: Arc::new(AtomicBool::new(false)),
                pending_open: None,
                error: None,
            },
        );
        true
    }
    pub(super) fn begin(&mut self, id: &str, path: PathBuf) -> bool {
        let Some(slot) = self.slots.get_mut(id) else {
            return false;
        };
        if slot.phase != Phase::Reserved {
            return false;
        }
        slot.phase = Phase::Opening;
        slot.pending_open = Some(path);
        self.submit_pending_opens();
        true
    }
    fn submit_pending_opens(&mut self) {
        for (id, slot) in &mut self.slots {
            let Some(path) = slot.pending_open.take() else {
                continue;
            };
            match self.sender.try_send(Command::Open {
                id: id.clone(),
                path,
                cancelled: Arc::clone(&slot.cancelled),
            }) {
                Ok(()) => {}
                Err(TrySendError::Full(Command::Open { path, .. })) => {
                    slot.pending_open = Some(path)
                }
                Err(_) => {
                    slot.phase = Phase::Failed;
                    slot.error = Some(io::ErrorKind::BrokenPipe);
                }
            }
        }
    }
    pub(super) fn write(&mut self, id: &str, bytes: &[u8]) -> bool {
        let Some(slot) = self.slots.get_mut(id) else {
            return false;
        };
        if bytes.is_empty() || bytes.len() > CHUNK_BYTES {
            slot.cancelled.store(true, Ordering::Release);
            slot.phase = Phase::Failed;
            slot.error = Some(io::ErrorKind::InvalidData);
            let _ = self.sender.try_send(Command::Wake);
            return false;
        }
        if slot.phase != Phase::Ready || slot.cancelled.load(Ordering::Acquire) {
            return false;
        }
        match self.sender.try_send(Command::Write {
            id: id.to_owned(),
            bytes: bytes.to_owned(),
        }) {
            Ok(()) => {
                slot.phase = Phase::Writing;
                true
            }
            Err(TrySendError::Full(_)) => false,
            Err(_) => {
                slot.phase = Phase::Failed;
                slot.error = Some(io::ErrorKind::BrokenPipe);
                false
            }
        }
    }
    pub(super) fn finish(&mut self, id: &str) -> bool {
        let Some(slot) = self.slots.get_mut(id) else {
            return false;
        };
        if slot.phase != Phase::Ready {
            return false;
        }
        match self.sender.try_send(Command::Finish { id: id.to_owned() }) {
            Ok(()) => {
                slot.phase = Phase::Closing;
                true
            }
            Err(TrySendError::Full(_)) => false,
            Err(_) => {
                slot.phase = Phase::Failed;
                slot.error = Some(io::ErrorKind::BrokenPipe);
                false
            }
        }
    }
    pub(super) fn phase(&self, id: &str) -> Phase {
        self.slots
            .get(id)
            .map_or(Phase::Cancelled, |slot| slot.phase)
    }
    pub(super) fn bytes(&self, id: &str) -> i64 {
        self.slots.get(id).map_or(0, |slot| slot.bytes)
    }
    pub(super) fn error(&self, id: &str) -> Option<io::ErrorKind> {
        self.slots.get(id).and_then(|slot| slot.error)
    }
    pub(super) fn cancel(&mut self, id: &str) {
        if let Some(slot) = self.slots.get_mut(id) {
            slot.cancelled.store(true, Ordering::Release);
            slot.pending_open = None;
            slot.phase = Phase::Cancelled;
            let _ = self.sender.try_send(Command::Wake);
        }
    }
    pub(super) fn release(&mut self, id: &str) {
        self.cancel(id);
        self.slots.remove(id);
    }
    pub(super) fn poll(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    if let Some(slot) = self.slots.get_mut(&event.id) {
                        if !slot.cancelled.load(Ordering::Acquire) {
                            slot.phase = event.phase;
                            slot.bytes = slot.bytes.max(event.bytes);
                            slot.error = event.error;
                            changed.push(event.id);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    for (id, slot) in &mut self.slots {
                        if matches!(
                            slot.phase,
                            Phase::Opening | Phase::Ready | Phase::Writing | Phase::Closing
                        ) {
                            slot.phase = Phase::Failed;
                            slot.error = Some(io::ErrorKind::BrokenPipe);
                            changed.push(id.clone());
                        }
                    }
                    break;
                }
            }
        }
        self.submit_pending_opens();
        changed.sort_unstable();
        changed.dedup();
        changed
    }
}
impl Drop for StreamDownloadWriter {
    fn drop(&mut self) {
        for slot in self.slots.values() {
            slot.cancelled.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    struct Fixture {
        directory: PathBuf,
        writer: StreamDownloadWriter,
        wakes: Receiver<()>,
    }
    impl Fixture {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("ferric-stream-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&directory).unwrap();
            let (sender, wakes) = mpsc::channel();
            let writer = StreamDownloadWriter::spawn(Arc::new(move || {
                let _ = sender.send(());
            }))
            .unwrap();
            Self {
                directory,
                writer,
                wakes,
            }
        }
        fn wait(&mut self, id: &str, phase: Phase) {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                self.writer.poll();
                if self.writer.phase(id) == phase {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "stream {id}: {:?}, wanted {phase:?}",
                    self.writer.phase(id)
                );
                self.wakes
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap();
            }
        }
        fn open(&mut self, id: &str) -> PathBuf {
            let path = self.directory.join(id);
            assert!(self.writer.reserve(id.to_owned()));
            assert!(self.writer.begin(id, path.clone()));
            self.wait(id, Phase::Ready);
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
    #[test]
    fn binary_payload_waits_for_each_ack_and_survives_completion() {
        let mut f = Fixture::new();
        let path = f.open("binary");
        let bytes: Vec<_> = (0..CHUNK_BYTES)
            .map(|value| u8::try_from(value % 256).unwrap())
            .collect();
        for _ in 0..16 {
            assert!(f.writer.write("binary", &bytes));
            assert!(
                !f.writer.write("binary", &bytes),
                "only one unacknowledged chunk per stream"
            );
            assert!(!f.writer.finish("binary"));
            f.wait("binary", Phase::Ready);
        }
        assert_eq!(f.writer.bytes("binary"), 1_048_576);
        assert!(f.writer.finish("binary"));
        f.wait("binary", Phase::Complete);
        f.writer.release("binary");
        assert_eq!(fs::read(path).unwrap(), bytes.repeat(16));
    }
    #[test]
    fn admission_is_bounded_and_cancellation_revokes_queued_work() {
        let mut f = Fixture::new();
        let path = f.open("cancel");
        assert!(f.writer.write("cancel", b"partial"));
        f.writer.cancel("cancel");
        assert!(!f.writer.write("cancel", b"late"));
        assert!(!f.writer.finish("cancel"));
        // A subsequent acknowledged open is a causal worker barrier after cancellation.
        f.open("barrier");
        assert!(!path.exists());
        assert_eq!(f.writer.phase("cancel"), Phase::Cancelled);
        for index in 2..CAPACITY {
            assert!(f.writer.reserve(index.to_string()));
        }
        assert!(!f.writer.reserve("overflow".into()));
        f.writer.release("cancel");
        assert!(f.writer.reserve("replacement".into()));
    }
    #[test]
    fn existing_files_and_symlinks_are_never_opened_or_removed() {
        use std::os::unix::fs::symlink;
        let mut f = Fixture::new();
        let target = f.directory.join("original");
        fs::write(&target, b"preserved").unwrap();
        let link = f.directory.join("link");
        symlink(&target, &link).unwrap();
        for path in [&target, &link] {
            let id = path.file_name().unwrap().to_str().unwrap();
            assert!(f.writer.reserve(id.to_owned()));
            assert!(f.writer.begin(id, path.clone()));
            f.wait(id, Phase::Failed);
            assert_eq!(f.writer.error(id), Some(io::ErrorKind::AlreadyExists));
            f.writer.release(id);
        }
        assert_eq!(fs::read(&target).unwrap(), b"preserved");
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    }
    #[test]
    fn oversized_chunks_fail_without_writing_or_leaving_a_partial_file() {
        let mut f = Fixture::new();
        let path = f.open("oversized");
        assert!(!f.writer.write("oversized", &vec![1; CHUNK_BYTES + 1]));
        assert_eq!(f.writer.phase("oversized"), Phase::Failed);
        assert_eq!(
            f.writer.error("oversized"),
            Some(io::ErrorKind::InvalidData)
        );
        f.open("barrier");
        assert!(!path.exists());
    }
}
