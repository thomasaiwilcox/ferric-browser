#![no_main]

use browser_config::ActionTargetConfig;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let _ = toml::from_str::<ActionTargetConfig>(input);
});
