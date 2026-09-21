#![no_main]

use ferric_browser_config::{SiteRule, validate_site_rule};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(rule) = toml::from_str::<SiteRule>(input) else {
        return;
    };
    let _ = validate_site_rule(&rule);
});
