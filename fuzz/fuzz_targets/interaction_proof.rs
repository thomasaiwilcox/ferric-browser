#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ferric_browser_engine_qt::interaction_proof::fuzz_support::exercise_annotations(data);
});
