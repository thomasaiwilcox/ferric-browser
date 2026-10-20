use std::{
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread::{self, JoinHandle},
};

const QUEUE_CAPACITY: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    Busy,
    QueueFull,
    Stopped,
}

#[derive(Debug)]
pub enum Poll<T> {
    Pending,
    Ready(T),
    Stopped,
}

/// Owns the common lifecycle of a bounded worker that accepts one request at
/// a time. Domain wrappers remain responsible for request validation and for
/// translating lifecycle failures into user-facing messages.
pub struct SingleFlightWorker<Request, Response> {
    sender: Option<SyncSender<Request>>,
    receiver: Receiver<Response>,
    pending: bool,
    join: Option<JoinHandle<()>>,
}

impl<Request, Response> SingleFlightWorker<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    /// Starts a worker with an owned request handler.
    ///
    /// # Errors
    /// Returns an I/O error if the operating system cannot start the thread.
    pub fn spawn(
        thread_name: &str,
        mut handle: impl FnMut(Request) -> Response + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, requests) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (responses, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let join = thread::Builder::new()
            .name(thread_name.into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    if responses.send(handle(request)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            pending: false,
            join: Some(join),
        })
    }

    /// Submits work without blocking the caller.
    ///
    /// # Errors
    /// Rejects work while another request is pending, when the queue is full,
    /// or after the worker has stopped.
    pub fn submit(&mut self, request: Request) -> Result<(), SubmitError> {
        if self.pending {
            return Err(SubmitError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(SubmitError::Stopped)?;
        match sender.try_send(request) {
            Ok(()) => {
                self.pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
        }
    }

    pub fn poll(&mut self) -> Poll<Response> {
        match self.receiver.try_recv() {
            Ok(response) => {
                self.pending = false;
                Poll::Ready(response)
            }
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.pending = false;
                Poll::Stopped
            }
        }
    }

    /// Stop accepting work without waiting for an in-flight owned job.
    /// Its inputs and any required durability leases remain on the worker;
    /// no UI object may be captured.
    pub fn detach(mut self) {
        self.sender.take();
        self.join.take();
    }

    #[must_use]
    #[inline]
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

impl<Request, Response> Drop for SingleFlightWorker<Request, Response> {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join) = self.join.take() {
            // A worker owns every input and never captures a GUI object. Closing
            // both channels retires it after the current job; destructors must
            // not wait for filesystem or process I/O on the calling thread.
            if join.is_finished() {
                let _ = join.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Poll, SingleFlightWorker, SubmitError};
    use std::time::{Duration, Instant};

    #[test]
    fn enforces_single_flight_and_accepts_the_next_request_after_polling() {
        let mut worker = SingleFlightWorker::spawn("single-flight-test", |value| value * 2)
            .expect("worker starts");

        worker.submit(21).expect("first request");
        assert_eq!(worker.submit(22), Err(SubmitError::Busy));
        assert_eq!(wait_for_result(&mut worker), 42);

        worker.submit(3).expect("request after polling");
        assert_eq!(wait_for_result(&mut worker), 6);
    }

    #[test]
    fn dropping_a_busy_worker_does_not_wait_for_its_owned_job() {
        let (release, held) = std::sync::mpsc::channel();
        let (started, entered) = std::sync::mpsc::channel();
        let (dropped, retired) = std::sync::mpsc::channel();
        let owner = std::thread::spawn(move || {
            let mut worker = SingleFlightWorker::spawn("retirement-test", move |()| {
                started.send(()).unwrap();
                held.recv().ok();
            })
            .unwrap();
            worker.submit(()).unwrap();
            entered.recv().unwrap();
            drop(worker);
            dropped.send(()).unwrap();
        });
        let result = retired.recv_timeout(Duration::from_secs(1));
        let _ = release.send(());
        owner.join().unwrap();
        result.expect("retirement must not wait for the job release");
    }

    fn wait_for_result(worker: &mut SingleFlightWorker<i32, i32>) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match worker.poll() {
                Poll::Ready(value) => return value,
                Poll::Pending if Instant::now() < deadline => std::thread::yield_now(),
                Poll::Pending => panic!("worker did not answer before the deadline"),
                Poll::Stopped => panic!("worker stopped before answering"),
            }
        }
    }
}
