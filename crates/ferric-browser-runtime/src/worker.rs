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

/// Owns one bounded worker. Deferred workers allocate their channels and start
/// their native thread only when the first request is submitted.
pub struct SingleFlightWorker<Request, Response> {
    state: Option<WorkerState<Request, Response>>,
}

type Starter<Request, Response> =
    Box<dyn FnOnce() -> std::io::Result<RunningWorker<Request, Response>> + Send>;

enum WorkerState<Request, Response> {
    Deferred(Starter<Request, Response>),
    Running(RunningWorker<Request, Response>),
}

struct RunningWorker<Request, Response> {
    sender: Option<SyncSender<Request>>,
    receiver: Receiver<Response>,
    pending: bool,
    join: Option<JoinHandle<()>>,
}

impl<Request: Send + 'static, Response: Send + 'static> RunningWorker<Request, Response> {
    fn spawn(
        name: String,
        mut handle: impl FnMut(Request) -> Response + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, requests) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (responses, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let join = thread::Builder::new().name(name).spawn(move || {
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
}

impl<Request, Response> SingleFlightWorker<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    /// Starts a worker immediately with an owned request handler.
    ///
    /// # Errors
    /// Returns an I/O error if the operating system cannot start the thread.
    pub fn spawn(
        thread_name: &str,
        handle: impl FnMut(Request) -> Response + Send + 'static,
    ) -> std::io::Result<Self> {
        Ok(Self {
            state: Some(WorkerState::Running(RunningWorker::spawn(
                thread_name.into(),
                handle,
            )?)),
        })
    }

    /// Prepares an owned handler without starting a thread or allocating queues.
    /// A failure to start at first submission returns [`SubmitError::Stopped`]
    /// before the request is admitted. Domain wrappers retain error reporting.
    #[must_use]
    pub fn deferred(
        thread_name: &str,
        handle: impl FnMut(Request) -> Response + Send + 'static,
    ) -> Self {
        let name = thread_name.to_owned();
        Self {
            state: Some(WorkerState::Deferred(Box::new(move || {
                RunningWorker::spawn(name, handle)
            }))),
        }
    }

    /// Submits work without waiting for job completion. A deferred worker starts
    /// its thread first; subsequent submissions reuse that owner's thread.
    ///
    /// # Errors
    /// Rejects work while another request is pending, when the queue is full,
    /// or after the worker has stopped, including a failed deferred start.
    pub fn submit(&mut self, request: Request) -> Result<(), SubmitError> {
        let state = self.state.take().ok_or(SubmitError::Stopped)?;
        let running = match state {
            WorkerState::Deferred(start) => start().map_err(|_| SubmitError::Stopped)?,
            WorkerState::Running(worker) => worker,
        };
        self.state = Some(WorkerState::Running(running));
        let Some(WorkerState::Running(worker)) = self.state.as_mut() else {
            return Err(SubmitError::Stopped);
        };
        if worker.pending {
            return Err(SubmitError::Busy);
        }
        let sender = worker.sender.as_ref().ok_or(SubmitError::Stopped)?;
        match sender.try_send(request) {
            Ok(()) => {
                worker.pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
        }
    }

    pub fn poll(&mut self) -> Poll<Response> {
        match self.state.as_mut() {
            Some(WorkerState::Deferred(_)) => Poll::Pending,
            None => Poll::Stopped,
            Some(WorkerState::Running(worker)) => match worker.receiver.try_recv() {
                Ok(response) => {
                    worker.pending = false;
                    Poll::Ready(response)
                }
                Err(TryRecvError::Empty) => Poll::Pending,
                Err(TryRecvError::Disconnected) => {
                    worker.pending = false;
                    Poll::Stopped
                }
            },
        }
    }

    /// Retires the owner without waiting for an admitted job. Its inputs and
    /// durability leases remain on the running thread; no UI object is captured.
    pub fn detach(mut self) {
        if let Some(WorkerState::Running(worker)) = self.state.as_mut() {
            worker.sender.take();
            worker.join.take();
        }
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self.state.as_ref(), Some(WorkerState::Running(worker)) if worker.pending)
    }
}

impl<Request, Response> Drop for RunningWorker<Request, Response> {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join) = self.join.take() {
            // Never wait for filesystem/process I/O on the owner's thread.
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
    fn deferred_start_waits_for_submission_and_reuses_its_owner_thread() {
        let mut worker =
            SingleFlightWorker::deferred("deferred-test", |()| std::thread::current().id());
        assert!(matches!(
            worker.state,
            Some(super::WorkerState::Deferred(_))
        ));
        assert!(matches!(worker.poll(), Poll::Pending));
        assert!(!worker.is_pending());
        worker.submit(()).unwrap();
        assert!(worker.is_pending());
        assert_eq!(worker.submit(()), Err(SubmitError::Busy));
        let first = wait_for_any_result(&mut worker);
        worker.submit(()).unwrap();
        assert_eq!(first, wait_for_any_result(&mut worker));
    }

    #[test]
    fn a_deferred_start_failure_never_admits_a_request() {
        let mut worker = SingleFlightWorker::<(), ()> {
            state: Some(super::WorkerState::Deferred(Box::new(|| {
                Err(std::io::Error::other("injected start failure"))
            }))),
        };
        assert_eq!(worker.submit(()), Err(SubmitError::Stopped));
        assert!(!worker.is_pending());
        assert!(matches!(worker.poll(), Poll::Stopped));
        assert_eq!(worker.submit(()), Err(SubmitError::Stopped));
    }

    #[test]
    fn unused_deferred_handler_is_released_without_invocation() {
        let owned = std::sync::Arc::new(());
        let captured = owned.clone();
        let worker = SingleFlightWorker::deferred("unused-test", move |()| {
            drop(captured.clone());
            panic!("unused handler invoked")
        });
        assert_eq!(std::sync::Arc::strong_count(&owned), 2);
        worker.detach();
        assert_eq!(std::sync::Arc::strong_count(&owned), 1);
    }

    fn wait_for_any_result<T: Send + 'static>(worker: &mut SingleFlightWorker<(), T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match worker.poll() {
                Poll::Ready(value) => return value,
                Poll::Pending if Instant::now() < deadline => std::thread::yield_now(),
                _ => panic!("worker did not deliver its response"),
            }
        }
    }

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
        for deferred in [false, true] {
            let (release, held) = std::sync::mpsc::channel();
            let (started, entered) = std::sync::mpsc::channel();
            let (dropped, retired) = std::sync::mpsc::channel();
            let owner = std::thread::spawn(move || {
                let handle = move |()| {
                    started.send(()).unwrap();
                    held.recv().ok();
                };
                let mut worker = if deferred {
                    SingleFlightWorker::deferred("retirement-test", handle)
                } else {
                    SingleFlightWorker::spawn("retirement-test", handle).unwrap()
                };
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
    }

    #[test]
    fn deferred_owners_keep_separate_native_threads() {
        let mut first =
            SingleFlightWorker::deferred("first-owner", |()| std::thread::current().id());
        let mut second =
            SingleFlightWorker::deferred("second-owner", |()| std::thread::current().id());
        first.submit(()).unwrap();
        second.submit(()).unwrap();
        assert_ne!(
            wait_for_any_result(&mut first),
            wait_for_any_result(&mut second)
        );
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
