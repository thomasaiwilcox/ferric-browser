//! Nonblocking local IPC with one deadline covering connect, write and EOF.
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{ffi::OsStrExt, net::UnixStream},
    },
    path::Path,
    time::{Duration, Instant},
};

fn timeout() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "Compositor request deadline expired",
    )
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(timeout)
}

#[allow(unsafe_code)]
fn wait(stream: &UnixStream, events: i16, deadline: Instant) -> io::Result<()> {
    loop {
        let millis = i32::try_from(remaining(deadline)?.as_millis().max(1)).unwrap_or(i32::MAX);
        let mut descriptor = libc::pollfd {
            fd: stream.as_raw_fd(),
            events,
            revents: 0,
        };
        // SAFETY: one valid pollfd owned on this stack for the whole syscall.
        let result = unsafe { libc::poll(&raw mut descriptor, 1, millis) };
        if result > 0 {
            remaining(deadline)?;
            return Ok(());
        }
        if result == 0 {
            return Err(timeout());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[allow(unsafe_code)]
fn connect(path: &Path, deadline: Instant) -> io::Result<UnixStream> {
    // SAFETY: sockaddr_un is a C integer/byte-array POD; zero is valid for all
    // its fields. The pathname below is bounded and explicitly NUL-terminated.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid local compositor socket path",
        ));
    }
    address.sun_family = libc::AF_UNIX
        .try_into()
        .map_err(|_| io::ErrorKind::InvalidInput)?;
    for (output, input) in address.sun_path.iter_mut().zip(bytes) {
        *output = libc::c_char::from_ne_bytes([*input]);
    }
    // SAFETY: no pointer arguments; an error yields -1, otherwise a fresh fd.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raw is a fresh, uniquely owned descriptor from socket above.
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    let stream = UnixStream::from(descriptor);
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    let length = libc::socklen_t::try_from(length).map_err(|_| io::ErrorKind::InvalidInput)?;
    loop {
        remaining(deadline)?;
        // SAFETY: address is initialized, remains alive, and length covers its
        // family and bounded pathname without exceeding sockaddr_un.
        let result =
            unsafe { libc::connect(stream.as_raw_fd(), (&raw const address).cast(), length) };
        if result == 0 {
            return Ok(stream);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EISCONN) => return Ok(stream),
            Some(libc::EINPROGRESS | libc::EALREADY) => {
                wait(&stream, libc::POLLOUT, deadline)?;
                if let Some(error) = stream.take_error()? {
                    return Err(error);
                }
                return Ok(stream);
            }
            Some(libc::EAGAIN) => {
                // AF_UNIX backlog exhaustion does not start a pending connect.
                // Retry on the worker with bounded sleeps and the same deadline.
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(2)));
            }
            Some(libc::EINTR) => {}
            _ => return Err(error),
        }
    }
}

fn connect_with_backoff(path: &Path, deadline: Instant) -> io::Result<UnixStream> {
    for attempt in 0..3_u64 {
        match connect(path, deadline) {
            Ok(stream) => return Ok(stream),
            Err(error)
                if attempt < 2
                    && matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::TimedOut
                    ) =>
            {
                // Each failed attempt dropped its fd. Preserve restart recovery
                // inside the same absolute deadline, never after sending bytes.
                std::thread::sleep(
                    remaining(deadline)?.min(Duration::from_millis(20 * (attempt + 1))),
                );
            }
            Err(error) => return Err(error),
        }
    }
    Err(timeout())
}

#[derive(Debug)]
pub(super) enum DispatchError {
    NotSent(io::Error),
    PossiblySent(io::Error),
}

impl DispatchError {
    fn into_io(self) -> io::Error {
        match self {
            Self::NotSent(error) | Self::PossiblySent(error) => error,
        }
    }
}

pub(super) struct PreparedRequest {
    stream: UnixStream,
    deadline: Instant,
}

pub(super) struct PendingResponse(PreparedRequest);

impl PreparedRequest {
    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(super) fn connect(path: &Path, budget: Duration) -> io::Result<Self> {
        let deadline = Instant::now() + budget;
        Ok(Self {
            stream: connect_with_backoff(path, deadline)?,
            deadline,
        })
    }

    /// One nonblocking write; no polling, retry, sleep or Qt event processing.
    pub(super) fn dispatch(mut self, command: &[u8]) -> Result<PendingResponse, DispatchError> {
        if command.len() > 64 * 1024 {
            return Err(DispatchError::NotSent(io::ErrorKind::InvalidInput.into()));
        }
        remaining(self.deadline).map_err(DispatchError::NotSent)?;
        match self.stream.write(command).map_err(DispatchError::NotSent)? {
            0 => return Err(DispatchError::NotSent(io::ErrorKind::WriteZero.into())),
            written if written == command.len() => {}
            _ => return Err(DispatchError::PossiblySent(io::ErrorKind::WriteZero.into())),
        }
        self.stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(DispatchError::PossiblySent)?;
        Ok(PendingResponse(self))
    }
}

impl PendingResponse {
    pub(super) fn finish(self, maximum: usize) -> io::Result<Vec<u8>> {
        let PreparedRequest {
            mut stream,
            deadline,
        } = self.0;
        read_response(&mut stream, deadline, maximum)
    }
}

pub(super) fn request(
    path: &Path,
    command: &[u8],
    maximum: usize,
    budget: Duration,
) -> io::Result<Vec<u8>> {
    PreparedRequest::connect(path, budget)?
        .dispatch(command)
        .map_err(DispatchError::into_io)?
        .finish(maximum)
}

fn read_response(
    stream: &mut UnixStream,
    deadline: Instant,
    maximum: usize,
) -> io::Result<Vec<u8>> {
    let mut response = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        remaining(deadline)?;
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(response),
            Ok(count) => {
                if response.len().saturating_add(count) > maximum {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Compositor reply exceeds its bound",
                    ));
                }
                response.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                wait(stream, libc::POLLIN, deadline)?;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::net::UnixListener, thread};

    #[test]
    fn expired_queued_dispatch_sends_no_bytes() {
        let root = std::env::temp_dir().join(format!("ferric-expired-{}", crate::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("socket");
        let server = UnixListener::bind(&path).unwrap();
        let prepared = PreparedRequest::connect(&path, Duration::from_millis(10)).unwrap();
        thread::sleep(Duration::from_millis(20));
        assert!(
            matches!(prepared.dispatch(b"must-not-be-sent"), Err(DispatchError::NotSent(error)) if error.kind() == io::ErrorKind::TimedOut)
        );
        let (mut stream, _) = server.accept().unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).unwrap();
        assert!(received.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[allow(unsafe_code)]
    fn a_full_unix_backlog_has_a_connect_deadline() {
        let root = std::env::temp_dir().join(format!("ferric-backlog-{}", crate::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("socket");
        let server = UnixListener::bind(&path).unwrap();
        // SAFETY: this test owns the live listener; listen changes only its backlog.
        assert_eq!(unsafe { libc::listen(server.as_raw_fd(), 0) }, 0);
        let occupied = PreparedRequest::connect(&path, Duration::from_millis(80)).unwrap();
        let start = Instant::now();
        assert!(
            matches!(PreparedRequest::connect(&path, Duration::from_millis(80)), Err(error) if error.kind() == io::ErrorKind::TimedOut)
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(occupied);
        drop(server);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[allow(unsafe_code)]
    fn gui_dispatch_is_one_nonblocking_write_and_never_drains_a_full_buffer() {
        let root = std::env::temp_dir().join(format!("ferric-send-{}", crate::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("socket");
        let server = UnixListener::bind(&path).unwrap();
        let prepared = PreparedRequest::connect(&path, Duration::from_secs(1)).unwrap();
        let fd = prepared.stream.as_raw_fd();
        // SAFETY: fd remains uniquely owned/live, F_GETFL has no variadic argument.
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        let small: libc::c_int = 4096;
        // SAFETY: the live socket and initialized integer remain valid; its
        // exact integer size is supplied, without exposing aliases to the fd.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_SNDBUF,
                    (&raw const small).cast(),
                    libc::socklen_t::try_from(std::mem::size_of_val(&small)).unwrap(),
                )
            },
            0
        );
        let start = Instant::now();
        let result = prepared.dispatch(&vec![b'x'; 64 * 1024]);
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_millis(500));
        let (mut stream, _) = server.accept().unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).unwrap();
        assert!(received.len() < 64 * 1024);
        assert_eq!(
            matches!(result, Err(DispatchError::PossiblySent(_))),
            !received.is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trickle_reply_cannot_extend_total_request_deadline() {
        let root = std::env::temp_dir().join(format!("ferric-deadline-{}", crate::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("socket");
        let server = UnixListener::bind(&path).unwrap();
        let peer = thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            let mut command = Vec::new();
            stream.read_to_end(&mut command).unwrap();
            assert_eq!(command, b"owned-test");
            for _ in 0..30 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        let error = request(&path, b"owned-test", 64, Duration::from_millis(80)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(1));
        peer.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
