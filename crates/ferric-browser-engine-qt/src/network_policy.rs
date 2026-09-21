use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
};

use adblock::{
    Engine,
    lists::{FilterSet, ParseOptions},
    request::Request,
};
use ferric_browser_storage::StorageRoots;
use serde::Serialize;
use serde_json::Value;

const MAX_LIST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TOTAL_LIST_BYTES: u64 = 128 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 8 * 1024;
const MAX_METADATA_FIELD_BYTES: usize = 4096;
const MAX_HOST_RULES: usize = 100_000;

#[derive(Debug, Default)]
pub struct PolicySnapshot {
    pub blocked_hosts: Vec<String>,
    pub exception_hosts: Vec<String>,
    pub blocked_rule_lists: BTreeMap<String, String>,
    pub exception_rule_lists: BTreeMap<String, String>,
    pub cosmetic_rules: Vec<CosmeticRule>,
    pub cosmetic_exceptions: Vec<CosmeticRule>,
    pub bypass_sites: Vec<String>,
    pub security_deny_hosts: Vec<String>,
    pub loaded_lists: Vec<String>,
    pub list_metadata: Vec<ListMetadata>,
    pub skipped_lists: Vec<String>,
    pub compile_failures: Vec<String>,
    pub adblock_engine: Option<AdblockEngineHandle>,
    pub adblock_source_ids: Vec<String>,
}

pub struct AdblockEngineHandle(Box<AdblockEngine>);

impl std::fmt::Debug for AdblockEngineHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdblockEngineHandle")
            .finish_non_exhaustive()
    }
}

pub(crate) struct AdblockEngine {
    engine: Engine,
}

impl AdblockEngineHandle {
    fn compile(lists: &[(String, Vec<u8>)]) -> Result<Self, String> {
        let mut filter_set = FilterSet::new(true);
        for (_, bytes) in lists {
            let text = std::str::from_utf8(bytes).map_err(|_| "list is not UTF-8".to_owned())?;
            filter_set.add_filter_list(text.to_owned(), ParseOptions::default());
        }
        Ok(Self(Box::new(AdblockEngine {
            engine: Engine::new_with_filter_set(filter_set),
        })))
    }

    pub(crate) fn into_raw(self) -> *mut AdblockEngine {
        Box::into_raw(self.0)
    }

    fn has_network_rules(&self) -> bool {
        self.0
            .engine
            .get_debug_info()
            .source_info
            .iter()
            .any(|source| source.network_filter_count > 0)
    }

    #[cfg(test)]
    fn blocks(&self, url: &str, source_url: &str, request_type: &str) -> bool {
        let request = Request::new(url, source_url, request_type, "GET")
            .expect("test request must have a valid URL");
        self.0.engine.check_network_request(&request).should_block()
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferric_browser_adblock_release(handle: *mut AdblockEngine) {
    if !handle.is_null() {
        // SAFETY: callers may release only handles returned by this module.
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ferric_browser_adblock_check(
    handle: *const AdblockEngine,
    url: *const u8,
    url_len: usize,
    source_url: *const u8,
    source_url_len: usize,
    request_type: *const u8,
    request_type_len: usize,
    method: *const u8,
    method_len: usize,
    source_index: *mut u32,
    matched_rule: *mut u8,
    matched_rule_capacity: usize,
    matched_rule_length: *mut usize,
) -> bool {
    if handle.is_null()
        || url.is_null()
        || source_url.is_null()
        || request_type.is_null()
        || method.is_null()
    {
        return false;
    }
    // SAFETY: all pointers are checked for non-null and lengths come from the
    // same immutable Qt byte arrays for the duration of this call.
    let (url, source_url, request_type, method) = unsafe {
        (
            std::slice::from_raw_parts(url, url_len),
            std::slice::from_raw_parts(source_url, source_url_len),
            std::slice::from_raw_parts(request_type, request_type_len),
            std::slice::from_raw_parts(method, method_len),
        )
    };
    let Ok(url) = std::str::from_utf8(url) else {
        return false;
    };
    let Ok(source_url) = std::str::from_utf8(source_url) else {
        return false;
    };
    let Ok(request_type) = std::str::from_utf8(request_type) else {
        return false;
    };
    let Ok(method) = std::str::from_utf8(method) else {
        return false;
    };
    let Ok(request) = Request::new(url, source_url, request_type, method) else {
        return false;
    };
    // SAFETY: the handle is an engine pointer created by `AdblockEngineHandle`.
    let result = unsafe { (*handle).engine.check_network_request(&request) };
    if !source_index.is_null() {
        // SAFETY: the caller provides a valid writable u32 when non-null.
        let index = unsafe { &mut *source_index };
        *index = result
            .filter
            .as_ref()
            .and_then(|filter| filter.source_location.as_ref())
            .map_or(u32::MAX, |location| location.source_index);
    }
    if !matched_rule_length.is_null() {
        let raw_line = result
            .filter
            .as_ref()
            .and_then(|filter| filter.raw_line.as_deref())
            .unwrap_or_default()
            .as_bytes();
        let copy_length = raw_line.len().min(matched_rule_capacity);
        if copy_length > 0 && !matched_rule.is_null() {
            // SAFETY: the caller provides a writable output buffer of the
            // declared capacity when non-null.
            unsafe {
                std::ptr::copy_nonoverlapping(raw_line.as_ptr(), matched_rule, copy_length);
            }
        }
        // SAFETY: the caller provides a valid writable length when non-null.
        unsafe {
            *matched_rule_length = copy_length;
        }
    }
    result.should_block()
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ListMetadata {
    pub id: String,
    pub bytes: u64,
    pub last_modified: Option<String>,
    pub etag_present: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CosmeticRule {
    #[serde(rename = "host")]
    pub host_pattern: String,
    #[serde(rename = "selector")]
    pub selector: String,
}

/// Compiles the bounded cached ABP lists off the request callback. The
/// immutable engine handle is transferred to Qt for synchronous URL matching;
/// the host vectors remain as a conservative fallback and for diagnostics.
#[must_use]
pub fn load(roots: Option<&StorageRoots>, config: &Value) -> PolicySnapshot {
    let enabled = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let network_filtering = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("network_filtering"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let cosmetic_filtering = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("cosmetic_filtering"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let security_deny_hosts = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("security_deny_hosts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(64)
        .filter(|host| valid_host_pattern(host))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !enabled || !network_filtering {
        return PolicySnapshot {
            security_deny_hosts,
            ..PolicySnapshot::default()
        };
    }

    let bypass_sites = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("bypass_sites"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(64)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let Some(roots) = roots else {
        return PolicySnapshot {
            bypass_sites,
            security_deny_hosts,
            skipped_lists: vec!["no-cache-root".into()],
            ..PolicySnapshot::default()
        };
    };
    let list_ids = config
        .get("blocking")
        .and_then(Value::as_object)
        .and_then(|blocking| blocking.get("lists"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(64)
        .map(str::to_owned)
        .collect::<Vec<_>>();

    let mut blocked = BTreeSet::new();
    let mut exceptions = BTreeSet::new();
    let mut blocked_rule_lists = BTreeMap::new();
    let mut exception_rule_lists = BTreeMap::new();
    let mut cosmetic_rules = BTreeSet::new();
    let mut cosmetic_exceptions = BTreeSet::new();
    let mut loaded_lists = Vec::new();
    let mut list_metadata = Vec::new();
    let mut skipped_lists = Vec::new();
    let mut compile_failures = Vec::new();
    let mut engine_lists = Vec::new();
    let mut engine_source_ids = Vec::new();
    let mut total_bytes: u64 = 0;
    for list_id in list_ids {
        let Some(cache_id) = source_cache_id(&list_id) else {
            skipped_lists.push(list_id);
            continue;
        };
        let path = roots
            .cache
            .join("blocklists")
            .join(format!("{cache_id}.txt"));
        let Ok(metadata) = fs::metadata(&path) else {
            skipped_lists.push(list_id);
            continue;
        };
        if !metadata.is_file()
            || metadata.len() > MAX_LIST_BYTES
            || total_bytes.saturating_add(metadata.len()) > MAX_TOTAL_LIST_BYTES
        {
            skipped_lists.push(list_id);
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            skipped_lists.push(list_id);
            continue;
        };
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        let mut list_blocked = BTreeSet::new();
        let mut list_exceptions = BTreeSet::new();
        if parse_list(
            &bytes,
            &mut list_blocked,
            &mut list_exceptions,
            cosmetic_filtering,
            &mut cosmetic_rules,
            &mut cosmetic_exceptions,
        )
        .is_err()
        {
            skipped_lists.push(list_id);
            compile_failures.push(cache_id);
            continue;
        }
        engine_source_ids.push(cache_id.clone());
        engine_lists.push((cache_id.clone(), bytes.clone()));
        for host in list_blocked {
            blocked_rule_lists
                .entry(host.clone())
                .or_insert_with(|| cache_id.clone());
            blocked.insert(host);
        }
        for host in list_exceptions {
            exception_rule_lists
                .entry(host.clone())
                .or_insert_with(|| cache_id.clone());
            exceptions.insert(host);
        }
        list_metadata.push(read_list_metadata(roots, &cache_id, bytes.len() as u64));
        loaded_lists.push(cache_id);
        if blocked.len().saturating_add(exceptions.len()) >= MAX_HOST_RULES {
            break;
        }
    }
    let adblock_engine = if engine_lists.is_empty() {
        None
    } else {
        match AdblockEngineHandle::compile(&engine_lists) {
            Ok(engine) => Some(engine),
            Err(error) => {
                compile_failures.push(format!("adblock-engine: {error}"));
                None
            }
        }
    };
    PolicySnapshot {
        blocked_hosts: blocked.into_iter().collect(),
        exception_hosts: exceptions.into_iter().collect(),
        blocked_rule_lists,
        exception_rule_lists,
        cosmetic_rules: cosmetic_rules
            .into_iter()
            .map(|(host_pattern, selector)| CosmeticRule {
                host_pattern,
                selector,
            })
            .collect(),
        cosmetic_exceptions: cosmetic_exceptions
            .into_iter()
            .map(|(host_pattern, selector)| CosmeticRule {
                host_pattern,
                selector,
            })
            .collect(),
        bypass_sites,
        security_deny_hosts,
        loaded_lists,
        list_metadata,
        skipped_lists,
        compile_failures,
        adblock_engine,
        adblock_source_ids: engine_source_ids,
    }
}

fn read_list_metadata(roots: &StorageRoots, id: &str, bytes: u64) -> ListMetadata {
    let path = roots.cache.join("blocklists").join(format!("{id}.meta"));
    let mut metadata = ListMetadata {
        id: id.to_owned(),
        bytes,
        ..ListMetadata::default()
    };
    let Ok(file_metadata) = fs::metadata(&path) else {
        return metadata;
    };
    if !file_metadata.is_file() || file_metadata.len() > MAX_METADATA_BYTES {
        return metadata;
    }
    let Ok(bytes) = fs::read(&path) else {
        return metadata;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return metadata;
    };
    let mut lines = text.lines();
    metadata.etag_present = bounded_metadata_field(lines.next()).is_some();
    metadata.last_modified = bounded_metadata_field(lines.next());
    metadata
}

fn bounded_metadata_field(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty()
        || value.len() > MAX_METADATA_FIELD_BYTES
        || !value.is_ascii()
        || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return None;
    }
    Some(value.to_owned())
}

pub(crate) fn valid_list_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn source_cache_id(source: &str) -> Option<String> {
    if valid_list_id(source) {
        return Some(source.to_owned());
    }
    if !valid_https_source(source) {
        return None;
    }
    let digest = sha256_hex(source.as_bytes());
    Some(format!("url-{}", &digest[..60]))
}

fn valid_https_source(source: &str) -> bool {
    if !(10..=2048).contains(&source.len())
        || !source.is_ascii()
        || !source.bytes().all(|byte| byte.is_ascii_graphic())
        || source.chars().any(char::is_control)
        || !source.starts_with("https://")
        || source.contains('@')
        || source.contains('#')
    {
        return false;
    }
    let authority = source[8..].split(['/', '?']).next().unwrap_or_default();
    let host = authority.split(':').next().unwrap_or_default();
    valid_host_pattern(host)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn sha256_hex(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let padded_len = (input.len() + 9).div_ceil(64) * 64;
    let mut padded = vec![0_u8; padded_len];
    padded[..input.len()].copy_from_slice(input);
    padded[input.len()] = 0x80;
    padded[padded_len - 8..].copy_from_slice(&bit_len.to_be_bytes());

    let mut state = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, bytes) in block.chunks_exact(4).take(16).enumerate() {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..64 {
            let value = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let next = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(value)
                .wrapping_add(words[index - 7])
                .wrapping_add(next);
        }
        let mut working = state;
        for index in 0..64 {
            let choice = (working[4] & working[5]) ^ (!working[4] & working[6]);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let sigma_one = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let sigma_zero = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let temp_one = working[7]
                .wrapping_add(sigma_one)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(words[index]);
            let temp_two = sigma_zero.wrapping_add(majority);
            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3].wrapping_add(temp_one);
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp_one.wrapping_add(temp_two);
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(working[index]);
        }
    }
    let mut output = String::with_capacity(64);
    for word in state {
        write!(output, "{word:08x}").expect("writing to a string cannot fail");
    }
    output
}

pub(crate) fn validate_download(bytes: &[u8]) -> Result<(), ()> {
    let max_list_bytes = usize::try_from(MAX_LIST_BYTES).unwrap_or(usize::MAX);
    if bytes.is_empty() || bytes.len() > max_list_bytes {
        return Err(());
    }
    let mut blocked = BTreeSet::new();
    let mut exceptions = BTreeSet::new();
    let mut cosmetic_rules = BTreeSet::new();
    let mut cosmetic_exceptions = BTreeSet::new();
    parse_list(
        bytes,
        &mut blocked,
        &mut exceptions,
        true,
        &mut cosmetic_rules,
        &mut cosmetic_exceptions,
    )?;
    if blocked.is_empty()
        && exceptions.is_empty()
        && cosmetic_rules.is_empty()
        && cosmetic_exceptions.is_empty()
    {
        let engine =
            AdblockEngineHandle::compile(&[("download".into(), bytes.to_vec())]).map_err(|_| ())?;
        if !engine.has_network_rules() {
            return Err(());
        }
    }
    Ok(())
}

fn parse_list(
    bytes: &[u8],
    blocked: &mut BTreeSet<String>,
    exceptions: &mut BTreeSet<String>,
    cosmetic_filtering: bool,
    cosmetic_rules: &mut BTreeSet<(String, String)>,
    cosmetic_exceptions: &mut BTreeSet<(String, String)>,
) -> Result<(), ()> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    for raw_line in text.lines().take(1_000_000) {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
            continue;
        }
        if cosmetic_filtering && (line.contains("##") || line.contains("#@#")) {
            parse_cosmetic_rule(line, cosmetic_rules, cosmetic_exceptions);
            continue;
        }
        if line.contains("##") || line.contains("#@#") || line.contains("#$#") {
            continue;
        }
        let (exception, rule) = line
            .strip_prefix("@@")
            .map_or((false, line), |rule| (true, rule));
        let Some(host_rule) = rule.strip_prefix("||") else {
            continue;
        };
        let host = host_rule.split(['^', '$', '/']).next().unwrap_or_default();
        if !valid_host_pattern(host) {
            continue;
        }
        if blocked.len().saturating_add(exceptions.len()) >= MAX_HOST_RULES {
            break;
        }
        if exception {
            exceptions.insert(host.to_ascii_lowercase());
        } else {
            blocked.insert(host.to_ascii_lowercase());
        }
    }
    Ok(())
}

fn parse_cosmetic_rule(
    line: &str,
    cosmetic_rules: &mut BTreeSet<(String, String)>,
    cosmetic_exceptions: &mut BTreeSet<(String, String)>,
) {
    let (exception, marker, separator_len) = if let Some(index) = line.find("#@#") {
        (true, index, 3)
    } else if let Some(index) = line.find("##") {
        (false, index, 2)
    } else {
        return;
    };
    let domains = &line[..marker];
    let selector = line[marker + separator_len..].trim();
    if domains.is_empty() || !valid_cosmetic_selector(selector) {
        return;
    }
    for domain in domains.split(',').map(str::trim) {
        if !valid_host_pattern(domain) {
            continue;
        }
        let item = (domain.to_ascii_lowercase(), selector.to_owned());
        if exception {
            cosmetic_exceptions.insert(item);
        } else {
            cosmetic_rules.insert(item);
        }
    }
}

fn valid_cosmetic_selector(selector: &str) -> bool {
    let lowered = selector.to_ascii_lowercase();
    if selector.is_empty()
        || selector.len() > 256
        || lowered.contains("url")
        || lowered.contains("expression")
    {
        return false;
    }
    selector.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b' ' | b'.'
                    | b'#'
                    | b'['
                    | b']'
                    | b'='
                    | b'"'
                    | b'\''
                    | b':'
                    | b'>'
                    | b'+'
                    | b'~'
                    | b'*'
                    | b'-'
                    | b'_'
                    | b'('
                    | b')'
                    | b'^'
                    | b'$'
                    | b'|'
            )
    })
}

fn valid_host_pattern(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 || host.contains('*') && !host.starts_with("*.") {
        return false;
    }
    let host = host.strip_prefix("*.").unwrap_or(host);
    !host.is_empty()
        && !host.contains("..")
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::{
        load, parse_list, sha256_hex, source_cache_id, valid_cosmetic_selector, valid_host_pattern,
        valid_https_source, validate_download,
    };
    use ferric_browser_storage::{RootSpec, StorageRoots};
    use serde_json::json;
    use std::{
        collections::BTreeSet,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_host_rules_and_exceptions_but_ignores_cosmetic_rules() {
        let input = b"! comment\n||ads.example^\n@@||ads.example^\nexample##.ad\n||*.tracking.example^\n||ads.example/path\n";
        let mut blocked = BTreeSet::new();
        let mut exceptions = BTreeSet::new();
        parse_list(
            input,
            &mut blocked,
            &mut exceptions,
            false,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
        .expect("valid UTF-8 list");
        assert!(blocked.contains("ads.example"));
        assert!(blocked.contains("*.tracking.example"));
        assert!(exceptions.contains("ads.example"));
        assert_eq!(blocked.len(), 2);
    }

    #[test]
    fn maintained_abp_engine_matches_url_type_and_exception_options() {
        let engine = super::AdblockEngineHandle::compile(&[(
            "easylist".into(),
            b"||ads.example^$script\n@@||ads.example/allowed.js$script,domain=publisher.example\n||tracker.example^$third-party\n".to_vec(),
        )])
        .expect("ABP list compiles");
        assert!(engine.blocks(
            "https://ads.example/ad.js",
            "https://publisher.example/story",
            "script"
        ));
        assert!(!engine.blocks(
            "https://ads.example/allowed.js",
            "https://publisher.example/story",
            "script"
        ));
        assert!(!engine.blocks(
            "https://tracker.example/pixel",
            "https://tracker.example/story",
            "image"
        ));
    }

    #[test]
    fn maintained_abp_engine_matches_loopback_fixture_paths() {
        let engine = super::AdblockEngineHandle::compile(&[(
            "fixture".into(),
            b"||127.0.0.1^$image\n@@||127.0.0.1^$script\n".to_vec(),
        )])
        .expect("fixture list compiles");
        assert!(engine.blocks(
            "http://127.0.0.1:18774/probe",
            "http://127.0.0.1:41000/blocking",
            "image"
        ));
        assert!(!engine.blocks(
            "http://127.0.0.1:18774/probe",
            "http://127.0.0.1:41000/blocking",
            "script"
        ));
    }

    #[test]
    fn adblock_ffi_boundary_returns_match_provenance_and_releases() {
        let engine = super::AdblockEngineHandle::compile(&[(
            "easylist".into(),
            b"||ads.example^\n".to_vec(),
        )])
        .expect("ABP list compiles");
        let handle = engine.into_raw();
        let url = b"https://ads.example/ad.js";
        let source = b"https://publisher.example/story";
        let request_type = b"script";
        let method = b"GET";
        let mut source_index = u32::MAX;
        let mut matched_rule = [0_u8; 64];
        let mut matched_rule_length = 0_usize;
        // SAFETY: these byte slices and the opaque handle remain valid for the call.
        let blocked = unsafe {
            super::ferric_browser_adblock_check(
                handle,
                url.as_ptr(),
                url.len(),
                source.as_ptr(),
                source.len(),
                request_type.as_ptr(),
                request_type.len(),
                method.as_ptr(),
                method.len(),
                &raw mut source_index,
                matched_rule.as_mut_ptr(),
                matched_rule.len(),
                &raw mut matched_rule_length,
            )
        };
        assert!(blocked);
        assert_eq!(source_index, 0);
        assert_eq!(&matched_rule[..matched_rule_length], b"||ads.example^");
        // SAFETY: ownership was transferred to the raw handle exactly once.
        unsafe { super::ferric_browser_adblock_release(handle) };
    }

    #[test]
    fn host_rules_are_conservative_and_bounded() {
        assert!(valid_host_pattern("example.test"));
        assert!(valid_host_pattern("*.example.test"));
        assert!(!valid_host_pattern("example..test"));
        assert!(!valid_host_pattern("https://example.test"));
        assert!(!valid_host_pattern("*ads.example"));
        assert!(validate_download(b"! comments only\n").is_err());
        assert!(validate_download(b"||valid.example^\n").is_ok());
        assert!(validate_download(b"/ads\\.js/\n").is_ok());
        assert!(validate_download(b"example.test##.ad\n").is_ok());
    }

    #[test]
    fn parses_bounded_host_scoped_cosmetic_rules_and_exceptions() {
        let input = b"example.test##.ad\nexample.test##[data-sponsored=\"true\"]\nexample.test#@#.ad\n##.generic\nexample.test##div { display:none }\n";
        let mut blocked = BTreeSet::new();
        let mut exceptions = BTreeSet::new();
        let mut cosmetics = BTreeSet::new();
        let mut cosmetic_exceptions = BTreeSet::new();
        parse_list(
            input,
            &mut blocked,
            &mut exceptions,
            true,
            &mut cosmetics,
            &mut cosmetic_exceptions,
        )
        .expect("valid UTF-8 list");
        assert!(cosmetics.contains(&("example.test".into(), ".ad".into())));
        assert!(cosmetics.contains(&("example.test".into(), "[data-sponsored=\"true\"]".into())));
        assert!(cosmetic_exceptions.contains(&("example.test".into(), ".ad".into())));
        assert!(!cosmetics.iter().any(|(_, selector)| selector == ".generic"));
        assert!(!valid_cosmetic_selector(".ad { display:none }"));
        assert!(!valid_cosmetic_selector("[style*=url]"));
        assert!(!valid_cosmetic_selector("[style*=URL]"));
    }

    #[test]
    fn explicit_https_sources_use_stable_opaque_cache_ids() {
        let source = "https://filters.example/list.txt";
        let cache_id = source_cache_id(source).expect("HTTPS source is valid");
        assert_eq!(cache_id.len(), 64);
        assert!(cache_id.starts_with("url-"));
        assert_eq!(source_cache_id(source), Some(cache_id.clone()));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(valid_https_source(source));
        assert!(!valid_https_source("http://filters.example/list.txt"));
        assert!(!valid_https_source("https://user@filters.example/list.txt"));
        assert!(!valid_https_source(
            "https://filters.example/list.txt#fragment"
        ));
        assert!(!valid_https_source("https://"));
    }

    #[test]
    fn loads_named_cached_list_with_explicit_exception() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("ferric-browser-network-policy-{suffix}"));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("safe test root");
        fs::create_dir_all(roots.cache.join("blocklists")).expect("cache directory");
        fs::write(
            roots.cache.join("blocklists").join("fixture.txt"),
            b"||ads.example^\n@@||ads.example^\nexample.test##.ad\nexample.test#@#.sponsored\n",
        )
        .expect("fixture list");
        fs::write(
            roots.cache.join("blocklists").join("fixture.meta"),
            b"W/\"fixture-etag\"\nWed, 21 Oct 2015 07:28:00 GMT\n",
        )
        .expect("fixture metadata");

        let snapshot = load(
            Some(&roots),
            &json!({
                "blocking": {
                    "enabled": true,
                    "network_filtering": true,
                    "cosmetic_filtering": true,
                    "lists": ["fixture"],
                    "bypass_sites": ["example.test"],
                    "security_deny_hosts": ["blocked.example"]
                }
            }),
        );

        assert_eq!(snapshot.loaded_lists, ["fixture"]);
        assert_eq!(snapshot.blocked_hosts, ["ads.example"]);
        assert_eq!(snapshot.exception_hosts, ["ads.example"]);
        assert_eq!(snapshot.blocked_rule_lists["ads.example"], "fixture");
        assert_eq!(snapshot.exception_rule_lists["ads.example"], "fixture");
        assert_eq!(snapshot.cosmetic_rules[0].host_pattern, "example.test");
        assert_eq!(snapshot.cosmetic_rules[0].selector, ".ad");
        assert_eq!(snapshot.cosmetic_exceptions[0].selector, ".sponsored");
        assert_eq!(snapshot.bypass_sites, ["example.test"]);
        assert_eq!(snapshot.security_deny_hosts, ["blocked.example"]);
        assert_eq!(snapshot.list_metadata.len(), 1);
        assert_eq!(snapshot.list_metadata[0].id, "fixture");
        assert_eq!(snapshot.list_metadata[0].bytes, 76);
        assert_eq!(
            snapshot.list_metadata[0].last_modified.as_deref(),
            Some("Wed, 21 Oct 2015 07:28:00 GMT")
        );
        assert!(snapshot.list_metadata[0].etag_present);
        assert!(snapshot.compile_failures.is_empty());
        fs::remove_dir_all(base).expect("remove test root");
    }

    #[test]
    fn security_deny_hosts_survive_disabled_adblocking() {
        let snapshot = load(
            None,
            &json!({
                "blocking": {
                    "enabled": false,
                    "network_filtering": false,
                    "security_deny_hosts": ["blocked.example"]
                }
            }),
        );
        assert_eq!(snapshot.security_deny_hosts, ["blocked.example"]);
        assert!(snapshot.blocked_hosts.is_empty());
    }

    #[test]
    fn malformed_validator_metadata_is_ignored_without_disabling_the_list() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("ferric-browser-network-metadata-{suffix}"));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("safe test root");
        fs::create_dir_all(roots.cache.join("blocklists")).expect("cache directory");
        fs::write(
            roots.cache.join("blocklists").join("fixture.txt"),
            b"||ads.example^\n",
        )
        .expect("fixture list");
        fs::write(
            roots.cache.join("blocklists").join("fixture.meta"),
            vec![b'x'; 8 * 1024 + 1],
        )
        .expect("oversized metadata");

        let snapshot = load(
            Some(&roots),
            &json!({
                "blocking": {
                    "enabled": true,
                    "network_filtering": true,
                    "lists": ["fixture"]
                }
            }),
        );

        assert_eq!(snapshot.loaded_lists, ["fixture"]);
        assert_eq!(snapshot.blocked_hosts, ["ads.example"]);
        assert_eq!(snapshot.list_metadata[0].last_modified, None);
        assert!(!snapshot.list_metadata[0].etag_present);
        fs::remove_dir_all(base).expect("remove test root");
    }

    #[test]
    fn invalid_utf8_is_reported_as_a_compile_failure() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("ferric-browser-network-invalid-{suffix}"));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("safe test root");
        fs::create_dir_all(roots.cache.join("blocklists")).expect("cache directory");
        fs::write(
            roots.cache.join("blocklists").join("broken.txt"),
            [0xff, 0xfe, 0xfd],
        )
        .expect("invalid fixture list");

        let snapshot = load(
            Some(&roots),
            &json!({
                "blocking": {
                    "enabled": true,
                    "network_filtering": true,
                    "lists": ["broken"]
                }
            }),
        );

        assert_eq!(snapshot.skipped_lists, ["broken"]);
        assert_eq!(snapshot.compile_failures, ["broken"]);
        assert!(snapshot.blocked_hosts.is_empty());
        fs::remove_dir_all(base).expect("remove test root");
    }
}
