#[derive(Debug)]
pub(crate) enum AdmissionError {
    InvalidEnvelope,
    InvalidOrigin,
    PublicSuffixListUnavailable,
    UnsupportedOptions,
    InvalidRequest,
}

impl AdmissionError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::InvalidEnvelope => "invalid_envelope",
            Self::InvalidOrigin => "invalid_origin",
            Self::PublicSuffixListUnavailable => "unavailable",
            Self::UnsupportedOptions => "unsupported_options",
            Self::InvalidRequest => "invalid_request",
        }
    }
}
