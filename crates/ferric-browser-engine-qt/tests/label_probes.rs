//! Opt-in S25 ergonomics model using the production `HintInteraction` and label
//! assignment APIs. This compares deterministic synthetic codes; it is not a
//! user study and does not change production defaults.

use std::collections::{BTreeMap, BTreeSet};

use ferric_browser_core::{
    HintAutoFollow, HintCandidate, HintGeometry, HintInputMode, HintInteraction,
    HintInteractionInput, HintInteractionOutcome, HintKind, LabeledHint, MAX_HINT_CANDIDATES,
    assign_labels, assign_labels_with_options, refresh_labels,
};
use serde::{Deserialize, Serialize};

const DEFAULT_ALPHABET: &str = "asdfghjkl";
const NUMERIC_ALPHABET: &str = "0123456789";
const CORPUS_SOURCE: &str = include_str!("fixtures/webapp-interaction/label-corpus.json");

#[derive(Clone, Debug, Deserialize)]
struct Corpus {
    schema_version: u32,
    candidate_counts: Vec<usize>,
    assumed_ms_per_gesture: u64,
    filler_prefix: String,
    task_targets: Vec<TaskTarget>,
    word_dictionary: Vec<String>,
    mutation: Mutation,
}

#[derive(Clone, Debug, Deserialize)]
struct TaskTarget {
    id: String,
    accessible_name: String,
    query: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Mutation {
    remove_element_id_modulus: u32,
    add_replacements: bool,
    width_boundary: WidthBoundary,
}

#[derive(Clone, Debug, Deserialize)]
struct WidthBoundary {
    from: usize,
    to: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Strategy {
    StableLabels,
    Numeric,
    Scattered,
    WordPairs,
    TextSearch,
}

impl Strategy {
    const ALL: [Self; 5] = [
        Self::StableLabels,
        Self::Numeric,
        Self::Scattered,
        Self::WordPairs,
        Self::TextSearch,
    ];
}

#[derive(Debug, Serialize)]
struct TaskMeasurement {
    task_id: String,
    target_index: usize,
    query_chars: usize,
    input_gestures: usize,
    assumed_time_ms: u64,
    activation_confirmed: bool,
}

#[derive(Debug, Serialize)]
struct ScenarioMeasurement {
    candidate_count: usize,
    strategy: Strategy,
    task_count: usize,
    median_input_gestures: f64,
    median_assumed_time_ms: f64,
    average_label_characters: f64,
    maximum_adjacent_first_key_run: usize,
    labels_unique: bool,
    labels_prefix_free: bool,
    deterministic: bool,
    modeled_mismatches: usize,
    task_measurements: Vec<TaskMeasurement>,
}

#[derive(Debug, Serialize)]
struct ChurnMeasurement {
    strategy: Strategy,
    scenario: String,
    previous_candidates: usize,
    refreshed_candidates: usize,
    retained_candidates: usize,
    changed_retained_labels: usize,
    churn_fraction: f64,
}

#[derive(Debug, Serialize)]
struct Evaluation {
    schema_version: u32,
    evidence_class: &'static str,
    fixture: &'static str,
    assumed_ms_per_gesture: u64,
    task_count: usize,
    candidate_counts: Vec<usize>,
    results: Vec<ScenarioMeasurement>,
    refresh_churn: Vec<ChurnMeasurement>,
    human_error_rate: &'static str,
    limitations: Vec<&'static str>,
}

fn read_corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(CORPUS_SOURCE).expect("fixed S25 corpus parses");
    assert_eq!(corpus.schema_version, 1);
    assert!(!corpus.task_targets.is_empty());
    assert!(corpus.word_dictionary.len() >= 72);
    assert!(
        corpus
            .word_dictionary
            .iter()
            .all(|word| { !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase()) })
    );
    assert!(corpus.word_dictionary.len() * corpus.word_dictionary.len() >= MAX_HINT_CANDIDATES);
    let queries = corpus
        .task_targets
        .iter()
        .map(|task| task.query.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(queries.len(), corpus.task_targets.len());
    corpus
}

fn candidate(index: usize, text: String) -> HintCandidate {
    let element_id = u32::try_from(index + 1).expect("synthetic target ID fits u32");
    HintCandidate {
        element_id,
        related_element_id: None,
        kind: HintKind::Button,
        frame_path: "0".into(),
        text,
        href: None,
        presentation: None,
        geometry: HintGeometry {
            x: f64::from(u32::try_from(index * 2).expect("bounded fixture coordinate fits u32")),
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
    }
}

fn candidates(corpus: &Corpus, count: usize) -> Vec<HintCandidate> {
    assert!(count >= corpus.task_targets.len());
    assert!(count <= MAX_HINT_CANDIDATES);
    let mut result = corpus
        .task_targets
        .iter()
        .enumerate()
        .map(|(index, task)| candidate(index, task.accessible_name.clone()))
        .collect::<Vec<_>>();
    for index in result.len()..count {
        result.push(candidate(
            index,
            format!("{} {:05}", corpus.filler_prefix, index),
        ));
    }
    result
}

fn labels_for(strategy: Strategy, input: Vec<HintCandidate>, corpus: &Corpus) -> Vec<LabeledHint> {
    match strategy {
        Strategy::StableLabels | Strategy::TextSearch => {
            assign_labels(input).expect("production default labels")
        }
        Strategy::Numeric => assign_labels_with_options(input, NUMERIC_ALPHABET, 1)
            .expect("fixed-width numeric prototype labels"),
        Strategy::Scattered => {
            let mut hints = assign_labels(input).expect("base labels for scatter prototype");
            for hint in &mut hints {
                hint.label = hint.label.chars().rev().collect();
            }
            hints
        }
        Strategy::WordPairs => word_labels(input, &corpus.word_dictionary),
    }
}

fn word_label(index: usize, words: &[String]) -> String {
    let capacity = words.len() * words.len();
    assert!(
        index < capacity,
        "fixed dictionary covers the candidate cap"
    );
    format!(
        "{} {}",
        words[index / words.len()],
        words[index % words.len()]
    )
}

fn word_labels(input: Vec<HintCandidate>, words: &[String]) -> Vec<LabeledHint> {
    input
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| LabeledHint {
            label: word_label(index, words),
            candidate,
        })
        .collect()
}

fn input_alphabet(strategy: Strategy, words: &[String]) -> String {
    match strategy {
        Strategy::StableLabels | Strategy::Scattered | Strategy::TextSearch => {
            DEFAULT_ALPHABET.to_owned()
        }
        Strategy::Numeric => NUMERIC_ALPHABET.to_owned(),
        Strategy::WordPairs => words
            .iter()
            .flat_map(|word| word.chars())
            .chain(std::iter::once(' '))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn activate_by_label(hints: &[LabeledHint], alphabet: &str, target_id: u32) -> usize {
    let target = hints
        .iter()
        .find(|hint| hint.candidate.element_id == target_id)
        .expect("task identity is in the label corpus");
    let target_label = target.label.clone();
    let mut interaction = HintInteraction::new(
        hints.to_vec(),
        alphabet.to_owned(),
        HintAutoFollow::FullMatch,
    );
    let characters = target_label.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().copied().enumerate() {
        let outcome = interaction.handle(HintInteractionInput::Character(character));
        if index + 1 == characters.len() {
            assert_eq!(
                outcome,
                HintInteractionOutcome::Activate(target_label.clone()),
                "the full deterministic code activates its exact identity"
            );
        } else {
            assert_eq!(outcome, HintInteractionOutcome::Updated);
        }
    }
    characters.len()
}

fn activate_by_text_search(hints: &[LabeledHint], query: &str, target_id: u32) -> (usize, usize) {
    let mut interaction = HintInteraction::new(
        hints.to_vec(),
        DEFAULT_ALPHABET.to_owned(),
        HintAutoFollow::FullMatch,
    );
    assert_eq!(
        interaction.handle(HintInteractionInput::EnterTextMode),
        HintInteractionOutcome::Updated
    );
    for character in query.chars() {
        assert_eq!(
            interaction.handle(HintInteractionInput::Character(character)),
            HintInteractionOutcome::Updated
        );
    }
    assert_eq!(interaction.snapshot().mode, HintInputMode::Text);
    let snapshot = interaction.snapshot();
    assert_eq!(snapshot.matching_labels.len(), 1, "query must be unique");
    let selected_id = hints
        .iter()
        .find(|hint| Some(hint.label.as_str()) == snapshot.active_label.as_deref())
        .map(|hint| hint.candidate.element_id)
        .expect("ranked result maps to an exact candidate");
    assert_eq!(
        selected_id, target_id,
        "ranked result retains task identity"
    );
    let confirmed = interaction.handle(HintInteractionInput::Activate);
    assert_eq!(
        confirmed,
        HintInteractionOutcome::Activate(snapshot.active_label.expect("one active result"))
    );
    (1 + query.chars().count() + 1, query.chars().count())
}

fn is_prefix_free(hints: &[LabeledHint]) -> bool {
    let labels = hints
        .iter()
        .map(|hint| hint.label.as_str())
        .collect::<BTreeSet<_>>();
    if labels.len() != hints.len() {
        return false;
    }
    labels
        .iter()
        .all(|label| (1..label.len()).all(|length| !labels.contains(&label[..length])))
}

fn maximum_first_key_run(hints: &[LabeledHint]) -> usize {
    let mut previous = None;
    let mut current = 0;
    let mut maximum = 0;
    for hint in hints {
        let first = hint.label.chars().next().expect("labels are nonempty");
        if previous == Some(first) {
            current += 1;
        } else {
            previous = Some(first);
            current = 1;
        }
        maximum = maximum.max(current);
    }
    maximum
}

fn median(values: &mut [usize]) -> f64 {
    values.sort_unstable();
    match values.len() {
        0 => 0.0,
        count if count % 2 == 0 => {
            f64::from(u32::try_from(values[count / 2 - 1]).expect("bounded gesture count"))
                .midpoint(f64::from(
                    u32::try_from(values[count / 2]).expect("bounded gesture count"),
                ))
        }
        count => f64::from(u32::try_from(values[count / 2]).expect("bounded gesture count")),
    }
}

fn label_width(count: usize, base: usize) -> usize {
    let mut capacity = base;
    let mut width = 1;
    while capacity < count {
        capacity = capacity.saturating_mul(base);
        width += 1;
    }
    width
}

fn encode(mut index: usize, width: usize, alphabet: &[u8]) -> String {
    let base = alphabet.len();
    let mut bytes = vec![alphabet[0]; width];
    for slot in bytes.iter_mut().rev() {
        *slot = alphabet[index % base];
        index /= base;
    }
    String::from_utf8(bytes).expect("prototype alphabet is ASCII")
}

fn refresh_scattered(mut input: Vec<HintCandidate>, previous: &[LabeledHint]) -> Vec<LabeledHint> {
    input.sort_by(|left, right| {
        left.geometry
            .y
            .total_cmp(&right.geometry.y)
            .then_with(|| left.geometry.x.total_cmp(&right.geometry.x))
            .then_with(|| left.element_id.cmp(&right.element_id))
    });
    let width = label_width(input.len(), DEFAULT_ALPHABET.len());
    if previous
        .first()
        .is_some_and(|hint| hint.label.len() != width)
    {
        return labels_for(Strategy::Scattered, input, &read_corpus());
    }
    let old = previous
        .iter()
        .map(|hint| (hint.candidate.element_id, hint.label.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut used = BTreeSet::new();
    let mut labels = vec![String::new(); input.len()];
    for (index, candidate) in input.iter().enumerate() {
        if let Some(label) = old.get(&candidate.element_id)
            && label.len() == width
            && used.insert(label.clone())
        {
            labels[index].clone_from(label);
        }
    }
    let mut next = 0;
    for label in &mut labels {
        if !label.is_empty() {
            continue;
        }
        loop {
            let code = encode(next, width, DEFAULT_ALPHABET.as_bytes())
                .chars()
                .rev()
                .collect::<String>();
            next += 1;
            if used.insert(code.clone()) {
                *label = code;
                break;
            }
        }
    }
    input
        .into_iter()
        .zip(labels)
        .map(|(candidate, label)| LabeledHint { label, candidate })
        .collect()
}

fn refresh_words(
    input: Vec<HintCandidate>,
    previous: &[LabeledHint],
    words: &[String],
) -> Vec<LabeledHint> {
    let old = previous
        .iter()
        .map(|hint| (hint.candidate.element_id, hint.label.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut used = BTreeSet::new();
    let mut labels = vec![String::new(); input.len()];
    for (index, candidate) in input.iter().enumerate() {
        if let Some(label) = old.get(&candidate.element_id)
            && used.insert(label.clone())
        {
            labels[index].clone_from(label);
        }
    }
    let mut next = 0;
    for label in &mut labels {
        if !label.is_empty() {
            continue;
        }
        loop {
            let code = word_label(next, words);
            next += 1;
            if used.insert(code.clone()) {
                *label = code;
                break;
            }
        }
    }
    input
        .into_iter()
        .zip(labels)
        .map(|(candidate, label)| LabeledHint { label, candidate })
        .collect()
}

fn refreshed_labels(
    strategy: Strategy,
    input: Vec<HintCandidate>,
    previous: &[LabeledHint],
    corpus: &Corpus,
) -> Vec<LabeledHint> {
    match strategy {
        Strategy::StableLabels | Strategy::TextSearch => {
            refresh_labels(input, previous, DEFAULT_ALPHABET, 1)
                .expect("production stable label refresh")
        }
        Strategy::Numeric => refresh_labels(input, previous, NUMERIC_ALPHABET, 1)
            .expect("production numeric label refresh"),
        Strategy::Scattered => refresh_scattered(input, previous),
        Strategy::WordPairs => refresh_words(input, previous, &corpus.word_dictionary),
    }
}

fn churn_measurement(
    strategy: Strategy,
    scenario: &str,
    previous: &[LabeledHint],
    refreshed: &[LabeledHint],
) -> ChurnMeasurement {
    let current = refreshed
        .iter()
        .map(|hint| (hint.candidate.element_id, hint.label.as_str()))
        .collect::<BTreeMap<_, _>>();
    let retained = previous
        .iter()
        .filter_map(|hint| {
            current
                .get(&hint.candidate.element_id)
                .map(|label| (hint, *label))
        })
        .collect::<Vec<_>>();
    let changed = retained
        .iter()
        .filter(|(hint, current_label)| hint.label != **current_label)
        .count();
    ChurnMeasurement {
        strategy,
        scenario: scenario.to_owned(),
        previous_candidates: previous.len(),
        refreshed_candidates: refreshed.len(),
        retained_candidates: retained.len(),
        changed_retained_labels: changed,
        churn_fraction: if retained.is_empty() {
            0.0
        } else {
            f64::from(u32::try_from(changed).expect("bounded retained candidate count"))
                / f64::from(
                    u32::try_from(retained.len()).expect("bounded retained candidate count"),
                )
        },
    }
}

fn build_result(corpus: &Corpus, count: usize, strategy: Strategy) -> ScenarioMeasurement {
    let input = candidates(corpus, count);
    let hints = labels_for(strategy, input.clone(), corpus);
    let repeated = labels_for(strategy, input, corpus);
    let deterministic = hints
        .iter()
        .map(|hint| (&hint.candidate.element_id, &hint.label))
        .eq(repeated
            .iter()
            .map(|hint| (&hint.candidate.element_id, &hint.label)));
    let alphabet = input_alphabet(strategy, &corpus.word_dictionary);
    let mut task_measurements = Vec::with_capacity(corpus.task_targets.len());
    let mut gestures = Vec::with_capacity(corpus.task_targets.len());
    for (index, task) in corpus.task_targets.iter().enumerate() {
        let target_id = u32::try_from(index + 1).expect("task ID fits u32");
        let (input_gestures, query_chars, activation_confirmed) =
            if strategy == Strategy::TextSearch {
                let (count, query_chars) = activate_by_text_search(&hints, &task.query, target_id);
                (count, query_chars, true)
            } else {
                (activate_by_label(&hints, &alphabet, target_id), 0, true)
            };
        gestures.push(input_gestures);
        task_measurements.push(TaskMeasurement {
            task_id: task.id.clone(),
            target_index: index,
            query_chars,
            input_gestures,
            assumed_time_ms: u64::try_from(input_gestures)
                .expect("bounded gesture count")
                .saturating_mul(corpus.assumed_ms_per_gesture),
            activation_confirmed,
        });
    }
    let mut median_gestures = gestures;
    let median_input_gestures = median(&mut median_gestures);
    let median_assumed_time_ms = median_input_gestures
        * f64::from(u32::try_from(corpus.assumed_ms_per_gesture).expect("assumed time fits u32"));
    let total_label_characters = hints
        .iter()
        .map(|hint| {
            u32::try_from(hint.label.chars().count()).expect("bounded label length fits u32")
        })
        .sum::<u32>();
    let average_label_characters = f64::from(total_label_characters)
        / f64::from(u32::try_from(hints.len()).expect("candidate count fits u32"));
    let labels_unique = hints
        .iter()
        .map(|hint| hint.label.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        == hints.len();
    let labels_prefix_free = is_prefix_free(&hints);
    let modeled_mismatches = task_measurements
        .iter()
        .filter(|measurement| !measurement.activation_confirmed)
        .count();
    ScenarioMeasurement {
        candidate_count: count,
        strategy,
        task_count: task_measurements.len(),
        median_input_gestures,
        median_assumed_time_ms,
        average_label_characters,
        maximum_adjacent_first_key_run: maximum_first_key_run(&hints),
        labels_unique,
        labels_prefix_free,
        deterministic,
        modeled_mismatches,
        task_measurements,
    }
}

#[test]
#[ignore = "opt-in S25 deterministic label ergonomics model"]
fn s25_evaluate_label_strategies() {
    let corpus = read_corpus();
    let mut results = Vec::new();
    let mut refresh_churn = Vec::new();
    for count in corpus.candidate_counts.iter().copied() {
        for strategy in Strategy::ALL {
            results.push(build_result(&corpus, count, strategy));
            let previous = labels_for(strategy, candidates(&corpus, count), &corpus);
            let mut refreshed_candidates = candidates(&corpus, count)
                .into_iter()
                .filter(|candidate| {
                    candidate.element_id % corpus.mutation.remove_element_id_modulus != 0
                })
                .collect::<Vec<_>>();
            if corpus.mutation.add_replacements {
                let target_count = count;
                while refreshed_candidates.len() < target_count {
                    let index = count + refreshed_candidates.len() - (count - count / 10);
                    refreshed_candidates.push(candidate(
                        index,
                        format!("{} {:05}", corpus.filler_prefix, index),
                    ));
                }
            }
            let refreshed = refreshed_labels(strategy, refreshed_candidates, &previous, &corpus);
            refresh_churn.push(churn_measurement(
                strategy,
                &format!("{count}-10-percent-replacement"),
                &previous,
                &refreshed,
            ));
        }
    }
    let boundary = &corpus.mutation.width_boundary;
    let initial = candidates(&corpus, boundary.from);
    let expanded = candidates(&corpus, boundary.to);
    for strategy in Strategy::ALL {
        let previous = labels_for(strategy, initial.clone(), &corpus);
        let refreshed = refreshed_labels(strategy, expanded.clone(), &previous, &corpus);
        refresh_churn.push(churn_measurement(
            strategy,
            &format!(
                "{}-to-{}-candidate-width-boundary",
                boundary.from, boundary.to
            ),
            &previous,
            &refreshed,
        ));
    }
    assert!(
        results
            .iter()
            .all(|row| { row.labels_unique && row.labels_prefix_free && row.deterministic })
    );
    assert!(results.iter().all(|row| row.modeled_mismatches == 0));
    let evaluation = Evaluation {
        schema_version: 1,
        evidence_class: "deterministic synthetic model; not a human study",
        fixture: "tests/fixtures/webapp-interaction/label-corpus.json",
        assumed_ms_per_gesture: corpus.assumed_ms_per_gesture,
        task_count: corpus.task_targets.len(),
        candidate_counts: corpus.candidate_counts,
        results,
        refresh_churn,
        human_error_rate: "not measured; model mismatches are not user errors",
        limitations: vec![
            "The synthetic task names and full-name search queries are fixed and deliberately unambiguous.",
            "Modeled time multiplies keyboard gestures by a fixed assumption; it is not observed completion time.",
            "The word-pair prototype uses a fixed local dictionary and an in-memory alphabet; production configuration and UI support are not implemented.",
            "First-key run length is a distribution descriptor, not a readability or usability score.",
            "No page-derived values, labels, passwords, or field contents are used by the word prototype.",
        ],
    };
    println!(
        "S25_LABEL_EVALUATION_JSON {}",
        serde_json::to_string(&evaluation).expect("evaluation serializes")
    );
}
