use std::fmt;

const MAX_URL_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedUrl(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UrlError {
    Empty,
    TooLong,
    ControlCharacter,
    MissingScheme,
    UnsupportedScheme(String),
    Malformed(String),
}

impl fmt::Display for UrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("URL is empty"),
            Self::TooLong => formatter.write_str("URL exceeds the size limit"),
            Self::ControlCharacter => formatter.write_str("URL contains a control character"),
            Self::MissingScheme => formatter.write_str("URL has no scheme"),
            Self::UnsupportedScheme(scheme) => {
                write!(formatter, "unsupported URL scheme: {scheme}")
            }
            Self::Malformed(reason) => write!(formatter, "malformed URL: {reason}"),
        }
    }
}

impl std::error::Error for UrlError {}

impl ValidatedUrl {
    /// Validates and canonicalizes a URL at an application boundary.
    ///
    /// The canonicalizer changes only policy-neutral URL structure: scheme and
    /// DNS host casing, trailing DNS dots, IDN labels, and default ports. It
    /// never decodes or reorders path, query, or fragment bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, control-character-containing,
    /// schemeless, malformed, or unsupported URLs.
    pub fn parse(value: impl Into<String>) -> Result<Self, UrlError> {
        Ok(Self(canonicalize(&value.into())?))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ValidatedUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Canonicalizes a supported browser URL while preserving escaped components.
///
/// This is intentionally a small, non-networking parser. It is used before a
/// URL enters policy or reducer state, so ambiguous authorities are rejected
/// instead of being guessed at by a policy caller.
///
/// # Errors
///
/// Returns a typed error when the scheme, authority, port, host, or bounded
/// URL size is invalid.
pub fn canonicalize(value: &str) -> Result<String, UrlError> {
    if value.is_empty() {
        return Err(UrlError::Empty);
    }
    if value.len() > MAX_URL_BYTES {
        return Err(UrlError::TooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(UrlError::ControlCharacter);
    }
    let Some((raw_scheme, remainder)) = value.split_once(':') else {
        return Err(UrlError::MissingScheme);
    };
    if raw_scheme.is_empty()
        || !raw_scheme.chars().enumerate().all(|(index, character)| {
            if index == 0 {
                character.is_ascii_alphabetic()
            } else {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            }
        })
    {
        return Err(UrlError::Malformed("scheme is invalid".into()));
    }
    let scheme = raw_scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "file" | "about") {
        return Err(UrlError::UnsupportedScheme(scheme));
    }

    if matches!(scheme.as_str(), "http" | "https") {
        if !remainder.starts_with("//") {
            return Err(UrlError::Malformed(
                "HTTP(S) URL must contain an authority".into(),
            ));
        }
        let after_slashes = &remainder[2..];
        let authority_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..authority_end];
        let tail = &after_slashes[authority_end..];
        let authority = canonical_authority(authority, &scheme, true)?;
        reject_raw_url_whitespace(tail)?;
        return Ok(format!("{scheme}://{authority}{tail}"));
    }

    if scheme == "file" && remainder.starts_with("//") {
        let after_slashes = &remainder[2..];
        let authority_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..authority_end];
        let tail = &after_slashes[authority_end..];
        let authority = if authority.is_empty() {
            String::new()
        } else {
            canonical_authority(authority, &scheme, false)?
        };
        reject_raw_url_whitespace(tail)?;
        return Ok(format!("{scheme}://{authority}{tail}"));
    }

    reject_raw_url_whitespace(remainder)?;
    Ok(format!("{scheme}:{remainder}"))
}

/// Returns the canonical HTTP(S) security origin for a URL.
///
/// User information, paths, queries, and fragments are excluded. Opaque,
/// file:, and malformed URLs have no durable site origin.
#[must_use]
pub fn canonical_origin(value: &str) -> Option<String> {
    let canonical = canonicalize(value).ok()?;
    let (scheme, remainder) = canonical.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if authority.is_empty() || authority.contains(['@', '\\']) {
        return None;
    }
    Some(format!("{scheme}://{authority}"))
}

fn canonical_authority(
    authority: &str,
    scheme: &str,
    allow_userinfo: bool,
) -> Result<String, UrlError> {
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| character.is_ascii_whitespace())
    {
        return Err(UrlError::Malformed(
            "authority is empty or contains whitespace".into(),
        ));
    }
    let (userinfo, host_port) = if allow_userinfo {
        authority
            .rsplit_once('@')
            .map_or(("", authority), |(userinfo, host_port)| {
                (userinfo, host_port)
            })
    } else {
        ("", authority)
    };
    if (!allow_userinfo && authority.contains('@')) || userinfo.contains(['/', '\\', '?', '#']) {
        return Err(UrlError::Malformed(
            "authority user information is invalid".into(),
        ));
    }
    let (host, port) = split_host_port(host_port)?;
    let host = canonical_host(host)?;
    let port = canonical_port(port, scheme)?;
    let authority = if let Some(port) = port {
        format_host(&host, host_port.starts_with('['), port)
    } else {
        format_host_without_port(&host, host_port.starts_with('['))
    };
    if userinfo.is_empty() {
        Ok(authority)
    } else {
        Ok(format!("{userinfo}@{authority}"))
    }
}

fn split_host_port(value: &str) -> Result<(&str, Option<&str>), UrlError> {
    if value.starts_with('[') {
        let close = value
            .find(']')
            .ok_or_else(|| UrlError::Malformed("IPv6 authority is missing ]".into()))?;
        let host = &value[1..close];
        if host.is_empty() || host.contains(['[', ']']) {
            return Err(UrlError::Malformed("IPv6 host is invalid".into()));
        }
        let suffix = &value[close + 1..];
        let port =
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':').ok_or_else(|| {
                    UrlError::Malformed("IPv6 authority suffix is invalid".into())
                })?)
            };
        return Ok((host, port));
    }
    if value.contains('[') || value.contains(']') {
        return Err(UrlError::Malformed("IPv6 brackets are unbalanced".into()));
    }
    match value.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() => {
            if !port.chars().all(|character| character.is_ascii_digit()) {
                return Err(UrlError::Malformed("port is not numeric".into()));
            }
            Ok((host, Some(port)))
        }
        Some((_, _)) => Err(UrlError::Malformed("port is empty".into())),
        None => Ok((value, None)),
    }
}

fn canonical_host(host: &str) -> Result<String, UrlError> {
    if host.is_empty() || host.contains(['/', '\\', '@', '?', '#', '%']) {
        return Err(UrlError::Malformed("host is invalid".into()));
    }
    let host = host.trim_end_matches('.');
    if host.is_empty() {
        return Err(UrlError::Malformed("host is empty".into()));
    }
    if host.contains(':') {
        if !host
            .chars()
            .all(|character| character.is_ascii_hexdigit() || matches!(character, ':' | '.'))
        {
            return Err(UrlError::Malformed(
                "IPv6 host contains invalid characters".into(),
            ));
        }
        return Ok(host.to_ascii_lowercase());
    }
    let mut labels = Vec::new();
    for label in host.split('.') {
        if label.is_empty() {
            return Err(UrlError::Malformed("host contains an empty label".into()));
        }
        labels.push(canonical_host_label(label)?);
    }
    Ok(labels.join("."))
}

fn canonical_host_label(label: &str) -> Result<String, UrlError> {
    let lower = label.to_lowercase();
    if lower.is_empty() || lower.len() > 255 || lower.starts_with('-') || lower.ends_with('-') {
        return Err(UrlError::Malformed("host label is invalid".into()));
    }
    if lower.is_ascii() {
        if !lower
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Err(UrlError::Malformed(
                "host label contains invalid characters".into(),
            ));
        }
        return Ok(lower);
    }
    let codepoints = lower.chars().collect::<Vec<_>>();
    Ok(format!("xn--{}", punycode(&codepoints)?))
}

fn canonical_port(port: Option<&str>, scheme: &str) -> Result<Option<u16>, UrlError> {
    let Some(port) = port else {
        return Ok(None);
    };
    let port = port
        .parse::<u16>()
        .map_err(|_| UrlError::Malformed("port is outside the valid range".into()))?;
    if (scheme == "http" && port == 80) || (scheme == "https" && port == 443) {
        Ok(None)
    } else {
        Ok(Some(port))
    }
}

fn format_host(host: &str, bracketed: bool, port: u16) -> String {
    if bracketed || host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn format_host_without_port(host: &str, bracketed: bool) -> String {
    if bracketed || host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

fn reject_raw_url_whitespace(value: &str) -> Result<(), UrlError> {
    if value
        .chars()
        .any(|character| character.is_ascii_whitespace() || character == '\\')
    {
        Err(UrlError::Malformed(
            "URL contains unescaped whitespace or separator".into(),
        ))
    } else {
        Ok(())
    }
}

fn punycode(codepoints: &[char]) -> Result<String, UrlError> {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;
    const INITIAL_BIAS: u32 = 72;
    const INITIAL_N: u32 = 128;

    let mut output = String::new();
    let mut basic_count = 0_u32;
    for &character in codepoints {
        if character.is_ascii() {
            if !character.is_ascii_alphanumeric() && character != '-' {
                return Err(UrlError::Malformed(
                    "IDN label contains an invalid character".into(),
                ));
            }
            output.push(character);
            basic_count += 1;
        }
    }
    let mut handled = basic_count;
    if basic_count > 0 {
        output.push('-');
    }
    let mut n = INITIAL_N;
    let mut delta = 0_u32;
    let mut bias = INITIAL_BIAS;
    while handled < u32::try_from(codepoints.len()).unwrap_or(u32::MAX) {
        let next = codepoints
            .iter()
            .map(|character| *character as u32)
            .filter(|&value| value >= n)
            .min()
            .ok_or_else(|| UrlError::Malformed("IDN label cannot be encoded".into()))?;
        let increment = next
            .checked_sub(n)
            .and_then(|value| value.checked_mul(handled + 1))
            .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
        delta = delta
            .checked_add(increment)
            .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
        n = next;
        for &character in codepoints {
            let value = character as u32;
            if value < n {
                delta = delta
                    .checked_add(1)
                    .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
            } else if value == n {
                let mut quotient = delta;
                let mut k = BASE;
                loop {
                    let threshold = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if quotient < threshold {
                        break;
                    }
                    let digit = threshold + (quotient - threshold) % (BASE - threshold);
                    output.push(punycode_digit(digit));
                    quotient = (quotient - threshold) / (BASE - threshold);
                    k = k
                        .checked_add(BASE)
                        .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
                }
                output.push(punycode_digit(quotient));
                bias = punycode_adapt(delta, handled + 1, handled == basic_count);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta
            .checked_add(1)
            .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
        n = n
            .checked_add(1)
            .ok_or_else(|| UrlError::Malformed("IDN label is too complex".into()))?;
    }
    Ok(output)
}

fn punycode_digit(value: u32) -> char {
    match value {
        0..=25 => char::from(b'a' + u8::try_from(value).unwrap_or(0)),
        26..=35 => char::from(b'0' + u8::try_from(value - 26).unwrap_or(0)),
        _ => '?',
    }
}

fn punycode_adapt(mut delta: u32, points: u32, first: bool) -> u32 {
    const DAMP: u32 = 700;
    const SKEW: u32 = 38;
    delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / points;
    let mut k = 0_u32;
    while delta > ((36 - 1) * 26) / 2 {
        delta /= 36 - 1;
        k += 36;
    }
    k + (((36 - 1 + 1) * delta) / (delta + SKEW))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_authority_without_touching_escaped_components() {
        assert_eq!(
            canonicalize("HTTPS://Example.TEST.:443/a%2Fb?x=%2F#frag%23"),
            Ok("https://example.test/a%2Fb?x=%2F#frag%23".into())
        );
        assert_eq!(
            canonicalize("https://[2001:DB8::1]:443/a"),
            Ok("https://[2001:db8::1]/a".into())
        );
        assert_eq!(
            canonicalize("https://example.test:8443/?q=a%26b"),
            Ok("https://example.test:8443/?q=a%26b".into())
        );
    }

    #[test]
    fn canonicalizes_idn_and_file_urls() {
        assert_eq!(
            canonicalize("https://例え.テスト./path"),
            Ok("https://xn--r8jz45g.xn--zckzah/path".into())
        );
        assert_eq!(
            canonicalize("FILE:///tmp/a%20b#frag"),
            Ok("file:///tmp/a%20b#frag".into())
        );
        assert_eq!(canonicalize("about:blank"), Ok("about:blank".into()));
    }

    #[test]
    fn reserves_internal_document_scheme_for_browser_owned_surfaces() {
        assert_eq!(
            canonicalize("rb://settings"),
            Err(UrlError::UnsupportedScheme("rb".into()))
        );
    }

    #[test]
    fn rejects_ambiguous_authorities_and_raw_separators() {
        assert!(canonicalize("https://example.test:bad/").is_err());
        assert!(canonicalize("https://[::1/").is_err());
        assert!(canonicalize("https://example.test/a b").is_err());
        assert!(canonicalize("https://example.test/a\\b").is_err());
    }

    #[test]
    fn origins_drop_userinfo_and_non_origin_components() {
        assert_eq!(
            canonical_origin("HTTPS://user:pass@Example.test.:443/path?q=1#f"),
            Some("https://example.test".into())
        );
        assert_eq!(canonical_origin("file:///tmp/a"), None);
        assert_eq!(canonical_origin("about:blank"), None);
    }
}
