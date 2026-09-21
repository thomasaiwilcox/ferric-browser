//! Shared, deterministic matching primitives for the universal switcher.

/// The maximum number of terms retained from one switcher query.
pub const MAX_SWITCHER_QUERY_TERMS: usize = 32;

/// The maximum size of one normalized switcher term in bytes.
pub const MAX_SWITCHER_TERM_BYTES: usize = 128;

/// The maximum amount of one candidate field inspected by fuzzy matching.
///
/// Switcher fields are already bounded at their source boundaries, but this
/// cap keeps the fallback matcher safe if a new source supplies a larger
/// display field.
const MAX_FUZZY_FIELD_CHARS: usize = 4_096;

/// Tokenizes a plain-text query using Unicode-aware lowercasing.
///
/// Query parsing deliberately has no operators or secondary query language.
/// Excess terms are discarded after the bounded prefix so callers cannot
/// create unbounded matching work.
#[must_use]
pub fn tokenize_query(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(str::to_lowercase)
        .take(MAX_SWITCHER_QUERY_TERMS)
        .collect()
}

/// Returns the deterministic match class for a switcher candidate.
///
/// `query` is expected to contain normalized terms from [`tokenize_query`].
/// The candidate is rejected when every term is not present in its ID, label,
/// or match fields.
#[must_use]
pub fn rank(query: &[String], id: &str, label: &str, fields: &[String]) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    if query
        .iter()
        .any(|term| term.len() > MAX_SWITCHER_TERM_BYTES)
    {
        return None;
    }
    let normalized_id = id.to_lowercase();
    let normalized_label = label.to_lowercase();
    let normalized_fields = fields
        .iter()
        .map(|field| field.to_lowercase())
        .collect::<Vec<_>>();
    let all_terms_match = query.iter().all(|term| {
        normalized_fields.iter().any(|field| field.contains(term))
            || normalized_id.contains(term)
            || normalized_label.contains(term)
    });
    if query.len() == 1 && normalized_id == query[0] {
        return Some(1_000);
    }
    if query.len() == 1 && normalized_label == query[0] {
        return Some(950);
    }
    if query.len() == 1 && normalized_label.starts_with(&query[0]) {
        return Some(900);
    }
    if query
        .iter()
        .all(|term| normalized_label.split_whitespace().any(|word| word == term))
    {
        return Some(850);
    }
    if all_terms_match {
        return Some(700);
    }

    // Keep the fuzzy tier below every contiguous match. Matching is a
    // Unicode-aware subsequence search with a bounded scan; compact matches
    // near the beginning of a field rank above scattered matches. This is a
    // deliberately small fallback, not an unbounded edit-distance search.
    let searchable = std::iter::once(normalized_id.as_str())
        .chain(std::iter::once(normalized_label.as_str()))
        .chain(normalized_fields.iter().map(String::as_str));
    let mut fuzzy_score = 0_i64;
    for term in query {
        let best = searchable
            .clone()
            .filter_map(|field| fuzzy_subsequence_score(term, field))
            .max()?;
        fuzzy_score += best;
    }
    Some(500 + fuzzy_score / i64::try_from(query.len()).unwrap_or(1))
}

fn fuzzy_subsequence_score(term: &str, field: &str) -> Option<i64> {
    let term_chars = term.chars().collect::<Vec<_>>();
    if term_chars.is_empty() {
        return Some(0);
    }
    let field_chars = field
        .chars()
        .take(MAX_FUZZY_FIELD_CHARS)
        .collect::<Vec<_>>();
    let mut matched = 0;
    let mut first = None;
    let mut previous = None;
    let mut gaps = 0_usize;
    let mut boundary_bonus = 0_i64;
    for (index, character) in field_chars.iter().copied().enumerate() {
        if character != term_chars[matched] {
            continue;
        }
        if first.is_none() {
            first = Some(index);
        }
        if index == 0 || !field_chars[index - 1].is_alphanumeric() {
            boundary_bonus = boundary_bonus.saturating_add(8);
        }
        if let Some(previous) = previous {
            gaps = gaps.saturating_add(index.saturating_sub(previous + 1));
        }
        previous = Some(index);
        matched += 1;
        if matched == term_chars.len() {
            let penalty = first.unwrap_or_default().saturating_add(gaps).min(99);
            let base = 100 - i64::try_from(penalty).unwrap_or(99);
            return Some(base.saturating_add(boundary_bonus).min(100));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{MAX_SWITCHER_QUERY_TERMS, fuzzy_subsequence_score, rank, tokenize_query};

    #[test]
    fn tokenization_is_unicode_aware_and_bounded() {
        let query = tokenize_query("CAFÉ Straße 東京");
        assert_eq!(query, ["café", "straße", "東京"]);

        let many_terms = (0..MAX_SWITCHER_QUERY_TERMS + 4)
            .map(|index| format!("term{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(tokenize_query(&many_terms).len(), MAX_SWITCHER_QUERY_TERMS);
    }

    #[test]
    fn ranking_is_deterministic_for_unicode_labels() {
        let query = tokenize_query("café");
        assert_eq!(rank(&query, "entry-1", "CAFÉ guide", &[]), Some(900));
        assert_eq!(
            rank(&query, "entry-1", "Other", &["Café".into()]),
            Some(700)
        );
        assert_eq!(rank(&query, "entry-1", "Other", &["茶".into()]), None);
    }

    #[test]
    fn ranking_has_a_bounded_fuzzy_fallback_below_contiguous_matches() {
        let fuzzy = tokenize_query("brwsr");
        let contiguous = tokenize_query("browser");
        let fuzzy_rank = rank(&fuzzy, "entry-1", "Ferric Browser", &[]).expect("fuzzy match");
        let contiguous_rank =
            rank(&contiguous, "entry-1", "Ferric Browser", &[]).expect("contiguous match");
        assert!(fuzzy_rank >= 500);
        assert!(fuzzy_rank < 700);
        assert!(contiguous_rank >= 700);
        assert_eq!(
            rank(&tokenize_query("xyz"), "entry-1", "Ferric Browser", &[]),
            None
        );
    }

    #[test]
    fn fuzzy_word_boundary_matches_are_at_least_as_good_as_scattered_matches() {
        let boundary =
            fuzzy_subsequence_score("fb", "xferric browser").expect("boundary fuzzy match");
        let scattered =
            fuzzy_subsequence_score("rb", "xferric_browser").expect("scattered fuzzy match");
        assert!(boundary >= scattered);
        assert!(boundary <= 100);
    }
}
