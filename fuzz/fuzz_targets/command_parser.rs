#![no_main]

use ferric_browser_core::{CommandRegistry, ParseInput, parse_chain};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let registry = CommandRegistry::default_v1();
    for source in [ParseInput::Interactive, ParseInput::Cli, ParseInput::Ipc] {
        if let Ok(commands) = parse_chain(input, source) {
            let _ = registry.expand_chain(commands);
        }
    }
});
