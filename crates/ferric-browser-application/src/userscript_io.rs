//! Bounded byte-stream handling for userscript subprocesses.

use std::io::Read;

/// Reads a userscript pipe without allowing an untrusted child to grow an
/// in-memory buffer beyond the declared protocol or diagnostic limit.
///
/// # Errors
/// Returns an error on an I/O failure or when the byte limit is exceeded.
pub fn read_bounded<R: Read>(mut reader: R, maximum: usize) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("userscript pipe read failed: {error}"))?;
        if count == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(count) > maximum {
            return Err(format!("userscript output exceeds {maximum} bytes"));
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

/// Reads a regular UTF-8 source through the same descriptor used for its size
/// check. The streaming bound still holds if another process grows the file.
pub(crate) fn read_text(path: &std::path::Path, maximum: u64) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("userscript file is unavailable: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect userscript file: {error}"))?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(format!(
            "userscript file must be regular and at most {maximum} bytes"
        ));
    }
    let maximum =
        usize::try_from(maximum).map_err(|_| "userscript byte limit is invalid".to_owned())?;
    String::from_utf8(read_bounded(file, maximum)?)
        .map_err(|_| "userscript source is not UTF-8".into())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::read_bounded;

    #[test]
    fn bounds_untrusted_child_output_without_returning_a_partial_payload() {
        assert_eq!(read_bounded(Cursor::new(b"safe"), 4), Ok(b"safe".to_vec()));
        assert!(read_bounded(Cursor::new(b"oversized"), 4).is_err());
    }
}
