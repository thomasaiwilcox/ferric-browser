#![no_main]

use ferric_browser_config::{ContextsConfig, matching_context_routes};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(config) = toml::from_str::<ContextsConfig>(input) else {
        return;
    };
    for url in [input, "https://example.test/path?query=1"] {
        for entry_point in ["startup", "new-tab", "explicit-open", ""] {
            let _ = matching_context_routes(&config, url, entry_point);
        }
    }
});
