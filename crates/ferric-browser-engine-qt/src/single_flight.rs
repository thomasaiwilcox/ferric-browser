use std::{
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread::{self, JoinHandle},
};

const QUEUE_CAPACITY: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SubmitError {
    Busy,
    QueueFull,
    Stopped,
}

#[derive(Debug)]
pub(crate) enum Poll<T> {
    Pending,
    Ready(T),
    Stopped,
}

/// Owns the common lifecycle of a bounded worker that accepts one request at
/// a time. Domain wrappers remain responsible for request validation and for
/// translating lifecycle failures into user-facing messages.
pub(crate) struct SingleFlightWorker<Request, Response> {
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
    pub(crate) fn spawn(
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

    pub(crate) fn submit(&mut self, request: Request) -> Result<(), SubmitError> {
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

    pub(crate) fn poll(&mut self) -> Poll<Response> {
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

    #[inline]
    pub(crate) fn is_pending(&self) -> bool {
        self.pending
    }
}

impl<Request, Response> Drop for SingleFlightWorker<Request, Response> {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
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
