//! Opt-in measurement of event dispatch with large tab collections.
use std::{hint::black_box, time::Instant};

use ferric_browser_core::{Event, PrivacyKind};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("tabs,sample,iterations,nanoseconds_per_dispatch");
    for tabs in [100, 1_000, 5_000] {
        let (mut runtime, window, tab) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "benchmark")?;
        for _ in 1..tabs {
            runtime.handle(RuntimeInput::Lifecycle(Event::OpenTab { window }))?;
        }
        let target = runtime
            .view()
            .capture_target(tab)
            .ok_or("missing benchmark tab")?;
        let input = RuntimeInput::UserDecision(Event::Reload {
            target,
            bypass_cache: false,
        });
        for _ in 0..100 {
            black_box(runtime.handle(input.clone())?);
        }
        for sample in 1..=3 {
            let started = Instant::now();
            for _ in 0..1_000 {
                black_box(runtime.handle(input.clone())?);
            }
            println!(
                "{tabs},{sample},1000,{}",
                started.elapsed().as_nanos() / 1_000
            );
        }
    }
    Ok(())
}
