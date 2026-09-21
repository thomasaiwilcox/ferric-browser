#![no_main]

use ferric_browser_core::{switcher_rank, tokenize_switcher_query};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let query = tokenize_switcher_query(input);
    let fields = vec![input.to_owned(), input.to_lowercase()];
    let _ = switcher_rank(&query, input, input, &fields);
});
