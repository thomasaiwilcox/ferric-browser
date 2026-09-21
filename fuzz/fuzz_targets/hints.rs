#![no_main]

use browser_core::{HintCandidate, HintGeometry, HintKind, assign_labels};
use libfuzzer_sys::fuzz_target;

fn candidate(bytes: &[u8], index: usize) -> HintCandidate {
    let first = bytes.first().copied().unwrap_or_default();
    let second = bytes.get(1).copied().unwrap_or_default();
    let kind = match first % 7 {
        0 => HintKind::Link,
        1 => HintKind::Button,
        2 => HintKind::Input,
        3 => HintKind::Select,
        4 => HintKind::Textarea,
        5 => HintKind::ContentEditable,
        _ => HintKind::Aria,
    };
    HintCandidate {
        element_id: u32::try_from(index).unwrap_or(u32::MAX),
        kind,
        frame_path: format!("frame-{}", second % 4),
        text: format!("candidate-{index}-{}", first % 16),
        href: (kind == HintKind::Link).then(|| "https://example.test/".to_owned()),
        geometry: HintGeometry {
            x: f64::from(first),
            y: f64::from(second),
            width: 1.0 + f64::from(bytes.get(2).copied().unwrap_or_default()) / 16.0,
            height: 1.0 + f64::from(bytes.get(3).copied().unwrap_or_default()) / 16.0,
        },
    }
}

fuzz_target!(|data: &[u8]| {
    if data.first() == Some(&u8::MAX) {
        let candidates = (0..5_001)
            .map(|index| candidate(data.get(1..).unwrap_or_default(), index))
            .collect();
        let _ = assign_labels(candidates);
        return;
    }
    let candidates = data
        .chunks(4)
        .take(128)
        .enumerate()
        .map(|(index, chunk)| candidate(chunk, index))
        .collect();
    let _ = assign_labels(candidates);
});
