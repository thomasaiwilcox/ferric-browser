//! Failure categories shared by domain services, independent of wire protocols.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    InvalidArgument,
    Config,
    NotFound,
    StaleTarget,
    Denied,
    ConfirmationRequired,
    Unsupported,
    Busy,
    Timeout,
    Io,
    Storage,
    Engine,
    Cancelled,
}
