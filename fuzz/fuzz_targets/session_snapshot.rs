#![no_main]

use browser_storage::SessionSnapshot;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(snapshot) = serde_json::from_slice::<SessionSnapshot>(data) else {
        return;
    };
    let _ = snapshot.restore_plan();
});
