//! Bounded address and response-name policy for explicit browser downloads.
use ferric_browser_core::ValidatedUrl;

pub(super) fn address(value: &str) -> Option<String> {
    let value = ValidatedUrl::parse(value.to_owned()).ok()?;
    let value = value.as_str().split('#').next()?;
    let (_, authority) = value.split_once("://")?;
    if !(value.starts_with("http://") || value.starts_with("https://"))
        || authority.split(['/', '?']).next()?.contains('@')
    {
        return None;
    }
    Some(value.to_owned())
}
pub(super) fn origin(value: &str) -> Option<String> {
    let value = address(value)?;
    let (scheme, rest) = value.split_once("://")?;
    Some(format!("{scheme}://{}", rest.split(['/', '?']).next()?))
}

fn percent_bytes(value: &str) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(value.len());
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            result.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            result.push(byte);
        }
    }
    Some(result)
}
fn parameter_value(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.starts_with('"') {
        return Some(value.to_owned());
    }
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        result.push(if character == '\\' {
            chars.next()?
        } else {
            character
        });
    }
    Some(result)
}
fn disposition_name(header: &str) -> Option<String> {
    if header.len() > 8192 || header.chars().any(char::is_control) {
        return None;
    }
    let mut parts = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (offset, character) in header.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if character == ';' && !quoted {
            parts.push(&header[start..offset]);
            start = offset + 1;
        }
    }
    if quoted {
        return None;
    }
    parts.push(&header[start..]);
    let mut plain = None;
    for part in parts.into_iter().skip(1).take(64) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let Some(value) = parameter_value(value) else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("filename*") {
            let name = (|| {
                let mut fields = value.splitn(3, '\'');
                let charset = fields.next()?;
                let _language = fields.next()?;
                let bytes = percent_bytes(fields.next()?)?;
                if charset.eq_ignore_ascii_case("utf-8") {
                    String::from_utf8(bytes).ok()
                } else if charset.eq_ignore_ascii_case("iso-8859-1") {
                    Some(bytes.into_iter().map(char::from).collect())
                } else {
                    None
                }
            })();
            if name.as_ref().is_some_and(|name| !name.is_empty()) {
                return name;
            }
        } else if key.trim().eq_ignore_ascii_case("filename")
            && plain.is_none()
            && !value.is_empty()
        {
            plain = Some(value);
        }
    }
    plain
}
pub(super) fn suggested_name(url: &str, disposition: &str, mime: &str) -> String {
    let name = disposition_name(disposition)
        .or_else(|| {
            let address = address(url)?;
            let (_, rest) = address.split_once("://")?;
            let path = rest.split_once('/')?.1.split('?').next()?;
            let name = path.rsplit('/').next()?;
            if name.is_empty() {
                None
            } else {
                String::from_utf8(percent_bytes(name)?).ok()
            }
        })
        .unwrap_or_else(|| "download".into());
    let mut name = ferric_browser_storage::sanitize_download_filename(&name);
    if !name.contains('.') {
        let suffix = match mime
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "text/plain" => ".txt",
            "text/html" => ".html",
            "application/pdf" => ".pdf",
            "application/zip" => ".zip",
            "image/png" => ".png",
            "image/jpeg" => ".jpg",
            _ => "",
        };
        name.push_str(suffix);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_network_addresses_without_embedded_credentials() {
        assert_eq!(
            address("HTTPS://EXAMPLE.TEST:443/a#comments").as_deref(),
            Some("https://example.test/a")
        );
        assert_eq!(
            origin("https://example.test/path?q=x").as_deref(),
            Some("https://example.test")
        );
        for value in [
            "https://user:password@example.test/",
            "file:///tmp/file",
            "javascript:alert(1)",
            "https://example.test/a\nb",
        ] {
            assert!(address(value).is_none(), "{value}");
        }
    }
    #[test]
    fn response_names_preserve_unicode_quoted_semicolons_and_language_tags() {
        assert_eq!(
            suggested_name(
                "https://example.test/fallback",
                "attachment; filename=old.pdf; filename*=UTF-8'en'R%C3%A9sum%C3%A9.pdf",
                "application/pdf"
            ),
            "Résumé.pdf"
        );
        assert_eq!(
            suggested_name(
                "https://example.test/fallback",
                "attachment; filename=\"Report; final.pdf\"",
                "application/pdf"
            ),
            "Report; final.pdf"
        );
        assert_eq!(
            suggested_name(
                "https://example.test/fallback",
                "attachment; filename*=iso-8859-1''caf%E9.txt",
                "text/plain"
            ),
            "café.txt"
        );
        assert_eq!(
            suggested_name(
                "https://example.test/fallback",
                "attachment; filename=plain.pdf; filename*=utf-8''bad%xy",
                "application/pdf"
            ),
            "plain.pdf"
        );
        assert_eq!(
            suggested_name("https://example.test/report", "", "APPLICATION/PDF"),
            "report.pdf"
        );
    }
    #[test]
    fn decoded_names_cannot_choose_a_directory_and_queries_never_become_names() {
        let name = suggested_name(
            "https://example.test/report?token=secret",
            "attachment; filename*=utf-8''..%2F..%2Freport.pdf",
            "application/pdf",
        );
        assert!(!name.contains('/') && !name.contains('\\'));
        assert_eq!(
            suggested_name(
                "https://example.test/Report%20final?token=secret",
                "",
                "application/pdf"
            ),
            "Report final.pdf"
        );
        assert_eq!(
            suggested_name("https://example.test/?token=secret", "", ""),
            "download"
        );
    }
}
