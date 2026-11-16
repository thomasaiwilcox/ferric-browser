use ferric_browser_core::{
    CheckedHintCodebook, HintAutoFollow, HintCandidate, HintCodeDeclaration, HintCodeExclusion,
    HintCodebookError, HintCodebookOptions, HintGeometry, HintKind, HintReservationCategory,
    MAX_HINT_SPECIAL_RESERVATIONS, assign_labels_with_options, compare_hint_candidates,
};
use std::collections::BTreeSet;

fn options(alphabet: &str, prefix: &str, minimum: u8) -> HintCodebookOptions {
    HintCodebookOptions {
        alphabet: alphabet.into(),
        min_chars: minimum,
        fallback_prefix: prefix.into(),
        excluded_chars: BTreeSet::new(),
        page_source_chars: BTreeSet::new(),
        additional_controls: BTreeSet::new(),
        auto_follow: HintAutoFollow::FullMatch,
    }
}

fn declarations(codes: &[&str]) -> Vec<HintCodeDeclaration> {
    codes
        .iter()
        .enumerate()
        .map(|(slot, code)| HintCodeDeclaration {
            category: HintReservationCategory::Collection,
            rule_id: format!("collection-{}", slot / 64),
            slot: slot % 64,
            code: (*code).into(),
        })
        .collect()
}

fn compile(alphabet: &str, prefix: &str, minimum: u8, codes: &[&str]) -> CheckedHintCodebook {
    CheckedHintCodebook::compile(options(alphabet, prefix, minimum), declarations(codes)).unwrap()
}

// Independent finite-language oracle: enumerate raw strings, then compare
// prefixes directly. It shares neither trie nodes nor rank/count arithmetic.
fn words(alphabet: &[u8], prefix: &str, width: usize, codes: &[&str]) -> Vec<String> {
    let mut raw = vec![prefix.to_owned()];
    for _ in 0..width {
        raw = raw
            .into_iter()
            .flat_map(|word| {
                alphabet.iter().map(move |byte| {
                    let mut next = word.clone();
                    next.push(char::from(*byte));
                    next
                })
            })
            .collect();
    }
    raw.into_iter()
        .filter(|word| {
            codes
                .iter()
                .all(|code| !word.starts_with(code) && !code.starts_with(word))
        })
        .collect()
}

fn prefix_free(codes: &[&str]) -> bool {
    codes.iter().enumerate().all(|(i, left)| {
        codes
            .iter()
            .enumerate()
            .all(|(j, right)| i == j || !left.starts_with(right))
    })
}

#[test]
fn independent_exhaustive_small_spaces_match_trie_capacity_and_rank_enumeration() {
    let pool = ["a", "b", "aa", "ab", "ba", "bb", "x", "xa", "xb"];
    let mut checked = 0;
    for mask in 0..(1 << pool.len()) {
        let codes: Vec<_> = pool
            .iter()
            .enumerate()
            .filter_map(|(i, code)| (mask & (1 << i) != 0).then_some(*code))
            .collect();
        if !prefix_free(&codes) {
            continue;
        }
        for alphabet in ["ab", "ba"] {
            for prefix in ["", "x", "a", "aa"] {
                let book = compile(alphabet, prefix, 1, &codes);
                for width in 1..=4 {
                    let expected = words(alphabet.as_bytes(), prefix, width, &codes);
                    assert_eq!(
                        book.capacity(u8::try_from(width).unwrap()),
                        expected.len() as u64,
                        "codes={codes:?}, prefix={prefix}, alphabet={alphabet}, width={width}"
                    );
                }
                for count in [0, 1, 3, 8] {
                    let maximum = 8 - prefix.len();
                    let expected = (1..=maximum)
                        .map(|width| (width, words(alphabet.as_bytes(), prefix, width, &codes)))
                        .find(|(_, available)| available.len() >= count);
                    match (book.clone().allocator(count), expected) {
                        (Ok(mut allocation), Some((width, expected))) => {
                            assert_eq!(usize::from(allocation.suffix_width()), width);
                            assert!(expected.iter().all(|word| !allocation.has_allocated(word)));
                            assert_eq!(allocation.by_ref().collect::<Vec<_>>(), expected);
                            for word in words(alphabet.as_bytes(), prefix, width, &[]) {
                                assert_eq!(
                                    allocation.has_allocated(&word),
                                    expected.contains(&word)
                                );
                            }
                        }
                        (Err(HintCodebookError::InsufficientCapacity { .. }), None) => {}
                        other => panic!("oracle disagrees: {other:?}"),
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 300);
}

#[test]
fn all_root_reservations_exhaust_every_depth_but_preserve_reserved_only_books() {
    let book = compile("ab", "", 1, &["a", "b"]);
    assert_eq!(book.declarations().len(), 2);
    for width in 1..=8 {
        assert_eq!(book.capacity(width), 0);
    }
    let mut empty = book.clone().allocator(0).unwrap();
    assert_eq!(empty.next(), None);
    assert_eq!(empty.next(), None);
    assert_eq!(
        book.allocator(1),
        Err(HintCodebookError::InsufficientCapacity {
            requested: 1,
            available: 0
        })
    );
}

#[test]
fn exclusions_apply_to_every_character_and_preserve_configured_alphabet_order() {
    let mut policy = options(" /bacd", "", 1);
    policy.excluded_chars.insert('c');
    policy.page_source_chars.insert('d');
    let book = CheckedHintCodebook::compile(policy.clone(), declarations(&["12"])).unwrap();
    assert_eq!(book.configured_alphabet(), " /bacd");
    assert_eq!(book.effective_alphabet(), "ba");
    assert_eq!(book.exclusion('d'), Some(HintCodeExclusion::PageSource));
    assert_eq!(book.allocator(2).unwrap().collect::<Vec<_>>(), ["b", "a"]);
    for (character, reason) in [
        ('c', HintCodeExclusion::Configured),
        ('d', HintCodeExclusion::PageSource),
        ('/', HintCodeExclusion::Control),
    ] {
        let code = format!("1{character}");
        assert_eq!(
            CheckedHintCodebook::compile(policy.clone(), declarations(&[&code])),
            Err(HintCodebookError::ExcludedCodeCharacter {
                index: 0,
                character,
                reason
            })
        );
        let mut prefixed = policy.clone();
        prefixed.fallback_prefix = code;
        assert_eq!(
            CheckedHintCodebook::compile(prefixed, vec![]),
            Err(HintCodebookError::ExcludedPrefixCharacter { character, reason })
        );
    }
    policy.additional_controls.insert('b');
    assert_eq!(
        CheckedHintCodebook::compile(policy.clone(), vec![]),
        Err(HintCodebookError::InvalidEffectiveAlphabet { count: 1 })
    );
    policy.additional_controls.insert('a');
    assert_eq!(
        CheckedHintCodebook::compile(policy, vec![]),
        Err(HintCodebookError::InvalidEffectiveAlphabet { count: 0 })
    );
}

#[test]
fn explicit_numeric_case_and_mixed_codes_do_not_depend_on_the_fallback_alphabet() {
    let book = compile("ab", "", 1, &["1", "Aa", "aB2", "xyztuvwx"]);
    assert_eq!(book.declarations()[1].code, "Aa");
    assert_eq!(
        book.allocator(2).unwrap().collect::<Vec<_>>(),
        ["aa", "ab", "ba", "bb"]
    );
    for codes in [vec!["a", "a2"], vec!["a2", "a"], vec!["1", "1"]] {
        assert!(matches!(
            CheckedHintCodebook::compile(options("ab", "", 1), declarations(&codes)),
            Err(HintCodebookError::PrefixConflict {
                first: 0,
                second: 1
            })
        ));
    }
}

#[test]
fn minimum_counts_the_prefix_and_width_and_cursor_never_change() {
    let mut allocation = compile("ba", "1234567", 8, &[]).allocator(2).unwrap();
    assert_eq!(allocation.suffix_width(), 1);
    assert_eq!(allocation.next().as_deref(), Some("1234567b"));
    assert_eq!(allocation.remaining(), 1);
    assert_eq!(allocation.next().as_deref(), Some("1234567a"));
    assert_eq!(allocation.next(), None);
    assert_eq!(allocation.suffix_width(), 1);
    assert_eq!(
        compile("ab", "x", 3, &[])
            .allocator(1)
            .unwrap()
            .next()
            .as_deref(),
        Some("xaa")
    );
    assert_eq!(
        compile("ab", "", 1, &[]).allocator(5000),
        Err(HintCodebookError::InsufficientCapacity {
            requested: 5000,
            available: 256
        })
    );
}

#[test]
fn exact_reservation_and_eight_character_limits_are_enforced() {
    let codes: Vec<_> = (0..=MAX_HINT_SPECIAL_RESERVATIONS)
        .map(|i| format!("{i:08}"))
        .collect();
    let refs: Vec<_> = codes.iter().map(String::as_str).collect();
    assert_eq!(compile("ab", "", 1, &refs[..512]).declarations().len(), 512);
    assert_eq!(
        CheckedHintCodebook::compile(options("ab", "", 1), declarations(&refs)),
        Err(HintCodebookError::TooManyReservations { count: 513 })
    );
    assert_eq!(
        CheckedHintCodebook::compile(options("ab", "", 1), declarations(&["123456789"])),
        Err(HintCodebookError::InvalidCode { index: 0 })
    );
    assert_eq!(
        CheckedHintCodebook::compile(options("ab", "12345678", 8), vec![]),
        Err(HintCodebookError::InvalidFallbackPrefix)
    );
    for follow in [HintAutoFollow::Always, HintAutoFollow::UniqueMatch] {
        let mut policy = options("ab", "", 1);
        policy.auto_follow = follow;
        assert_eq!(
            CheckedHintCodebook::compile(policy, vec![]),
            Err(HintCodebookError::ConflictingAutoFollow)
        );
    }
}

#[test]
fn full_capacity_is_not_silently_capped_at_the_current_candidate_limit() {
    let book = compile("abcdefghijklmnopqrstuvwxyz012345", "", 8, &[]);
    assert_eq!(book.capacity(8), 1_u64 << 40);
    let mut allocation = book.allocator(5000).unwrap();
    assert_eq!(allocation.capacity(), 1_u64 << 40);
    assert_eq!(allocation.nth(1_000_000).unwrap(), "aaaa4qsa");
    assert_eq!(allocation.remaining(), (1_u64 << 40) - 1_000_001);
    #[cfg(target_pointer_width = "64")]
    {
        let mut final_word = compile("abcdefghijklmnopqrstuvwxyz012345", "", 8, &[])
            .allocator(0)
            .unwrap();
        assert_eq!(
            final_word.nth((1_usize << 40) - 1).as_deref(),
            Some("55555555")
        );
        assert_eq!(final_word.next(), None);
    }
}

#[test]
fn production_bound_deep_reserved_subtrees_have_exact_capacity_without_scanning() {
    let codes: Vec<_> = (0..512).map(|i| format!("{i:08}")).collect();
    let refs: Vec<_> = codes.iter().map(String::as_str).collect();
    let book = compile("0123456789abcdefghijklmnopqrstuv", "", 1, &refs);
    assert_eq!(book.capacity(0), 0);
    assert_eq!(book.capacity(9), 0);
    assert_eq!(book.capacity(1), 31);
    assert_eq!(book.capacity(2), 1023);
    assert_eq!(book.capacity(8), (1_u64 << 40) - 512);
    let mut allocation = book.allocator(5000).unwrap();
    assert_eq!(allocation.suffix_width(), 3);
    assert_eq!(allocation.capacity(), 32767);
    assert_eq!(allocation.next().as_deref(), Some("001"));
    assert_eq!(allocation.nth(32765).as_deref(), Some("vvv"));
    assert_eq!(allocation.next(), None);
}

#[test]
fn invalid_settings_and_duplicate_slot_identity_never_escape_as_a_checked_book() {
    for alphabet in ["aab", "abé", "ab\t"] {
        assert_eq!(
            CheckedHintCodebook::compile(options(alphabet, "", 1), vec![]),
            Err(HintCodebookError::InvalidConfiguredAlphabet)
        );
    }
    for minimum in [0, 9] {
        assert_eq!(
            CheckedHintCodebook::compile(options("ab", "", minimum), vec![]),
            Err(HintCodebookError::InvalidMinimumLength)
        );
    }
    let mut policy = options("ab", "", 1);
    policy.excluded_chars.insert('é');
    assert_eq!(
        CheckedHintCodebook::compile(policy, vec![]),
        Err(HintCodebookError::InvalidExclusion)
    );
    let mut slots = declarations(&["1", "2"]);
    slots[1].slot = slots[0].slot;
    assert_eq!(
        CheckedHintCodebook::compile(options("ab", "", 1), slots),
        Err(HintCodebookError::DuplicateSlot {
            first: 0,
            second: 1
        })
    );
    for id in [String::new(), "invalid id".into(), "x".repeat(65)] {
        let mut slots = declarations(&["1"]);
        slots[0].rule_id = id;
        assert_eq!(
            CheckedHintCodebook::compile(options("ab", "", 1), slots),
            Err(HintCodebookError::InvalidReservation { index: 0 })
        );
    }
    assert_eq!(
        compile("ab", "", 1, &[]).allocator(5001),
        Err(HintCodebookError::TooManyOrdinaryCandidates { count: 5001 })
    );
}

#[test]
fn unicode_page_sources_do_not_make_an_ascii_namespace_unavailable() {
    let mut policy = options("ab", "", 1);
    policy.page_source_chars.extend(['é', '界']);
    let book = CheckedHintCodebook::compile(policy, vec![]).unwrap();
    assert_eq!(book.effective_alphabet(), "ab");
    assert_eq!(book.allocator(2).unwrap().collect::<Vec<_>>(), ["a", "b"]);
}

fn candidate() -> HintCandidate {
    HintCandidate {
        element_id: 10,
        related_element_id: Some(20),
        kind: HintKind::Link,
        frame_path: "0".into(),
        frame_owner: "owner".into(),
        text: "text".into(),
        href: Some("https://example.test/".into()),
        presentation: None,
        geometry: HintGeometry {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        },
    }
}

#[test]
fn canonical_candidate_order_preserves_every_existing_tie_breaker_without_labels() {
    let mut expected = Vec::new();
    let mut item = candidate();
    item.geometry.y = 4.0;
    expected.push(item);
    let mut item = candidate();
    item.geometry.x = 4.0;
    expected.push(item);
    let mut item = candidate();
    item.frame_owner = "earlier".into();
    expected.push(item);
    let mut item = candidate();
    item.frame_path = String::new();
    expected.push(item);
    let mut item = candidate();
    item.element_id = 9;
    expected.push(item);
    let mut item = candidate();
    item.related_element_id = None;
    expected.push(item);
    let mut item = candidate();
    item.related_element_id = Some(19);
    expected.push(item);
    let mut item = candidate();
    item.text = "before".into();
    expected.push(item);
    expected.push(candidate());
    let mut input = expected.clone();
    input.reverse();
    input.sort_by(compare_hint_candidates);
    assert_eq!(input, expected);
    let legacy = assign_labels_with_options(input, "ba", 1).unwrap();
    assert_eq!(
        legacy
            .into_iter()
            .map(|hint| hint.candidate)
            .collect::<Vec<_>>(),
        expected
    );
}
