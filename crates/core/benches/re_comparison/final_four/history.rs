//! Benchmark-only exact dictionary/varint compression of configuration keys.

use crate::model::Configuration;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) enum History<T> {
    Plain(BTreeSet<Configuration<T>>),
    Compressed { values: BTreeMap<T, usize>, keys: BTreeSet<Vec<u8>> },
}

struct Key { inline: [u8; 128], len: usize, overflow: Option<Vec<u8>> }
impl Key {
    fn new() -> Self { Self { inline: [0;128], len: 0, overflow: None } }
    fn push(&mut self, byte: u8) {
        if let Some(bytes) = &mut self.overflow { bytes.push(byte); }
        else if self.len < self.inline.len() { self.inline[self.len] = byte; }
        else { let mut bytes = self.inline.to_vec(); bytes.push(byte); self.overflow = Some(bytes); }
        self.len += 1;
    }
    fn as_slice(&self) -> &[u8] { self.overflow.as_deref().unwrap_or(&self.inline[..self.len.min(128)]) }
    fn into_vec(self) -> Vec<u8> { self.overflow.unwrap_or_else(|| self.inline[..self.len].to_vec()) }
}

fn integer(mut n: usize, key: &mut Key) {
    while n >= 128 { key.push((n as u8 & 127) | 128); n >>= 7; }
    key.push(n as u8);
}

impl<T: Clone + Ord> History<T> {
    pub(crate) fn append(&mut self, other: Self) {
        let other = match other {
            Self::Plain(mut rows) => {
                if let Self::Plain(own) = self { own.append(&mut rows); return; }
                rows.into_iter().collect::<Vec<_>>()
            }
            Self::Compressed { values, keys } => {
                let mut names: Vec<Option<T>> = vec![None; values.len()];
                for (value, id) in values { names[id] = Some(value); }
                keys.into_iter().map(|key| {
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
        for row in other { self.insert(row); }
    }
    pub(crate) fn contains(&self, configuration: &Configuration<T>) -> bool {
        match self {
            Self::Plain(set) => set.contains(configuration),
            Self::Compressed { values, keys } => {
                let mut key = Key::new();
                for (value, count) in configuration.multiplicities() {
                    let Some(&id) = values.get(value) else { return false; };
                    integer(id, &mut key); integer(*count as usize, &mut key);
                }
                keys.contains(key.as_slice())
            }
        }
    }

    pub(crate) fn insert(&mut self, configuration: Configuration<T>) -> bool {
        match self {
            Self::Plain(set) => set.insert(configuration),
            Self::Compressed { values, keys } => {
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

    pub(crate) fn len(&self) -> usize {
        match self { Self::Plain(set) => set.len(), Self::Compressed { keys, .. } => keys.len() }
    }
}

fn read_integer(first: u8, bytes: &mut impl Iterator<Item = u8>) -> usize {
    let mut result = usize::from(first & 127);
    let (mut byte, mut shift) = (first, 7);
    while byte & 128 != 0 {
        byte = bytes.next().unwrap(); result |= usize::from(byte & 127) << shift; shift += 7;
    }
    result
}

impl<T: Clone + Ord> FromIterator<Configuration<T>> for History<T> {
    fn from_iter<I: IntoIterator<Item = Configuration<T>>>(iter: I) -> Self {
        let mut result = if crate::four_flags::enabled("compressed") {
            Self::Compressed { values: BTreeMap::new(), keys: BTreeSet::new() }
        } else { Self::Plain(BTreeSet::new()) };
        for configuration in iter { result.insert(configuration); }
        result
    }
}
