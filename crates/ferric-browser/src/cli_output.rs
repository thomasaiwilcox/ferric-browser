//! Fallible CLI output, kept distinct from failures on the browser connection.

use std::{
    fmt,
    io::{self, Write},
};

use crate::cli_error::CliError;

pub(super) fn line(message: fmt::Arguments<'_>) -> Result<(), CliError> {
    write_line(&mut io::stdout().lock(), message).map_err(CliError::Output)
}

// Failure reporting must preserve the original exit status even when stderr
// has also been closed by a pipeline reader.
pub(super) fn error_line(message: fmt::Arguments<'_>) {
    let _ = write_line(&mut io::stderr().lock(), message);
}

fn write_line(output: &mut impl Write, message: fmt::Arguments<'_>) -> io::Result<()> {
    writeln!(output, "{message}")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_ipc::ErrorCode;

    struct FlushFailure(Vec<u8>);

    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn lines_preserve_formatting_unicode_and_empty_lines() {
        let mut output = Vec::new();
        write_line(&mut output, format_args!("Ferric — {}", 42)).unwrap();
        write_line(&mut output, format_args!("")).unwrap();
        assert_eq!(output, "Ferric — 42\n\n".as_bytes());
    }

    #[test]
    fn a_buffered_write_does_not_hide_a_flush_failure() {
        let mut output = FlushFailure(Vec::new());
        let error = write_line(&mut output, format_args!("accepted output")).unwrap_err();
        assert_eq!(output.0, b"accepted output\n");
        assert!(CliError::Output(error).is_broken_pipe());
    }

    #[test]
    fn other_output_failures_keep_the_io_status_and_hide_local_details() {
        let error = CliError::Output(io::Error::other("private-output-path"));
        assert!(!error.is_broken_pipe());
        assert_eq!(error.code(), ErrorCode::Io);
        assert_eq!(error.status(), 1);
        let public = error.into_protocol();
        assert_eq!(public.code, "E_IO");
        assert!(
            !serde_json::to_string(&public)
                .unwrap()
                .contains("private-output-path")
        );
    }

    #[test]
    fn transport_errors_are_not_mistaken_for_a_closed_output_pipe() {
        let error = CliError::local(ErrorCode::Io, "Broken pipe", "browser connection closed");
        assert!(!error.is_broken_pipe());
        assert_eq!(error.status(), 1);
    }
}
