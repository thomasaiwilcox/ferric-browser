#![no_main]

use browser_core::{ApplicationState, Event, Mode, PrivacyKind, SearchCase, ValidatedUrl, reduce};
use libfuzzer_sys::fuzz_target;

fn url(value: &str) -> ValidatedUrl {
    ValidatedUrl::parse(value).expect("fixed fuzz URL")
}

fuzz_target!(|data: &[u8]| {
    let mut state = ApplicationState::new();
    let _ = reduce(
        &mut state,
        Event::CreateProfile {
            label: "fuzz".to_owned(),
            privacy: PrivacyKind::Normal,
        },
    );
    let profile = *state.profiles.keys().next().expect("profile created");
    let _ = reduce(&mut state, Event::CreateWindow { profile });

    for (index, byte) in data.iter().take(256).enumerate() {
        let Some(window) = state.windows.keys().next().copied() else {
            break;
        };
        let Some(tab) = state.windows[&window].active_tab else {
            let _ = reduce(&mut state, Event::OpenTab { window });
            let _ = state.validate();
            continue;
        };
        let Some(target) = state.capture_target(tab) else {
            let _ = state.validate();
            continue;
        };

        match byte % 14 {
            0 => {
                let _ = reduce(&mut state, Event::OpenTab { window });
            }
            1 => {
                let _ = reduce(&mut state, Event::FocusWindow { window });
            }
            2 => {
                let _ = reduce(&mut state, Event::ActivateTab { window, tab });
            }
            3 => {
                let _ = reduce(
                    &mut state,
                    Event::SetTabPinned {
                        tab,
                        pinned: byte & 1 == 0,
                    },
                );
            }
            4 => {
                let _ = reduce(
                    &mut state,
                    Event::SetTabMuted {
                        tab,
                        muted: byte & 1 == 0,
                    },
                );
            }
            5 => {
                let _ = reduce(
                    &mut state,
                    Event::SetTabZoom {
                        tab,
                        zoom_hundredths: 50 + (u32::from(*byte) * 5),
                    },
                );
            }
            6 => {
                let _ = reduce(
                    &mut state,
                    Event::StartNavigation {
                        target,
                        url: url("https://fuzz.example.test/start"),
                    },
                );
            }
            7 => {
                let _ = reduce(
                    &mut state,
                    Event::CommitNavigation {
                        target,
                        url: url("https://fuzz.example.test/commit"),
                        title: format!("fuzz-{index}"),
                    },
                );
            }
            8 => {
                let _ = reduce(&mut state, Event::CompleteNavigation { target });
            }
            9 => {
                let _ = reduce(
                    &mut state,
                    Event::StartSearch {
                        target,
                        query: format!("q-{index}"),
                        backward: byte & 1 == 0,
                        case: SearchCase::Smart,
                    },
                );
            }
            10 => {
                let _ = reduce(
                    &mut state,
                    Event::SearchNext {
                        target,
                        backward: byte & 1 == 0,
                    },
                );
            }
            11 => {
                let _ = reduce(&mut state, Event::EndSearch { target });
            }
            12 => {
                let _ = reduce(
                    &mut state,
                    Event::PushMode {
                        window,
                        mode: match byte % 4 {
                            0 => Mode::Normal,
                            1 => Mode::Command,
                            2 => Mode::Search,
                            _ => Mode::Hint,
                        },
                    },
                );
            }
            _ => {
                let _ = reduce(&mut state, Event::Escape { window });
            }
        }
        assert!(state.validate().is_ok());
    }
});
