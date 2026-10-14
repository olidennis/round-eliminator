//! LLM: Exact duplicate history with optional dictionary/varint storage.
//! LLM: Compression interns whole values, not labels, preserving custom algebras and
//! LLM: the Ord-only contract. Keys compare by equality, never by a fingerprint.

use crate::model::Configuration;
use std::collections::{BTreeMap, BTreeSet};

/// LLM: Exact duplicate history for generated configurations. Plain storage keeps complete
/// LLM: counted rows; compressed storage interns whole algebra values and stores prefix-free
/// LLM: value-ID/count keys, with no probabilistic fingerprints.
pub(super) enum History<T> {
    Plain(BTreeSet<Configuration<T>>),
    Compressed {
        values: BTreeMap<T, usize>,
        keys: BTreeSet<Vec<u8>>,
    },
}

/// LLM: A temporary encoded history key with a small inline byte buffer and owned overflow
/// LLM: storage. Its bytes encode the full sequence of interned values and counts; overflow
/// LLM: changes storage, not the accepted key length.
struct Key {
    inline: [u8; 128],
    len: usize,
    overflow: Option<Vec<u8>>,
}
impl Key {
    /// LLM: Create an empty inline history key. Used while encoding one counted row for lookup
    /// LLM: or insertion.
    fn new() -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            inline: [0; 128],
            len: 0,
            overflow: None,
        }
    }
    /// LLM: Append one encoded byte, allocating overflow storage only when needed. Used by
    /// LLM: prefix-free integer encoding without a key-length cutoff.
    fn push(&mut self, byte: u8) {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if let Some(bytes) = &mut self.overflow {
            bytes.push(byte);
        } else if self.len < self.inline.len() {
            self.inline[self.len] = byte;
        } else {
            let mut bytes = self.inline.to_vec();
            bytes.push(byte);
            self.overflow = Some(bytes);
        }
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.len += 1;
    }
    /// LLM: Borrow all encoded bytes from inline or overflow storage. Used for history lookup
    /// LLM: without another allocation.
    fn as_slice(&self) -> &[u8] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.overflow
            .as_deref()
            .unwrap_or(&self.inline[..self.len.min(128)])
    }
    /// LLM: Return an owned encoded key, reusing overflow storage when present. Used when
    /// LLM: inserting a compressed history entry.
    fn into_vec(self) -> Vec<u8> {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.overflow
            .unwrap_or_else(|| self.inline[..self.len].to_vec())
    }
}

/// LLM: Append one unsigned integer as a prefix-free varint to a history key. Used alternately
/// LLM: for interned value IDs and counts so each key identifies exactly one counted row.
fn integer(mut n: usize, key: &mut Key) {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    while n >= 128 {
        key.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    key.push(n as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    // LLM: An ordered value with neither Hash nor Serialize, like a custom algebra.
    /// LLM: An ordered test value without Hash or Serialize implementations, used to verify
    /// LLM: that compressed history supports the generic Ord-only algebra contract.
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Value(Vec<u16>);

    /// LLM: Regression test: verify that compressed history matches full keys across ids
    /// LLM: counts and long keys. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn compressed_history_matches_full_keys_across_ids_counts_and_long_keys() {
        // LLM: Prepare `plain` for the following computation; it stores the plain for this
        // LLM: stage.
        let mut plain = History::new(false);
        let mut compact = History::new(true);
        let mut rows = vec![Configuration::default()];
        for i in 0..260 {
            rows.push(Configuration::new(vec![(Value(vec![i, i + 1]), 1)]).unwrap());
        }
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for count in [1, 127, 128, 16383, 16384, u32::MAX] {
            rows.push(Configuration::new(vec![(Value(vec![128, 129]), count)]).unwrap());
        }
        // LLM: More than the inline key's 128 bytes, with noncontiguous dictionary
        // LLM: IDs and changing counts. Overflow uses owned storage, not a cutoff.

        for count in [1, 128] {
            rows.push(
                Configuration::new((0..200).map(|i| (Value(vec![i, i + 1]), count)).collect())
                    .unwrap(),
            );
        }
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for row in &rows {
            assert_eq!(compact.contains(row), plain.contains(row));
            assert_eq!(compact.insert(row.clone()), plain.insert(row.clone()));
            assert!(compact.contains(row));
            assert!(!compact.insert(row.clone()));
        }
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for row in rows.iter().rev() {
            assert_eq!(compact.contains(row), plain.contains(row));
        }
        let unknown_value = Configuration::new(vec![(Value(vec![1000]), 1)]).unwrap();
        let unknown_combination =
            Configuration::new(vec![(Value(vec![0, 1]), 1), (Value(vec![128, 129]), 127)]).unwrap();
        assert!(!compact.contains(&unknown_value));
        assert!(!compact.contains(&unknown_combination));
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(compact.len(), plain.len());
    }
}

impl<T: Clone + Ord> History<T> {
    /// LLM: Create empty exact history with plain rows or interned compressed keys. Used once
    /// LLM: per independent degree in maximization.
    pub(super) fn new(compressed: bool) -> Self {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if compressed {
            Self::Compressed {
                values: BTreeMap::new(),
                keys: BTreeSet::new(),
            }
        } else {
            Self::Plain(BTreeSet::new())
        }
    }

    /// LLM: Test exact generated-row membership without modifying history or interning unknown
    /// LLM: values. Used concurrently to filter candidates before ordered insertion.
    pub(super) fn contains(&self, configuration: &Configuration<T>) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self {
            Self::Plain(set) => set.contains(configuration),
            Self::Compressed { values, keys } => {
                // LLM: Encode the complete counted row for exact history lookup.
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let Some(&id) = values.get(value) else {
                        return false;
                    };
                    integer(id, &mut key);
                    integer(*count as usize, &mut key);
                }
                // LLM: Update the current working state before continuing this operation.
                keys.contains(key.as_slice())
            }
        }
    }

    /// LLM: Record a complete generated row and return whether it is new. Used by the closure
    /// LLM: to avoid repeating work even for locally rejected rows.
    pub(super) fn insert(&mut self, configuration: Configuration<T>) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self {
            Self::Plain(set) => set.insert(configuration),
            Self::Compressed { values, keys } => {
                // LLM: Encode the complete counted row for exact history lookup.
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let next = values.len();
                    let id = *values.entry(value.clone()).or_insert(next);
                    integer(id, &mut key);
                    integer(*count as usize, &mut key);
                }
                keys.insert(key.into_vec())
            }
        }
    }

    /// LLM: Return the number of stored entries from History. Used by the exact maximization
    /// LLM: engine without exposing mutable storage.
    pub(super) fn len(&self) -> usize {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self {
            Self::Plain(set) => set.len(),
            Self::Compressed { keys, .. } => keys.len(),
        }
    }
}
