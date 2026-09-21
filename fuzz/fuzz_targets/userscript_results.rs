#![no_main]

use ferric_browser_engine_qt::userscript::{FrameScope, Manifest, RunAt, parse_output};
use libfuzzer_sys::fuzz_target;

fn manifest() -> Manifest {
    Manifest {
        schema_version: 1,
        name: "fuzz".to_owned(),
        executable: "fuzz".to_owned(),
        argv: Vec::new(),
        context_fields: vec!["url".to_owned(), "selection".to_owned()],
        allowed_results: vec![
            "message".to_owned(),
            "open".to_owned(),
            "yank".to_owned(),
            "command".to_owned(),
        ],
        allowed_commands: vec!["open".to_owned(), "yank".to_owned(), "spawn".to_owned()],
        timeout_seconds: 30,
        allow_private: false,
        enabled: true,
        actions: Vec::new(),
        matches: Vec::new(),
        excludes: Vec::new(),
        run_at: RunAt::default(),
        frames: FrameScope::default(),
        page_world: false,
        source: None,
    }
}

fuzz_target!(|data: &[u8]| {
    let manifest = manifest();
    let _ = parse_output(data, &manifest);
});
