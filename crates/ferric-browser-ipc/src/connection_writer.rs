//! All output for one authenticated connection shares whole-frame ownership.
use crate::{FrameError, MAX_FRAME_BYTES};
use std::{
    io::{self, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct Gate {
    busy: bool,
    failed: bool,
}

#[derive(Debug)]
struct Inner {
    stream: UnixStream,
    gate: Mutex<Gate>,
    wake: Condvar,
}

/// Clones share one output gate and connection lifetime. Construct this once
/// per connection; never wrap separately cloned raw streams for its producers.
#[derive(Clone, Debug)]
pub struct ConnectionWriter(Arc<Inner>);

impl From<UnixStream> for ConnectionWriter {
    fn from(stream: UnixStream) -> Self {
        Self::new(stream)
    }
}

impl ConnectionWriter {
    #[must_use]
    pub fn new(stream: UnixStream) -> Self {
        Self(Arc::new(Inner {
            stream,
            gate: Mutex::new(Gate::default()),
            wake: Condvar::new(),
        }))
    }

    /// Writes one intact bounded frame. One absolute deadline includes waiting
    /// for the previous writer, cache publication and every partial syscall.
    ///
    /// # Errors
    /// Rejects invalid payloads. Timeout, poisoned synchronization or failed
    /// output closes the connection so no producer can append to a partial frame.
    pub fn write_frame(&self, payload: &[u8]) -> Result<(), FrameError> {
        checked_length(payload)?;
        self.acquire()?.write(payload)
    }

    /// Retires a failed connection and wakes all waiting output producers.
    pub fn close(&self) {
        {
            let mut gate = self
                .0
                .gate
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            gate.failed = true;
        }
        let _ = self.0.stream.shutdown(Shutdown::Both);
        self.0.wake.notify_all();
    }

    pub(super) fn acquire(&self) -> Result<FrameLease<'_>, FrameError> {
        let deadline = Instant::now() + WRITE_TIMEOUT;
        let mut gate = match self.0.gate.lock() {
            Ok(gate) => gate,
            Err(error) => {
                drop(error.into_inner());
                self.close();
                return Err(io::Error::other("IPC writer gate is poisoned").into());
            }
        };
        while gate.busy && !gate.failed {
            let Some(timeout) = remaining(deadline) else {
                drop(gate);
                self.close();
                return Err(timed_out());
            };
            gate = match self.0.wake.wait_timeout(gate, timeout) {
                Ok((gate, _)) => gate,
                Err(error) => {
                    drop(error.into_inner());
                    self.close();
                    return Err(io::Error::other("IPC writer wait is poisoned").into());
                }
            };
        }
        if gate.failed {
            return Err(
                io::Error::new(io::ErrorKind::BrokenPipe, "IPC connection is closed").into(),
            );
        }
        if remaining(deadline).is_none() {
            drop(gate);
            self.close();
            return Err(timed_out());
        }
        gate.busy = true;
        Ok(FrameLease {
            writer: self,
            deadline,
            complete: false,
        })
    }
}

pub(super) struct FrameLease<'a> {
    writer: &'a ConnectionWriter,
    deadline: Instant,
    complete: bool,
}

impl FrameLease<'_> {
    pub(super) fn write(mut self, payload: &[u8]) -> Result<(), FrameError> {
        let prefix = checked_length(payload)?.to_be_bytes();
        let mut socket = &self.writer.0.stream;
        for mut bytes in [&prefix[..], payload] {
            while !bytes.is_empty() {
                let timeout = remaining(self.deadline).ok_or_else(timed_out)?;
                socket
                    .set_write_timeout(Some(timeout))
                    .map_err(FrameError::from)?;
                match socket.write(bytes) {
                    Ok(0) => {
                        return Err(
                            io::Error::new(io::ErrorKind::WriteZero, "IPC stream closed").into(),
                        );
                    }
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
                    Err(error) => return Err(error.into()),
                }
            }
        }
        self.complete = true;
        Ok(())
    }
}

impl Drop for FrameLease<'_> {
    fn drop(&mut self) {
        if !self.complete {
            // Also covers unwinding or an early return before any output.
            self.writer.close();
        }
        {
            let mut gate = self
                .writer
                .0
                .gate
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            gate.busy = false;
        }
        self.writer.0.wake.notify_all();
    }
}

fn remaining(deadline: Instant) -> Option<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|timeout| !timeout.is_zero())
}

fn timed_out() -> FrameError {
    io::Error::new(io::ErrorKind::TimedOut, "IPC frame deadline elapsed").into()
}

fn checked_length(payload: &[u8]) -> Result<u32, FrameError> {
    if payload.is_empty() {
        return Err(FrameError::ZeroLength);
    }
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(payload.len()));
    }
    u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge(payload.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_frame;
    use std::{os::fd::AsRawFd, thread};

    #[test]
    fn dropping_an_unfinished_frame_closes_the_connection() {
        let (server, mut peer) = UnixStream::pair().unwrap();
        let writer = ConnectionWriter::new(server);
        drop(writer.acquire().unwrap());
        assert!(writer.write_frame(b"later").is_err());
        assert!(read_frame(&mut peer).unwrap().is_none());
    }

    #[test]
    fn waiting_for_frame_ownership_has_an_absolute_deadline() {
        let (server, mut peer) = UnixStream::pair().unwrap();
        let writer = ConnectionWriter::new(server);
        let held = writer.acquire().unwrap();
        let next = writer.clone();
        let started = Instant::now();
        let error = thread::spawn(move || next.write_frame(b"next"))
            .join()
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            FrameError::Io {
                kind: io::ErrorKind::TimedOut,
                ..
            }
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        drop(held);
        assert!(writer.write_frame(b"later").is_err());
        assert!(read_frame(&mut peer).unwrap().is_none());
    }

    #[test]
    fn poisoned_output_ownership_fails_closed() {
        let (server, mut peer) = UnixStream::pair().unwrap();
        let writer = ConnectionWriter::new(server);
        let poisoned = writer.clone();
        assert!(
            thread::spawn(move || {
                let _gate = poisoned.0.gate.lock().unwrap();
                panic!("fixture poisons output gate");
            })
            .join()
            .is_err()
        );
        assert!(writer.write_frame(b"later").is_err());
        assert!(read_frame(&mut peer).unwrap().is_none());
    }

    #[test]
    fn a_partial_frame_failure_prevents_every_later_frame() {
        let (server, mut peer) = UnixStream::pair().unwrap();
        let capacity: libc::c_int = 4096;
        // SAFETY: the owned live socket descriptor accepts SO_SNDBUF; the
        // pointer and length describe this initialized integer for the call.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    server.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_SNDBUF,
                    std::ptr::from_ref(&capacity).cast(),
                    std::mem::size_of_val(&capacity).try_into().unwrap(),
                )
            },
            0
        );
        let writer = ConnectionWriter::new(server);
        let blocked = writer.clone();
        let error = thread::spawn(move || blocked.write_frame(&vec![b'x'; MAX_FRAME_BYTES]))
            .join()
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            FrameError::Io {
                kind: io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock,
                ..
            }
        ));
        assert!(matches!(
            read_frame(&mut peer),
            Err(FrameError::Io {
                kind: io::ErrorKind::UnexpectedEof,
                ..
            })
        ));
        assert!(writer.write_frame(b"later").is_err());
    }
}
