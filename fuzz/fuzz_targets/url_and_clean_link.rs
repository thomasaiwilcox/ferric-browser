#![no_main]

use ferric_browser_core::{ValidatedUrl, builtin_rules, clean_link};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let _ = ValidatedUrl::parse(input);
    let rules = builtin_rules();
    let _ = clean_link(input, &rules);
});
