//! LLM: Benchmark-only exact dictionary/varint compression of configuration keys.

use crate::model::Configuration;
use std::collections::{BTreeMap, BTreeSet};

/// LLM: Exact duplicate history for generated configurations. Plain storage keeps complete
/// LLM: counted rows; compressed storage interns whole algebra values and stores prefix-free
/// LLM: value-ID/count keys, with no probabilistic fingerprints.
pub(crate) enum History<T> {
    Plain(BTreeSet<Configuration<T>>),
    Compressed { values: BTreeMap<T, usize>, keys: BTreeSet<Vec<u8>> },
}

/// LLM: A temporary encoded history key with a small inline byte buffer and owned overflow
/// LLM: storage. Its bytes encode the full sequence of interned values and counts; overflow
/// LLM: changes storage, not the accepted key length.
struct Key { inline: [u8; 128], len: usize, overflow: Option<Vec<u8>> }
impl Key {
    /// LLM: Create an empty inline history key. Used while encoding one counted row for lookup
    /// LLM: or insertion.
    fn new() -> Self {
        // LLM: Create an empty inline history key.
        Self { inline: [0;128], len: 0, overflow: None } }
    /// LLM: Append one encoded byte, allocating overflow storage only when needed. Used by
    /// LLM: prefix-free integer encoding without a key-length cutoff.
    fn push(&mut self, byte: u8) {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if let Some(bytes) = &mut self.overflow { bytes.push(byte); }
        else if self.len < self.inline.len() { self.inline[self.len] = byte; }
        else { let mut bytes = self.inline.to_vec(); bytes.push(byte); self.overflow = Some(bytes); }
        self.len += 1;
    }
    /// LLM: Borrow all encoded bytes from inline or overflow storage. Used for history lookup
    /// LLM: without another allocation.
    fn as_slice(&self) -> &[u8] {
        // LLM: Borrow all encoded bytes from inline or overflow storage.
        self.overflow.as_deref().unwrap_or(&self.inline[..self.len.min(128)]) }
    /// LLM: Return an owned encoded key, reusing overflow storage when present. Used when
    /// LLM: inserting a compressed history entry.
    fn into_vec(self) -> Vec<u8> {
        // LLM: Return an owned encoded key, reusing overflow storage when present.
        self.overflow.unwrap_or_else(|| self.inline[..self.len].to_vec()) }
}

/// LLM: Append one unsigned integer as a prefix-free varint to a history key. Used alternately
/// LLM: for interned value IDs and counts so each key identifies exactly one counted row.
fn integer(mut n: usize, key: &mut Key) {
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    while n >= 128 { key.push((n as u8 & 127) | 128); n >>= 7; }
    key.push(n as u8);
}

impl<T: Clone + Ord> History<T> {
    /// LLM: Merge another exact history, decoding its local value dictionary when necessary
    /// LLM: before reinterning rows. Used by the retained phase-pipeline experiment.
    pub(crate) fn append(&mut self, other: Self) {
        // LLM: Prepare `other` for the following computation; it stores the other for this
        // LLM: stage.
        let other = match other {
            Self::Plain(mut rows) => {
                if let Self::Plain(own) = self { own.append(&mut rows); return; }
                rows.into_iter().collect::<Vec<_>>()
            }
            Self::Compressed { values, keys } => {
                // LLM: Build readable names in the same order as the corresponding stored IDs.
                let mut names: Vec<Option<T>> = vec![None; values.len()];
                for (value, id) in values { names[id] = Some(value); }
                // LLM: Retain the newly constructed choices in the result for this stage.
                keys.into_iter().map(|key| {
                    // LLM: Prepare `bytes` for the following computation; it stores the bytes
                    // LLM: for this stage.
                    let mut bytes = key.into_iter();
                    let mut parts = Vec::new();
                    while let Some(byte) = bytes.next() {
                        let id = read_integer(byte, &mut bytes);
                        let count = read_integer(bytes.next().unwrap(), &mut bytes) as u32;
                        parts.push((names[id].as_ref().unwrap().clone(), count));
                    }
                    Configuration::new(parts).unwrap()
                }).collect()
            }
        };
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for row in other { self.insert(row); }
    }
    /// LLM: Test exact generated-row membership without modifying history or interning unknown
    /// LLM: values. Used concurrently to filter candidates before ordered insertion.
    pub(crate) fn contains(&self, configuration: &Configuration<T>) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self {
            Self::Plain(set) => set.contains(configuration),
            Self::Compressed { values, keys } => {
                // LLM: Encode the complete counted row for exact history lookup.
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let Some(&id) = values.get(value) else { return false; };
                    integer(id, &mut key); integer(*count as usize, &mut key);
                }
                keys.contains(key.as_slice())
            }
        }
    }

    /// LLM: Record a complete generated row and return whether it is new. Used by the closure
    /// LLM: to avoid repeating work even for locally rejected rows.
    pub(crate) fn insert(&mut self, configuration: Configuration<T>) -> bool {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self {
            Self::Plain(set) => set.insert(configuration),
            Self::Compressed { values, keys } => {
                // LLM: Encode the complete counted row for exact history lookup.
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let next = values.len();
                    let id = *values.entry(value.clone()).or_insert(next);
                    integer(id, &mut key); integer(*count as usize, &mut key);
                }
                keys.insert(key.into_vec())
            }
        }
    }

    /// LLM: Return the number of stored entries from History. Used by the retained comparison
    /// LLM: benchmarks without exposing mutable storage.
    pub(crate) fn len(&self) -> usize {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        match self { Self::Plain(set) => set.len(), Self::Compressed { keys, .. } => keys.len() }
    }
}

/// LLM: Decode a complete unsigned varint from a compressed history key. Used only by the
/// LLM: retained history-merge experiment to reconstruct counted rows.
fn read_integer(first: u8, bytes: &mut impl Iterator<Item = u8>) -> usize {
    // LLM: Collect the computed values, preserving their association with the requested work
    // LLM: items.
    let mut result = usize::from(first & 127);
    let (mut byte, mut shift) = (first, 7);
    while byte & 128 != 0 {
        byte = bytes.next().unwrap(); result |= usize::from(byte & 127) << shift; shift += 7;
    }
    result
}

impl<T: Clone + Ord> FromIterator<Configuration<T>> for History<T> {
    /// LLM: Create exact history from counted rows using the experiment's selected compression
    /// LLM: policy. Used by the retained comparison kernel.
    fn from_iter<I: IntoIterator<Item = Configuration<T>>>(iter: I) -> Self {
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let mut result = if crate::four_flags::enabled("compressed") {
            Self::Compressed { values: BTreeMap::new(), keys: BTreeSet::new() }
        } else { Self::Plain(BTreeSet::new()) };
        for configuration in iter { result.insert(configuration); }
        result
    }
}
