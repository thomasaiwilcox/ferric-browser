#![no_main]

use browser_core::{BindingResolver, BindingTrie, CommandRegistry, Mode};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let registry = CommandRegistry::default_v1();
    let Ok(trie) = BindingTrie::default_v1(registry) else {
        return;
    };
    let mut resolver = BindingResolver::new(trie, Mode::Normal);
    for (index, byte) in data.iter().enumerate() {
        let key = char::from(*byte).to_string();
        let _ = resolver.feed(&key, index as u64);
        let _ = resolver.tick(index as u64 + 1_000);
    }
    let _ = resolver.definitions();
});
