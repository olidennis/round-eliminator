//! Exact duplicate history with optional dictionary/varint storage.
//! Compression interns whole values, not labels, preserving custom algebras and
//! the Ord-only contract. Keys compare by equality, never by a fingerprint.

use crate::model::Configuration;
use std::collections::{BTreeMap, BTreeSet};

pub(super) enum History<T> {
    Plain(BTreeSet<Configuration<T>>),
    Compressed {
        values: BTreeMap<T, usize>,
        keys: BTreeSet<Vec<u8>>,
    },
}

struct Key {
    inline: [u8; 128],
    len: usize,
    overflow: Option<Vec<u8>>,
}
impl Key {
    fn new() -> Self {
        Self {
            inline: [0; 128],
            len: 0,
            overflow: None,
        }
    }
    fn push(&mut self, byte: u8) {
        if let Some(bytes) = &mut self.overflow {
            bytes.push(byte);
        } else if self.len < self.inline.len() {
            self.inline[self.len] = byte;
        } else {
            let mut bytes = self.inline.to_vec();
            bytes.push(byte);
            self.overflow = Some(bytes);
        }
        self.len += 1;
    }
    fn as_slice(&self) -> &[u8] {
        self.overflow
            .as_deref()
            .unwrap_or(&self.inline[..self.len.min(128)])
    }
    fn into_vec(self) -> Vec<u8> {
        self.overflow
            .unwrap_or_else(|| self.inline[..self.len].to_vec())
    }
}

/// Prefix-free unsigned varints. Alternating value IDs/counts until the key's
/// end encode exactly one canonical configuration, including the empty one.
fn integer(mut n: usize, key: &mut Key) {
    while n >= 128 {
        key.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    key.push(n as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    // An ordered value with neither Hash nor Serialize, like a custom algebra.
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Value(Vec<u16>);

    #[test]
    fn compressed_history_matches_full_keys_across_ids_counts_and_long_keys() {
        let mut plain = History::new(false);
        let mut compact = History::new(true);
        let mut rows = vec![Configuration::default()];
        for i in 0..260 {
            rows.push(Configuration::new(vec![(Value(vec![i, i + 1]), 1)]).unwrap());
        }
        for count in [1, 127, 128, 16383, 16384, u32::MAX] {
            rows.push(Configuration::new(vec![(Value(vec![128, 129]), count)]).unwrap());
        }
        // More than the inline key's 128 bytes, with noncontiguous dictionary
        // IDs and changing counts. Overflow uses owned storage, not a cutoff.
        for count in [1, 128] {
            rows.push(
                Configuration::new((0..200).map(|i| (Value(vec![i, i + 1]), count)).collect())
                    .unwrap(),
            );
        }
        for row in &rows {
            assert_eq!(compact.contains(row), plain.contains(row));
            assert_eq!(compact.insert(row.clone()), plain.insert(row.clone()));
            assert!(compact.contains(row));
            assert!(!compact.insert(row.clone()));
        }
        for row in rows.iter().rev() {
            assert_eq!(compact.contains(row), plain.contains(row));
        }
        let unknown_value = Configuration::new(vec![(Value(vec![1000]), 1)]).unwrap();
        let unknown_combination =
            Configuration::new(vec![(Value(vec![0, 1]), 1), (Value(vec![128, 129]), 127)]).unwrap();
        assert!(!compact.contains(&unknown_value));
        assert!(!compact.contains(&unknown_combination));
        assert_eq!(compact.len(), plain.len());
    }
}

impl<T: Clone + Ord> History<T> {
    pub(super) fn new(compressed: bool) -> Self {
        if compressed {
            Self::Compressed {
                values: BTreeMap::new(),
                keys: BTreeSet::new(),
            }
        } else {
            Self::Plain(BTreeSet::new())
        }
    }

    pub(super) fn contains(&self, configuration: &Configuration<T>) -> bool {
        match self {
            Self::Plain(set) => set.contains(configuration),
            Self::Compressed { values, keys } => {
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let Some(&id) = values.get(value) else {
                        return false;
                    };
                    integer(id, &mut key);
                    integer(*count as usize, &mut key);
                }
                keys.contains(key.as_slice())
            }
        }
    }

    pub(super) fn insert(&mut self, configuration: Configuration<T>) -> bool {
        match self {
            Self::Plain(set) => set.insert(configuration),
            Self::Compressed { values, keys } => {
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

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Plain(set) => set.len(),
            Self::Compressed { keys, .. } => keys.len(),
        }
    }
}
