use super::*;
use std::hint::black_box;
use std::time::Instant;

fn synthetic_hint_payload(count: usize) -> String {
    let candidates = (0..count)
        .map(|index| {
            serde_json::json!({
                "element_id": index + 1,
                "kind": "button",
                "frame_path": "0",
                "text": format!("Synthetic target {index}"),
                "href": null,
                "geometry": {
                    "x": (index % 25) * 20,
                    "y": (index / 25) * 20,
                    "width": 16.0,
                    "height": 16.0
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "candidates": candidates }).to_string()
}

fn hint_pipeline_sample(payload: &str) -> serde_json::Value {
    let started = Instant::now();
    let candidates = parse_hint_payload(payload).expect("synthetic candidate payload");
    let decode_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let alphabet = "asdfjklgh";
    let labeled = assign_labels_with_options(candidates, alphabet, 1).expect("labels");
    let labels_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let mut interaction = HintInteraction::new(
        labeled.clone(),
        alphabet.to_owned(),
        CoreHintAutoFollow::Never,
    );
    let setup_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let outcome = interaction.handle(HintInteractionInput::SetText(
        "synthetic target 42".to_owned(),
    ));
    let ranking_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(outcome, HintInteractionOutcome::Updated);

    let started = Instant::now();
    let snapshot = interaction.snapshot();
    let snapshot_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert!(snapshot.remaining > 0);
    black_box(snapshot.remaining);

    serde_json::json!({
        "decode_ms": decode_ms,
        "labels_ms": labels_ms,
        "interaction_setup_ms": setup_ms,
        "ranking_ms": ranking_ms,
        "snapshot_ms": snapshot_ms,
        "matches": snapshot.remaining,
        "count": snapshot.total
    })
}

#[test]
#[ignore = "opt-in C17 performance qualification; invoked explicitly by the S17 benchmark command"]
fn s17_rust_hint_pipeline_performance() {
    for count in [500_usize, 1_000, 5_000] {
        let payload = synthetic_hint_payload(count);
        let mut expected = serde_json::Value::Null;
        for _ in 0..10 {
            expected = hint_pipeline_sample(&payload);
        }

        let samples = (0..100)
            .map(|_| hint_pipeline_sample(&payload))
            .collect::<Vec<_>>();
        println!(
            "FERRIC_RUST_PERFORMANCE_JSON {}",
            serde_json::json!({
                "scenario": format!("rust-{count}"),
                "count": count,
                "warmups": 10,
                "payload_bytes": payload.len(),
                "samples": samples,
                "last_sample_shape": expected
            })
        );
    }
}
