//! Checked label strings and bounded allocation, without DOM or action authority.
use crate::{
    HintAutoFollow, MAX_HINT_ALPHABET_CHARS, MAX_HINT_CANDIDATES, MIN_HINT_ALPHABET_CHARS,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_HINT_CODE_BYTES: usize = 8;
pub const MAX_HINT_SPECIAL_RESERVATIONS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HintReservationCategory {
    Anchor,
    Collection,
}

/// One immutable declared slot, already projected onto one invocation family.
/// A missing target does not remove this reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintCodeDeclaration {
    pub category: HintReservationCategory,
    pub rule_id: String,
    pub slot: usize,
    pub code: String,
}

/// Effective browser-owned label settings. Only admitted Hint source characters
/// belong in `page_source_chars`; remap destinations are not exclusions.
/// Unicode page sources are legal and cannot overlap the ASCII label namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintCodebookOptions {
    pub alphabet: String,
    pub min_chars: u8,
    pub fallback_prefix: String,
    pub excluded_chars: BTreeSet<char>,
    pub page_source_chars: BTreeSet<char>,
    pub additional_controls: BTreeSet<char>,
    pub auto_follow: HintAutoFollow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintCodeExclusion {
    Control,
    Configured,
    PageSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HintCodebookError {
    TooManyReservations {
        count: usize,
    },
    InvalidReservation {
        index: usize,
    },
    DuplicateSlot {
        first: usize,
        second: usize,
    },
    InvalidCode {
        index: usize,
    },
    ExcludedCodeCharacter {
        index: usize,
        character: char,
        reason: HintCodeExclusion,
    },
    PrefixConflict {
        first: usize,
        second: usize,
    },
    InvalidConfiguredAlphabet,
    InvalidEffectiveAlphabet {
        count: usize,
    },
    InvalidExclusion,
    InvalidMinimumLength,
    InvalidFallbackPrefix,
    ExcludedPrefixCharacter {
        character: char,
        reason: HintCodeExclusion,
    },
    ConflictingAutoFollow,
    TooManyOrdinaryCandidates {
        count: usize,
    },
    InsufficientCapacity {
        requested: usize,
        available: u64,
    },
}

impl std::fmt::Display for HintCodebookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyReservations { count } => write!(
                f,
                "Hint codebook has {count} reservations; maximum is {MAX_HINT_SPECIAL_RESERVATIONS}"
            ),
            Self::InvalidReservation { index } => {
                write!(f, "Hint reservation {index} has an invalid rule identity")
            }
            Self::DuplicateSlot { first, second } => write!(
                f,
                "Hint reservations {first} and {second} name the same slot"
            ),
            Self::InvalidCode { index } => write!(
                f,
                "Hint reservation {index} requires 1–8 printable ASCII characters"
            ),
            Self::ExcludedCodeCharacter {
                index,
                character,
                reason,
            } => write!(
                f,
                "Hint reservation {index} contains excluded character {character:?} ({reason:?})"
            ),
            Self::PrefixConflict { first, second } => write!(
                f,
                "Hint reservations {first} and {second} overlap by prefix"
            ),
            Self::InvalidConfiguredAlphabet => {
                f.write_str("Hint alphabet requires unique printable ASCII characters")
            }
            Self::InvalidEffectiveAlphabet { count } => write!(
                f,
                "Filtered Hint alphabet has {count} characters; requires 2–32"
            ),
            Self::InvalidExclusion => {
                f.write_str("Hint exclusions require supported ASCII characters")
            }
            Self::InvalidMinimumLength => {
                f.write_str("Hint minimum total label length must be 1–8")
            }
            Self::InvalidFallbackPrefix => f.write_str(
                "Hint fallback prefix requires at most seven printable ASCII characters",
            ),
            Self::ExcludedPrefixCharacter { character, reason } => write!(
                f,
                "Hint fallback prefix contains excluded character {character:?} ({reason:?})"
            ),
            Self::ConflictingAutoFollow => f.write_str(
                "Enhanced hints require FullMatch or Never; change the auto-follow setting",
            ),
            Self::TooManyOrdinaryCandidates { count } => write!(
                f,
                "Hint codebook has {count} ordinary candidates; maximum is {MAX_HINT_CANDIDATES}"
            ),
            Self::InsufficientCapacity {
                requested,
                available,
            } => write!(
                f,
                "Hint namespace has capacity {available} for {requested} ordinary targets; change the alphabet, prefix or reservations and start a new session"
            ),
        }
    }
}

impl std::error::Error for HintCodebookError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TrieNode {
    children: BTreeMap<u8, usize>,
    terminal: Option<usize>,
    first_descendant: Option<usize>,
    capacity: [u64; MAX_HINT_CODE_BYTES + 1],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReservationTrie {
    nodes: Vec<TrieNode>,
}

impl ReservationTrie {
    fn new() -> Self {
        Self {
            nodes: vec![TrieNode::default()],
        }
    }

    fn insert(&mut self, code: &str, reservation: usize) -> Result<(), HintCodebookError> {
        let mut index = 0;
        let mut path = vec![0];
        for byte in code.bytes() {
            if let Some(first) = self.nodes[index].terminal {
                return Err(HintCodebookError::PrefixConflict {
                    first,
                    second: reservation,
                });
            }
            let child = if let Some(child) = self.nodes[index].children.get(&byte) {
                *child
            } else {
                let next = self.nodes.len();
                self.nodes.push(TrieNode::default());
                self.nodes[index].children.insert(byte, next);
                next
            };
            index = child;
            path.push(index);
        }
        if let Some(first) = self.nodes[index].first_descendant {
            return Err(HintCodebookError::PrefixConflict {
                first,
                second: reservation,
            });
        }
        self.nodes[index].terminal = Some(reservation);
        for index in path {
            self.nodes[index]
                .first_descendant
                .get_or_insert(reservation);
        }
        Ok(())
    }

    fn prefix(&self, prefix: &str) -> (Option<usize>, bool) {
        let mut current = Some(0);
        for byte in prefix.bytes() {
            let Some(index) = current else {
                return (None, false);
            };
            if self.nodes[index].terminal.is_some() {
                return (current, true);
            }
            current = self.nodes[index].children.get(&byte).copied();
        }
        (
            current,
            current.is_some_and(|index| self.nodes[index].terminal.is_some()),
        )
    }

    fn count_subtrees(&mut self, alphabet: &[u8]) {
        for index in (0..self.nodes.len()).rev() {
            let mut counts = [0; MAX_HINT_CODE_BYTES + 1];
            if self.nodes[index].terminal.is_none() {
                counts[0] = u64::from(self.nodes[index].first_descendant.is_none());
                for (remaining, count) in counts.iter_mut().enumerate().skip(1) {
                    *count = alphabet.iter().fold(0_u64, |sum, byte| {
                        let branch = self.nodes[index].children.get(byte).map_or_else(
                            || {
                                (alphabet.len() as u64).saturating_pow(
                                    u32::try_from(remaining - 1).expect("label bound is eight"),
                                )
                            },
                            |child| self.nodes[*child].capacity[remaining - 1],
                        );
                        sum.saturating_add(branch)
                    });
                }
            }
            self.nodes[index].capacity = counts;
        }
    }

    fn capacity(&self, node: Option<usize>, remaining: u8, alphabet: &[u8]) -> u64 {
        let Some(index) = node else {
            return (alphabet.len() as u64).saturating_pow(u32::from(remaining));
        };
        self.nodes[index].capacity[usize::from(remaining)]
    }

    fn select(
        &self,
        mut node: Option<usize>,
        mut remaining: u8,
        alphabet: &[u8],
        mut rank: u64,
        output: &mut String,
    ) {
        while remaining > 0 {
            for byte in alphabet {
                let child = node.and_then(|index| self.nodes[index].children.get(byte).copied());
                let capacity = self.capacity(child, remaining - 1, alphabet);
                if rank < capacity {
                    output.push(char::from(*byte));
                    node = child;
                    break;
                }
                rank -= capacity;
            }
            remaining -= 1;
        }
    }

    fn rank(&self, mut node: Option<usize>, alphabet: &[u8], word: &[u8]) -> Option<u64> {
        let mut rank = 0_u64;
        for (offset, byte) in word.iter().enumerate() {
            let remaining = u8::try_from(word.len() - offset - 1).ok()?;
            let mut selected = None;
            for symbol in alphabet {
                let child = node.and_then(|index| self.nodes[index].children.get(symbol).copied());
                let capacity = self.capacity(child, remaining, alphabet);
                if symbol == byte {
                    if capacity == 0 {
                        return None;
                    }
                    selected = Some(child);
                    break;
                }
                rank = rank.checked_add(capacity)?;
            }
            node = selected?;
        }
        Some(rank)
    }
}

/// Checked before discovery. This validates names, not target ownership or DOM
/// proof. Construction reserves absent/ambiguous slots just like live ones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedHintCodebook {
    options: HintCodebookOptions,
    effective_alphabet: String,
    declarations: Vec<HintCodeDeclaration>,
    trie: ReservationTrie,
}

impl CheckedHintCodebook {
    /// # Errors
    /// Rejects invalid settings, excluded characters and every duplicate or
    /// prefix collision in the projected reservation set.
    pub fn compile(
        options: HintCodebookOptions,
        declarations: Vec<HintCodeDeclaration>,
    ) -> Result<Self, HintCodebookError> {
        if declarations.len() > MAX_HINT_SPECIAL_RESERVATIONS {
            return Err(HintCodebookError::TooManyReservations {
                count: declarations.len(),
            });
        }
        let effective_alphabet = validate_options(&options)?;
        let excluded = |c| exclusion(&options, c);
        let mut slots = BTreeMap::new();
        let mut trie = ReservationTrie::new();
        for (index, declaration) in declarations.iter().enumerate() {
            if declaration.rule_id.is_empty()
                || declaration.rule_id.len() > 64
                || !declaration
                    .rule_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
            {
                return Err(HintCodebookError::InvalidReservation { index });
            }
            if let Some(first) = slots.insert(
                (declaration.category, &declaration.rule_id, declaration.slot),
                index,
            ) {
                return Err(HintCodebookError::DuplicateSlot {
                    first,
                    second: index,
                });
            }
            if declaration.code.is_empty()
                || declaration.code.len() > MAX_HINT_CODE_BYTES
                || !declaration.code.bytes().all(|b| b.is_ascii_graphic())
            {
                return Err(HintCodebookError::InvalidCode { index });
            }
            for character in declaration.code.chars() {
                if let Some(reason) = excluded(character) {
                    return Err(HintCodebookError::ExcludedCodeCharacter {
                        index,
                        character,
                        reason,
                    });
                }
            }
            trie.insert(&declaration.code, index)?;
        }
        trie.count_subtrees(effective_alphabet.as_bytes());
        Ok(Self {
            options,
            effective_alphabet,
            declarations,
            trie,
        })
    }

    #[must_use]
    pub fn declarations(&self) -> &[HintCodeDeclaration] {
        &self.declarations
    }
    #[must_use]
    pub fn configured_alphabet(&self) -> &str {
        &self.options.alphabet
    }
    #[must_use]
    pub fn effective_alphabet(&self) -> &str {
        &self.effective_alphabet
    }
    #[must_use]
    pub const fn auto_follow(&self) -> HintAutoFollow {
        self.options.auto_follow
    }
    #[must_use]
    pub fn exclusion(&self, character: char) -> Option<HintCodeExclusion> {
        exclusion(&self.options, character)
    }

    /// Number of usable fallback words at this suffix width. Trie branches are
    /// counted, never enumerated; saturation cannot wrap into false capacity.
    #[must_use]
    pub fn capacity(&self, suffix_width: u8) -> u64 {
        if suffix_width == 0
            || self.options.fallback_prefix.len() + usize::from(suffix_width) > MAX_HINT_CODE_BYTES
        {
            return 0;
        }
        let (node, blocked) = self.trie.prefix(&self.options.fallback_prefix);
        if blocked {
            return 0;
        }
        self.trie
            .capacity(node, suffix_width, self.effective_alphabet.as_bytes())
    }

    /// Fixes ordinary width after special ownership is known for the initial
    /// snapshot. Even zero ordinary rows retain the declared reservations.
    /// # Errors
    /// Rejects excessive candidates or initial capacity exhaustion at all
    /// allowed widths, without changing authored settings or special codes.
    pub fn allocator(self, ordinary_count: usize) -> Result<HintLabelAllocator, HintCodebookError> {
        if ordinary_count > MAX_HINT_CANDIDATES {
            return Err(HintCodebookError::TooManyOrdinaryCandidates {
                count: ordinary_count,
            });
        }
        let prefix = u8::try_from(self.options.fallback_prefix.len())
            .map_err(|_| HintCodebookError::InvalidFallbackPrefix)?;
        let minimum = self.options.min_chars.saturating_sub(prefix).max(1);
        let maximum = u8::try_from(MAX_HINT_CODE_BYTES - usize::from(prefix))
            .map_err(|_| HintCodebookError::InvalidFallbackPrefix)?;
        let mut available = 0;
        for width in minimum..=maximum {
            available = self.capacity(width);
            if available >= ordinary_count as u64 {
                return Ok(HintLabelAllocator {
                    codebook: self,
                    suffix_width: width,
                    capacity: available,
                    next: 0,
                });
            }
        }
        Err(HintCodebookError::InsufficientCapacity {
            requested: ordinary_count,
            available,
        })
    }
}

fn validate_options(options: &HintCodebookOptions) -> Result<String, HintCodebookError> {
    if !(1..=MAX_HINT_CODE_BYTES).contains(&usize::from(options.min_chars)) {
        return Err(HintCodebookError::InvalidMinimumLength);
    }
    if !matches!(
        options.auto_follow,
        HintAutoFollow::FullMatch | HintAutoFollow::Never
    ) {
        return Err(HintCodebookError::ConflictingAutoFollow);
    }
    if options.excluded_chars.len() > 32
        || options.page_source_chars.len() > 32
        || options
            .excluded_chars
            .iter()
            .any(|c| !c.is_ascii_graphic() && *c != ' ')
        || options.additional_controls.iter().any(|c| !c.is_ascii())
    {
        return Err(HintCodebookError::InvalidExclusion);
    }
    let mut seen = BTreeSet::new();
    if options.alphabet.is_empty()
        || !options
            .alphabet
            .bytes()
            .all(|b| (b.is_ascii_graphic() || b == b' ') && seen.insert(b))
    {
        return Err(HintCodebookError::InvalidConfiguredAlphabet);
    }
    let excluded = |c| exclusion(options, c);
    let effective_alphabet: String = options
        .alphabet
        .chars()
        .filter(|c| excluded(*c).is_none())
        .collect();
    if !(MIN_HINT_ALPHABET_CHARS..=MAX_HINT_ALPHABET_CHARS).contains(&effective_alphabet.len()) {
        return Err(HintCodebookError::InvalidEffectiveAlphabet {
            count: effective_alphabet.len(),
        });
    }
    if options.fallback_prefix.len() >= MAX_HINT_CODE_BYTES
        || !options
            .fallback_prefix
            .bytes()
            .all(|b| b.is_ascii_graphic())
    {
        return Err(HintCodebookError::InvalidFallbackPrefix);
    }
    for character in options.fallback_prefix.chars() {
        if let Some(reason) = excluded(character) {
            return Err(HintCodebookError::ExcludedPrefixCharacter { character, reason });
        }
    }
    Ok(effective_alphabet)
}

fn exclusion(options: &HintCodebookOptions, character: char) -> Option<HintCodeExclusion> {
    if !character.is_ascii_graphic()
        || character == '/'
        || options.additional_controls.contains(&character)
    {
        Some(HintCodeExclusion::Control)
    } else if options.excluded_chars.contains(&character) {
        Some(HintCodeExclusion::Configured)
    } else if options.page_source_chars.contains(&character) {
        Some(HintCodeExclusion::PageSource)
    } else {
        None
    }
}

/// Monotonic allocation cursor. It never revisits consumed or retired words,
/// never changes width, and needs no history proportional to skipped subtrees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintLabelAllocator {
    codebook: CheckedHintCodebook,
    suffix_width: u8,
    capacity: u64,
    next: u64,
}

impl HintLabelAllocator {
    #[must_use]
    pub const fn codebook(&self) -> &CheckedHintCodebook {
        &self.codebook
    }
    #[must_use]
    pub const fn suffix_width(&self) -> u8 {
        self.suffix_width
    }
    #[must_use]
    pub const fn capacity(&self) -> u64 {
        self.capacity
    }
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.capacity - self.next
    }

    pub(crate) fn pending_label(&self, offset: u64) -> Option<String> {
        let rank = self.next.checked_add(offset)?;
        if rank >= self.capacity {
            return None;
        }
        let mut output = self.codebook.options.fallback_prefix.clone();
        let (node, blocked) = self.codebook.trie.prefix(&output);
        debug_assert!(!blocked);
        self.codebook.trie.select(
            node,
            self.suffix_width,
            self.codebook.effective_alphabet.as_bytes(),
            rank,
            &mut output,
        );
        Some(output)
    }

    pub(crate) fn commit_labels(&mut self, count: u64) {
        debug_assert!(count <= self.remaining());
        self.next += count;
    }

    /// Reports whether an ordinary word's rank is behind this allocator's
    /// cursor, including ranks deliberately skipped by `Iterator::nth`.
    /// This is namespace history, never evidence of target/action ownership.
    #[must_use]
    pub fn has_allocated(&self, label: &str) -> bool {
        let prefix = &self.codebook.options.fallback_prefix;
        let Some(suffix) = label.strip_prefix(prefix) else {
            return false;
        };
        if suffix.len() != usize::from(self.suffix_width) {
            return false;
        }
        let (node, blocked) = self.codebook.trie.prefix(prefix);
        !blocked
            && self
                .codebook
                .trie
                .rank(
                    node,
                    self.codebook.effective_alphabet.as_bytes(),
                    suffix.as_bytes(),
                )
                .is_some_and(|rank| rank < self.next)
    }
}

impl Iterator for HintLabelAllocator {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        let output = self.pending_label(0)?;
        self.commit_labels(1);
        Some(output)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let skip = u64::try_from(n).unwrap_or(u64::MAX);
        if skip >= self.remaining() {
            self.next = self.capacity;
            return None;
        }
        self.next += skip;
        self.next()
    }
}

impl std::iter::FusedIterator for HintLabelAllocator {}
