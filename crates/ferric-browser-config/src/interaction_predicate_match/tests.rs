use super::*;
use InteractionAttributeEvidence::{CompleteAbsent, CompletePresent, Missing, Truncated};
use InteractionPredicateEvidenceError as Error;
use serde_json::{Value, json};

fn compile(value: Value) -> CompiledInteractionPredicates {
    let authored: InteractionPredicates = serde_json::from_value(value).unwrap();
    CompiledInteractionPredicates::try_from(&authored).unwrap()
}

fn attribute(operator: &str, value: Value) -> CompiledInteractionAttributeTest {
    let mut authored = json!({"name":"data-id"});
    authored[operator] = value;
    CompiledInteractionAttributeTest::try_from(
        &serde_json::from_value::<InteractionAttributeTest>(authored).unwrap(),
    )
    .unwrap()
}

#[test]
fn attribute_absence_completeness_and_empty_operands_are_distinct() {
    for test in [
        attribute("equals", json!("")),
        attribute("prefix", json!("")),
        attribute("present", json!(true)),
    ] {
        let mut work = InteractionPredicateWork::default();
        assert_eq!(test.matches(CompletePresent(""), &mut work), Ok(true));
        assert_eq!(test.matches(CompleteAbsent, &mut work), Ok(false));
        assert_eq!(test.matches(Missing, &mut work), Err(Error::Missing));
        assert_eq!(test.matches(Truncated, &mut work), Err(Error::Truncated));
        assert_eq!(work.remaining(), MAX_INTERACTION_PREDICATE_STEPS - 4);
    }
    let mut work = InteractionPredicateWork::default();
    let exact = attribute("equals", json!(" Café "));
    assert_eq!(exact.name(), "data-id");
    assert_eq!(
        exact.matches(CompletePresent(" Café "), &mut work),
        Ok(true)
    );
    for text in ["Café", " CAFÉ ", " Cafe\u{301} ", " Café more "] {
        assert_eq!(exact.matches(CompletePresent(text), &mut work), Ok(false));
    }
    let prefix = attribute("prefix", json!("é"));
    assert_eq!(
        prefix.matches(CompletePresent("é-more"), &mut work),
        Ok(true)
    );
    assert_eq!(
        prefix.matches(CompletePresent("e\u{301}-more"), &mut work),
        Ok(false)
    );
    // Long complete values need no invented cap; prefix/presence do no full scan.
    let long = format!("é{}", "x".repeat(MAX_INTERACTION_PREDICATE_STEPS + 1));
    assert_eq!(prefix.matches(CompletePresent(&long), &mut work), Ok(true));
    assert_eq!(
        attribute("present", json!(true)).matches(CompletePresent(&long), &mut work),
        Ok(true)
    );
}

#[test]
fn role_is_exact_membership_with_html_ascii_whitespace_only() {
    let mut work = InteractionPredicateWork::default();
    for token in ["button", "sélecteur", "custom:token", "custom\u{a0}token"] {
        let role = CompiledInteractionRoleMatch::compile(token).unwrap();
        assert_eq!(role.token(), token);
        for separator in [' ', '\t', '\n', '\r', '\u{c}'] {
            let actual = format!("{separator}other{separator}{token}{separator}last{separator}");
            assert_eq!(role.matches(CompletePresent(&actual), &mut work), Ok(true));
        }
        assert_eq!(role.matches(CompletePresent(token), &mut work), Ok(true));
        assert_eq!(
            role.matches(CompletePresent("BUTTON"), &mut work),
            Ok(false)
        );
        assert_eq!(role.matches(CompleteAbsent, &mut work), Ok(false));
        assert_eq!(role.matches(Missing, &mut work), Err(Error::Missing));
        assert_eq!(role.matches(Truncated, &mut work), Err(Error::Truncated));
    }
    let role = CompiledInteractionRoleMatch::compile("button").unwrap();
    for actual in [
        "",
        "buttons",
        "a\u{a0}button",
        "button\u{a0}a",
        "a\u{b}button",
    ] {
        assert_eq!(
            role.matches(CompletePresent(actual), &mut work),
            Ok(false),
            "{actual:?}"
        );
    }
}

fn href(origin: Option<&str>, path: Option<&str>) -> CompiledInteractionHrefMatch {
    CompiledInteractionHrefMatch::try_from(&InteractionHrefMatch {
        origin: origin.map(str::to_owned),
        path: path.map(str::to_owned),
    })
    .unwrap()
}

#[test]
fn href_is_link_only_origin_and_encoded_path_conjunction() {
    let mut work = InteractionPredicateWork::default();
    let exact = href(Some("https://example.test"), Some("/watch*"));
    for value in [
        "https://example.test/watch",
        "https://example.test/watch/12?q=else#fragment",
        "https://example.test/watch?path=/else",
    ] {
        assert_eq!(
            exact.matches(Some(true), CompletePresent(value), &mut work),
            Ok(true)
        );
    }
    for value in [
        "http://example.test/watch",
        "https://example.test:8766/watch",
        "https://other.test/watch",
        "https://example.test/else?path=/watch",
        "mailto:/watch",
        "data:text/plain,/watch",
    ] {
        assert_eq!(
            exact.matches(Some(true), CompletePresent(value), &mut work),
            Ok(false),
            "{value}"
        );
    }
    assert_eq!(exact.matches(Some(false), Missing, &mut work), Ok(false));
    assert_eq!(
        exact.matches(
            None,
            CompletePresent("https://example.test/watch"),
            &mut work
        ),
        Err(Error::Missing)
    );
    assert_eq!(
        exact.matches(Some(true), Missing, &mut work),
        Err(Error::Missing)
    );
    assert_eq!(
        exact.matches(Some(true), Truncated, &mut work),
        Err(Error::Truncated)
    );
    assert_eq!(
        exact.matches(Some(true), CompleteAbsent, &mut work),
        Ok(false)
    );
    for value in ["", "relative", "https://example.test/\nwatch", "http://["] {
        assert_eq!(
            exact.matches(Some(true), CompletePresent(value), &mut work),
            Err(Error::InvalidHref)
        );
    }
    for (origin, value) in [
        ("http://example.test:8766", "http://example.test:8766"),
        ("https://[::1]", "https://[::1]/?x#y"),
    ] {
        assert_eq!(
            href(Some(origin), Some("/")).matches(Some(true), CompletePresent(value), &mut work),
            Ok(true)
        );
    }
    let encoded = href(None, Some("/a%2Fb"));
    assert_eq!(
        encoded.matches(
            Some(true),
            CompletePresent("https://x.test/a%2Fb?q#hash"),
            &mut work
        ),
        Ok(true)
    );
    for value in [
        "https://x.test/a/b",
        "https://x.test/a%2fb",
        "https://x.test/else?x=/a%2Fb",
    ] {
        assert_eq!(
            encoded.matches(Some(true), CompletePresent(value), &mut work),
            Ok(false)
        );
    }
    // Brackets/question marks have no extra glob language; query is never path.
    assert_eq!(
        href(None, Some("/a[bc]")).matches(
            Some(true),
            CompletePresent("https://x.test/a[bc]"),
            &mut work
        ),
        Ok(true)
    );
    assert_eq!(
        href(None, Some("/a[bc]")).matches(
            Some(true),
            CompletePresent("https://x.test/ab"),
            &mut work
        ),
        Ok(false)
    );
}

#[test]
fn conjunction_names_geometry_and_unrequested_facts_are_separate() {
    let compiled = compile(json!({
        "hint_name":{"equals":" café ","case":"ascii-insensitive"},
        "href":{"origin":"https://example.test","path":"/watch*"},
        "role":"button", "attributes":[{"name":"data-id","prefix":"id-"}],
        "width_px":{"min":10.,"max":20.}, "height_px":{"min":0.,"max":0.},
        "viewport_x":{"min":0.,"max":1.}, "viewport_y":{"min":0.25,"max":0.75}
    }));
    let valid = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete("\u{a0}cafe\u{301}\t").unwrap(),
        is_link: Some(true),
        href: CompletePresent("https://example.test/watch?v=1#x"),
        role: CompletePresent("presentation button"),
        width_px: Some(10.),
        height_px: Some(0.),
        viewport_x: Some(1.),
        viewport_y: Some(0.25),
    };
    let mut work = InteractionPredicateWork::default();
    assert_eq!(
        compiled.matches(
            &valid,
            |name| {
                assert_eq!(name, "data-id");
                CompletePresent("id-1")
            },
            &mut work
        ),
        Ok(true)
    );
    assert_eq!(
        compiled.matches(
            &InteractionPredicateFacts {
                width_px: Some(20.),
                viewport_x: Some(0.),
                viewport_y: Some(0.75),
                ..valid
            },
            |_| CompletePresent("id-1"),
            &mut work
        ),
        Ok(true)
    );
    assert_eq!(
        compiled.matches(&valid, |_| CompletePresent("other"), &mut work),
        Ok(false)
    );
    assert_eq!(
        compiled.matches(&valid, |_| Missing, &mut work),
        Err(Error::Missing)
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let facts = InteractionPredicateFacts {
            width_px: Some(value),
            ..valid
        };
        assert_eq!(
            compiled.matches(&facts, |_| CompletePresent("id-1"), &mut work),
            Err(Error::InvalidGeometry)
        );
    }
    let absent = InteractionPredicateFacts {
        width_px: None,
        ..valid
    };
    assert_eq!(
        compiled.matches(&absent, |_| CompletePresent("id-1"), &mut work),
        Err(Error::Missing)
    );
    for width in [
        f64::from_bits(10_f64.to_bits() - 1),
        f64::from_bits(20_f64.to_bits() + 1),
    ] {
        assert_eq!(
            compiled.matches(
                &InteractionPredicateFacts {
                    width_px: Some(width),
                    ..valid
                },
                |_| CompletePresent("id-1"),
                &mut work
            ),
            Ok(false)
        );
    }
}

#[test]
fn unrequested_facts_and_unavailable_names_do_not_acquire_meaning() {
    let mut work = InteractionPredicateWork::default();
    let valid = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete(" cafe\u{301} ").unwrap(),
        ..Default::default()
    };
    assert_eq!(
        compile(json!({})).matches(
            &InteractionPredicateFacts::default(),
            |_| panic!("no requested attribute"),
            &mut work
        ),
        Ok(true)
    );
    let name = compile(json!({"hint_name":{"contains":"fé"}}));
    assert_eq!(name.matches(&valid, |_| Missing, &mut work), Ok(true));
    for evidence in [
        InteractionNameEvidence::missing(),
        InteractionNameEvidence::truncated(),
    ] {
        assert!(matches!(
            name.matches(
                &InteractionPredicateFacts {
                    hint_name: evidence,
                    ..valid
                },
                |_| Missing,
                &mut work
            ),
            Err(Error::Name(_))
        ));
    }
}

#[test]
fn work_is_shared_exact_exhaustion_never_becomes_a_nonmatch() {
    let test = attribute("equals", json!("x"));
    let mut work = InteractionPredicateWork::default();
    work.charge(MAX_INTERACTION_PREDICATE_STEPS - 4).unwrap();
    assert_eq!(test.matches(CompletePresent("x"), &mut work), Ok(true));
    assert_eq!(work.remaining(), 2);
    assert_eq!(test.matches(CompletePresent("x"), &mut work), Ok(true));
    assert_eq!(work.remaining(), 0);
    let mut unavailable_work = InteractionPredicateWork::default();
    unavailable_work
        .charge(MAX_INTERACTION_PREDICATE_STEPS - 2)
        .unwrap();
    let names = compile(json!({"hint_name":{"equals":""}}));
    for evidence in [
        InteractionNameEvidence::missing(),
        InteractionNameEvidence::truncated(),
    ] {
        assert!(matches!(
            names.matches(
                &InteractionPredicateFacts {
                    hint_name: evidence,
                    ..Default::default()
                },
                |_| Missing,
                &mut unavailable_work
            ),
            Err(Error::Name(_))
        ));
    }
    assert_eq!(unavailable_work.remaining(), 0);
    assert_eq!(
        names.matches(
            &InteractionPredicateFacts::default(),
            |_| Missing,
            &mut unavailable_work
        ),
        Err(Error::WorkExhausted)
    );
    assert_eq!(
        test.matches(CompletePresent("x"), &mut work),
        Err(Error::WorkExhausted)
    );
    assert_eq!(work.remaining(), 0);
    let mut work = InteractionPredicateWork::default();
    let long = "x".repeat(MAX_INTERACTION_PREDICATE_STEPS + 1);
    assert_eq!(
        CompiledInteractionRoleMatch::compile("button")
            .unwrap()
            .matches(CompletePresent(&long), &mut work),
        Err(Error::WorkExhausted)
    );
    let name = compile(json!({"hint_name":{"equals":""}}));
    assert_eq!(
        name.matches(
            &InteractionPredicateFacts {
                hint_name: InteractionNameEvidence::complete("").unwrap(),
                ..Default::default()
            },
            |_| Missing,
            &mut work
        ),
        Err(Error::WorkExhausted)
    );
}

#[test]
fn conjunction_reports_the_first_unavailable_or_proven_false_constraint() {
    let both = compile(json!({"hint_name":{"equals":"right"},"role":"button"}));
    let mut work = InteractionPredicateWork::default();
    // A known false name proves the conjunction false without a role fact.
    let false_name = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete("wrong").unwrap(),
        ..Default::default()
    };
    assert_eq!(both.matches(&false_name, |_| Missing, &mut work), Ok(false));
    // Unknown first facts do not permit a positive result or claim a later
    // contradiction was evaluated. The evaluator conservatively stops here.
    let unknown_name = InteractionPredicateFacts {
        role: CompletePresent("link"),
        ..Default::default()
    };
    assert_eq!(
        both.matches(&unknown_name, |_| Missing, &mut work),
        Err(Error::Name(InteractionNameEvidenceError::Missing))
    );
}

#[test]
fn href_transport_and_glob_retry_work_are_bounded_separately() {
    let any = href(None, Some("/*"));
    for bytes in [
        MAX_INTERACTION_HREF_EVIDENCE_BYTES - 1,
        MAX_INTERACTION_HREF_EVIDENCE_BYTES,
        MAX_INTERACTION_HREF_EVIDENCE_BYTES + 1,
    ] {
        let value = format!(
            "https://x.test/{}",
            "x".repeat(bytes - "https://x.test/".len())
        );
        let result = any.matches(
            Some(true),
            CompletePresent(&value),
            &mut InteractionPredicateWork::default(),
        );
        assert_eq!(
            result,
            if bytes <= MAX_INTERACTION_HREF_EVIDENCE_BYTES {
                Ok(true)
            } else {
                Err(Error::HrefTooLong)
            }
        );
    }
    // Input length plus one does not account for a star's repeated comparisons.
    let hard = href(None, Some(&format!("/*{}z", "a".repeat(256))));
    let value = format!("https://x.test/{}", "a".repeat(1000));
    let mut work = InteractionPredicateWork::default();
    assert_eq!(
        hard.matches(Some(true), CompletePresent(&value), &mut work),
        Err(Error::WorkExhausted)
    );
    assert_eq!(work.remaining(), 0);
    for (pattern, value, steps, expected) in [
        ("", "", 0, true),
        ("*", "", 1, true),
        ("/a*", "/abc", 5, true),
        ("/a", "/b", 2, false),
    ] {
        let mut remaining = steps;
        let matched = crate::routing::glob_matches_with_work(pattern, value, || {
            if remaining == 0 {
                false
            } else {
                remaining -= 1;
                true
            }
        });
        assert_eq!(matched, Some(expected), "{pattern}/{value}");
    }
    let mut remaining = 4;
    assert_eq!(
        crate::routing::glob_matches_with_work("/a*", "/abc", || {
            if remaining == 0 {
                false
            } else {
                remaining -= 1;
                true
            }
        }),
        None
    );
}

#[test]
fn glob_wildcards_remain_wildcards_when_a_url_contains_stars() {
    for (path, url) in [("/*", "https://x.test/**"), ("/*a", "https://x.test/*ba")] {
        let predicate = href(None, Some(path));
        assert_eq!(
            predicate.matches(
                Some(true),
                CompletePresent(url),
                &mut InteractionPredicateWork::default()
            ),
            Ok(true)
        );
        let rule = crate::SiteRule {
            id: "wildcard".into(),
            pattern: format!("https://x.test{path}"),
            priority: 0,
            set: [("content.zoom".into(), toml::Value::Float(1.0))].into(),
            interaction: None,
        };
        crate::validate_site_rule(&rule).unwrap();
        let config = crate::Config {
            site_rules: vec![rule],
            ..Default::default()
        };
        assert_eq!(crate::matching_site_rules(&config, url).len(), 1);
        let contexts = crate::ContextsConfig {
            schema_version: 3,
            contexts: Vec::new(),
            routes: vec![crate::ContextRoute {
                id: "wildcard".into(),
                pattern: format!("https://x.test{path}"),
                context: "default".into(),
                priority: 0,
                behavior: "prompt".into(),
                entry_points: vec!["explicit-open".into()],
            }],
        };
        assert_eq!(
            crate::matching_context_routes(&contexts, url, "explicit-open").len(),
            1
        );
    }
}

#[test]
fn glob_state_machine_agrees_with_independent_short_string_oracle() {
    // Independent DP: a star always consumes zero or more bytes; literals
    // consume exactly themselves. Includes stars in both patterns and targets.
    fn oracle(pattern: &str, target: &str) -> bool {
        let mut rows = vec![vec![false; target.len() + 1]; pattern.len() + 1];
        rows[0][0] = true;
        for (i, left) in pattern.bytes().enumerate() {
            rows[i + 1][0] = left == b'*' && rows[i][0];
            for (j, right) in target.bytes().enumerate() {
                rows[i + 1][j + 1] = if left == b'*' {
                    rows[i][j + 1] || rows[i + 1][j]
                } else {
                    left == right && rows[i][j]
                };
            }
        }
        rows[pattern.len()][target.len()]
    }
    let mut strings = vec![String::new()];
    let mut previous = vec![String::new()];
    for _ in 0..4 {
        let mut next = Vec::new();
        for prefix in &previous {
            for suffix in ["a", "*", "/", "é"] {
                next.push(format!("{prefix}{suffix}"));
            }
        }
        strings.extend(next.iter().cloned());
        previous = next;
    }
    for pattern in &strings {
        for target in &strings {
            let mut work = InteractionPredicateWork::default();
            assert_eq!(
                crate::routing::glob_matches_with_work(pattern, target, || work.charge(1).is_ok()),
                Some(oracle(pattern, target)),
                "pattern={pattern:?}, target={target:?}"
            );
        }
    }
}
