use super::{
    BTreeMap, CURRENT_SCHEMA_VERSION, Config, Deserialize, PermissionDecision, Serialize, SiteRule,
    default_context_behavior, default_context_target,
};

/// A user-maintained declarative profile definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDefinition {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub overrides: BTreeMap<String, toml::Value>,
}

/// The separate user-maintained `profiles.toml` document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilesConfig {
    pub schema_version: u64,
    #[serde(default)]
    pub profiles: Vec<ProfileDefinition>,
}

impl Default for ProfilesConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: Vec::new(),
        }
    }
}

impl ProfilesConfig {
    /// Returns the explicitly selected default profile, if one exists.
    #[must_use]
    pub fn default_profile(&self) -> Option<&ProfileDefinition> {
        self.profiles.iter().find(|profile| profile.default)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionRuleConfig {
    pub id: String,
    pub profile: String,
    pub origin: String,
    pub permission: String,
    pub decision: PermissionDecision,
}

/// Returns the user-defined site rules that match an HTTP(S) URL.
///
/// Matching is deliberately a small glob language: `*` matches any sequence
/// of characters, and all other characters match literally. Rules are
/// returned in the precedence order required by CONFIG-002: ascending
/// priority followed by source order.
#[must_use]
pub fn matching_site_rules<'a>(config: &'a Config, url: &str) -> Vec<&'a SiteRule> {
    if !is_http_url(url) {
        return Vec::new();
    }
    let mut rules = config
        .site_rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| site_pattern_matches(&rule.pattern, url))
        .collect::<Vec<_>>();
    rules.sort_by_key(|(index, rule)| (rule.priority, *index));
    rules.into_iter().map(|(_, rule)| rule).collect()
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SiteScheme {
    Http,
    Https,
    Any,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SitePattern {
    scheme: SiteScheme,
    host: String,
    port: Option<u16>,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SiteTarget {
    scheme: SiteScheme,
    host: String,
    port: u16,
    path: String,
}

pub(super) fn parse_site_pattern(pattern: &str) -> Result<SitePattern, String> {
    if pattern.is_empty()
        || pattern.len() > 512
        || pattern.chars().any(char::is_control)
        || pattern.chars().any(char::is_whitespace)
    {
        return Err("site pattern is empty, too long, or contains unsafe characters".into());
    }
    let (scheme, remainder) = pattern
        .split_once("://")
        .ok_or_else(|| "site pattern requires a scheme and authority".to_owned())?;
    let scheme = match scheme {
        "http" => SiteScheme::Http,
        "https" => SiteScheme::Https,
        "*" => SiteScheme::Any,
        _ => return Err("site pattern scheme must be http, https, or *".into()),
    };
    let authority_end = remainder.find('/').unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let path = if authority_end == remainder.len() {
        "/".to_owned()
    } else {
        remainder[authority_end..].to_owned()
    };
    if path.contains('?') || path.contains('#') || !path.starts_with('/') {
        return Err("site pattern path must be an HTTP path without query or fragment".into());
    }
    let (host, port) = parse_site_authority(authority, true)?;
    Ok(SitePattern {
        scheme,
        host,
        port,
        path,
    })
}

fn parse_site_target(url: &str) -> Option<SiteTarget> {
    let (scheme, remainder) = url.split_once("://")?;
    let scheme = match scheme {
        "http" => SiteScheme::Http,
        "https" => SiteScheme::Https,
        _ => return None,
    };
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let (host, explicit_port) = parse_site_authority(&remainder[..authority_end], false).ok()?;
    let port = explicit_port.unwrap_or(match scheme {
        SiteScheme::Http => 80,
        SiteScheme::Https => 443,
        SiteScheme::Any => return None,
    });
    let path_end = remainder[authority_end..]
        .find(['?', '#'])
        .map_or(remainder.len(), |index| authority_end + index);
    let path = if authority_end == remainder.len() || remainder.as_bytes()[authority_end] != b'/' {
        "/".to_owned()
    } else {
        remainder[authority_end..path_end].to_owned()
    };
    Some(SiteTarget {
        scheme,
        host,
        port,
        path,
    })
}

fn parse_site_authority(
    authority: &str,
    allow_wildcards: bool,
) -> Result<(String, Option<u16>), String> {
    if authority.is_empty() || authority.contains('@') {
        return Err("site pattern authority is empty or contains userinfo".into());
    }
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| "bracketed IPv6 authority is incomplete".to_owned())?;
        let host = &authority[1..end];
        let suffix = &authority[end + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            suffix
                .strip_prefix(':')
                .ok_or_else(|| "IPv6 authority has an invalid suffix".to_owned())?
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| "site authority port must be 1..65535".to_owned())
                .map(Some)?
        };
        (host.to_owned(), port)
    } else {
        if authority.matches(':').count() > 1 {
            return Err("IPv6 hosts require bracket syntax".into());
        }
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| {
                (host, port.parse::<u16>().ok().filter(|port| *port != 0))
            });
        if authority.contains(':') && port.is_none() {
            return Err("site authority port must be 1..65535".into());
        }
        (host.to_owned(), port)
    };
    let host = host.to_ascii_lowercase();
    if !valid_site_host(&host, allow_wildcards) {
        return Err("site authority host is invalid".into());
    }
    Ok((host, port))
}

fn valid_site_host(host: &str, allow_wildcards: bool) -> bool {
    if host == "*" {
        return allow_wildcards;
    }
    if host.contains(':') {
        return host.len() <= 128
            && host
                .chars()
                .all(|character| character.is_ascii_hexdigit() || matches!(character, ':' | '.'));
    }
    let host = if let Some(suffix) = host.strip_prefix("*.") {
        if !allow_wildcards || suffix.is_empty() || suffix.contains('*') {
            return false;
        }
        suffix
    } else {
        if host.contains('*') {
            return false;
        }
        host
    };
    host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn site_pattern_matches(pattern: &str, url: &str) -> bool {
    let Ok(pattern) = parse_site_pattern(pattern) else {
        return false;
    };
    let Some(target) = parse_site_target(url) else {
        return false;
    };
    if pattern.scheme != SiteScheme::Any && pattern.scheme != target.scheme {
        return false;
    }
    let host_matches = if pattern.host == "*" {
        true
    } else if let Some(suffix) = pattern.host.strip_prefix("*.") {
        target.host == suffix || target.host.ends_with(&format!(".{suffix}"))
    } else {
        target.host == pattern.host
    };
    if !host_matches {
        return false;
    }
    let expected_port = pattern.port.unwrap_or(match target.scheme {
        SiteScheme::Http => 80,
        SiteScheme::Https => 443,
        SiteScheme::Any => return false,
    });
    target.port == expected_port && glob_matches(&pattern.path, &target.path)
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pattern_index, mut value_index) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while value_index < value.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == value[value_index] {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star = Some(pattern_index);
            pattern_index += 1;
            retry = value_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry += 1;
            value_index = retry;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

/// A user-maintained durable browsing context definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDefinition {
    pub name: String,
    pub label: String,
    pub profile: String,
    #[serde(default)]
    pub sessions: Vec<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default = "default_context_target")]
    pub default_target: String,
}

/// A context route from an explicit browser-controlled entry point.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRoute {
    pub id: String,
    pub pattern: String,
    pub context: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_context_behavior")]
    pub behavior: String,
    #[serde(default)]
    pub entry_points: Vec<String>,
}

/// The separate user-maintained `contexts.toml` document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextsConfig {
    pub schema_version: u64,
    #[serde(default)]
    pub contexts: Vec<ContextDefinition>,
    #[serde(default)]
    pub routes: Vec<ContextRoute>,
}

/// Returns context routes that match one browser-controlled entry point.
///
/// Routes use the same deliberately narrow HTTP(S) match-pattern grammar as
/// CONFIG-006. Results are ordered by descending priority, then descending
/// pattern specificity, then authored order. The caller can show all results
/// in a preview while using the first result as the deterministic candidate.
#[must_use]
pub fn matching_context_routes<'a>(
    config: &'a ContextsConfig,
    url: &str,
    entry_point: &str,
) -> Vec<&'a ContextRoute> {
    if entry_point.is_empty() {
        return Vec::new();
    }
    let Some(url_parts) = parse_http_target(url) else {
        return Vec::new();
    };
    let mut routes = config
        .routes
        .iter()
        .enumerate()
        .filter_map(|(index, route)| {
            if !route.entry_points.iter().any(|entry| entry == entry_point) {
                return None;
            }
            let pattern = parse_context_pattern(&route.pattern)?;
            let specificity = pattern.specificity();
            pattern
                .matches(&url_parts)
                .then_some((route.priority, specificity, index, route))
        })
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    routes.into_iter().map(|(_, _, _, route)| route).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpTarget {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContextPattern {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

impl ContextPattern {
    fn matches(&self, target: &HttpTarget) -> bool {
        if self.scheme != "*" && self.scheme != target.scheme {
            return false;
        }
        let host_matches = if self.host == "*" {
            true
        } else if let Some(suffix) = self.host.strip_prefix("*.") {
            target.host == suffix || target.host.ends_with(&format!(".{suffix}"))
        } else {
            target.host == self.host
        };
        if !host_matches {
            return false;
        }
        let target_port = target.port.or_else(|| default_http_port(&target.scheme));
        let pattern_port = self.port.or_else(|| {
            default_http_port(if self.scheme == "*" {
                &target.scheme
            } else {
                &self.scheme
            })
        });
        pattern_port == target_port && glob_matches(&self.path, &target.path)
    }

    fn specificity(&self) -> u16 {
        let scheme = u16::from(self.scheme != "*") * 100;
        let host = if self.host == "*" {
            0
        } else if self.host.starts_with("*.") {
            50
        } else {
            100
        };
        let port = u16::from(self.port.is_some()) * 10;
        let path: u16 = self
            .path
            .bytes()
            .filter(|byte| *byte != b'*')
            .count()
            .min(99)
            .try_into()
            .expect("specificity path length is bounded to 99");
        scheme + host + port + path
    }
}

pub(super) fn parse_context_pattern(value: &str) -> Option<ContextPattern> {
    let target = parse_http_target(value)?;
    let (scheme, authority) = value.split_once("://")?;
    let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
    let authority = &authority[..authority_end];
    if scheme != "*" && !matches!(scheme, "http" | "https")
        || value.contains(['?', '#'])
        || authority.is_empty()
        || authority.contains('@')
    {
        return None;
    }
    let host = target.host;
    if host != "*" && !host.starts_with("*.") && host.contains('*')
        || host.starts_with("*.") && host.len() <= 2
    {
        return None;
    }
    let path = value
        .split_once("://")
        .and_then(|(_, remainder)| remainder.split_once('/').map(|(_, path)| path))
        .map_or_else(|| "/*".to_owned(), |path| format!("/{path}"));
    if path.is_empty() || path.chars().any(char::is_control) {
        return None;
    }
    Some(ContextPattern {
        scheme: scheme.to_ascii_lowercase(),
        host,
        port: target.port,
        path,
    })
}

fn parse_http_target(value: &str) -> Option<HttpTarget> {
    let (scheme, remainder) = value.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "*") || scheme == "*" && remainder.is_empty() {
        return None;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let (host, port) = parse_pattern_authority(authority)?;
    let path = remainder[authority_end..]
        .split(['?', '#'])
        .next()
        .filter(|path| !path.is_empty())
        .unwrap_or("/");
    Some(HttpTarget {
        scheme,
        host,
        port,
        path: path.to_owned(),
    })
}

fn parse_pattern_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    if authority == "*" {
        return Some(("*".into(), None));
    }
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']')?;
        let suffix = &authority[close + 1..];
        let port = if let Some(value) = suffix.strip_prefix(':') {
            Some(parse_port(value)?)
        } else {
            None
        };
        if !suffix.is_empty() && !suffix.starts_with(':') {
            return None;
        }
        (&authority[1..close], port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        (host, Some(parse_port(port)?))
    } else {
        (authority, None)
    };
    if host.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    if host != "*" && host != "localhost" && !host.starts_with("*.") && host.contains('*')
        || host.starts_with("*.") && host.len() <= 2
    {
        return None;
    }
    Some((host, port))
}

fn parse_port(value: &str) -> Option<u16> {
    let port = value.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

fn default_http_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

impl Default for ContextsConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            contexts: Vec::new(),
            routes: Vec::new(),
        }
    }
}
