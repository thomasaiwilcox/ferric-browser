//! Bounded byte-stream handling for userscript subprocesses.

use std::io::Read;

/// Reads a userscript pipe without allowing an untrusted child to grow an
/// in-memory buffer beyond the declared protocol or diagnostic limit.
pub(super) fn read_bounded<R: Read>(mut reader: R, maximum: usize) -> Result<Vec<u8>, String> {
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
