use crate::bignum::BigNum;
use crate::line::Line;
use crate::lineset::BigBitSet;
use crate::lineset::LineSet;

use either::Either;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// This struct represents a set of constraints.
/// It is represented by a set of lines.
/// Delta is the degree. Bits is the number of bits required to describe a group of labels.
/// Mask indicates which bits of the group should be considered.
/// Permutations indicates whether all permutations of each line are also included,
/// where true indicates that they are, false indicates that the lines have been minimized by removing permutations,
/// and none indicates that the constraints are arbitrary.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Constraint {
    pub lines: Vec<Line>,
    pub delta: usize,
    pub bits: usize,
    pub mask: BigNum,
    permutations: Option<bool>,
}

impl Constraint {
    /// Creates an empty set of constraints, where each line has `delta` groups of `bits` bits.
    pub fn new(delta: usize, bits: usize) -> Self {
        Constraint {
            lines: vec![],
            delta,
            bits,
            mask: (BigNum::one() << bits) - BigNum::one(),
            permutations: None,
        }
    }

    /// Make the constraints harder, by keeping only labels satisfying the bitmask `keepmask`.
    pub fn harden(&self, keepmask: BigNum) -> Constraint {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        for line in self.lines.iter() {
            if let Some(newline) = line.harden(keepmask) {
                newlines.push(newline);
            }
        }
        Self {
            lines: newlines,
            delta,
            bits,
            mask: self.mask & keepmask,
            permutations: None,
        }
    }

    /// Creates a new set of constraints, where for each line the label `from` is replaced by the label `to`.
    pub fn replace(&self, from: usize, to: usize) -> Constraint {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        for line in self.lines.iter() {
            let newline = line.replace(from, to);
            newlines.push(newline);
        }
        Self {
            lines: newlines,
            delta,
            bits,
            mask: self.mask & !(BigNum::one() << from),
            permutations: None,
        }
    }

    /// Add a line to the constraints.
    /// If some old line is included in the current one, remove the old one.
    /// If some old line includes the current one, do nothing.
    /// Does not change self.permutations.
    pub fn add_reduce(&mut self, newline: Line) {
        let l1 = self.lines.len();
        self.lines.retain(|oldline| !newline.includes(oldline));
        let l2 = self.lines.len();
        if l1 != l2 || self.lines.iter().all(|oldline| !oldline.includes(&newline)) {
            self.add(newline);
        }
    }

    /// Add a line to the constraints, no check is performed.
    pub fn add(&mut self, newline: Line) {
        self.lines.push(newline);
    }

    /// Add all possible permutations of the current lines.
    pub fn add_permutations(&mut self) {
        if self.permutations == Some(true) {
            return;
        }
        let old = std::mem::replace(&mut self.lines, vec![]);
        for oldline in old {
            for newline in oldline.permutations().iter() {
                self.add_reduce(newline);
            }
        }
        self.permutations = Some(true);
    }

    /// Minimize lines by removing permutations, that is, discard lines such that there are no two lines where one is a permutation of the other.
    pub fn remove_permutations(&mut self) {
        if self.permutations == Some(false) {
            return;
        }
        let old = std::mem::replace(&mut self.lines, vec![]);
        'outer: for oldline in old {
            for newline in oldline.permutations().iter() {
                if self.satisfies(&newline) {
                    continue 'outer;
                }
            }
            self.add_reduce(oldline);
        }
        self.permutations = Some(false);
    }

    /// Returns true if `v` is included in at least one line of the constraints.
    pub fn satisfies(&self, v: &Line) -> bool {
        self.lines.iter().any(|line| line.includes(v))
    }

    /// Rename the labels, that is, for each line, each possible group value gets a single bit in the new line.
    /// `mapping` indicates how to map groups to labels.
    /// For example, if 011010 should be mapped to 1000, then mapping should map `011010` to 3 (the position of the unique bit with value 1).
    pub fn renamed(&self, mapping: &HashMap<BigNum, usize>) -> Constraint {
        let newbits = mapping.len();
        if newbits * self.delta > BigNum::MAX.bits() {
            panic!("The result is too big");
        }
        let mut new = Constraint::new(self.delta, newbits);
        for line in self.lines.iter() {
            new.add_reduce(line.renamed(mapping));
        }
        new
    }

    /// Create constraints starting from their text representation.
    pub fn from_text(text: &str, mapping: &HashMap<String, usize>) -> Constraint {
        let vec = Self::string_to_vec(text);
        Self::from_vec(&vec, mapping)
    }

    /// Given a string that represents a set of constraint, the string is parsed and split in a vector representation,
    /// Each resulting vector represents a vector representation of a line, where
    /// each of its resulting vectors represents a single group of the line.
    /// Each group is represented by a vector of strings.
    pub fn string_to_vec(text: &str) -> Vec<Vec<Vec<String>>> {
        text.lines().map(|line| Line::string_to_vec(line)).collect()
    }

    /// Creates a set of constraints starting from its vector representation.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn from_vec(v: &Vec<Vec<Vec<String>>>, mapping: &HashMap<String, usize>) -> Constraint {
        let first = Line::from_vec(&v[0], mapping);
        let delta = first.delta;
        let bits = first.bits;

        let mut c = Constraint::new(delta, bits);
        for line in v {
            let line = Line::from_vec(line, mapping);
            assert!(line.delta == delta);
            assert!(line.bits == bits);
            c.add_reduce(line);
        }
        c
    }

    /// Creates a vector representation of the constraints.
    /// Each resulting vecot represents a line
    /// Each of its vectors represents a single group of the line.
    /// Each group is represented by a vector of strings.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn to_vec(&self, mapping: &HashMap<usize, String>) -> Vec<Vec<Vec<String>>> {
        self.lines.iter().map(|line| line.to_vec(mapping)).collect()
    }

    /// Returns the unique groups appearing among the lines of the constraints.
    pub fn sets(&self) -> impl Iterator<Item = BigNum> {
        self.lines
            .iter()
            .flat_map(|line| line.groups())
            .unique()
            .sorted()
    }

    /// Performs the existential step on the current constraints.
    /// `mapping` maps old sets to the new labels.
    pub fn new_constraint_exist(&self, mapping: &HashMap<BigNum, usize>) -> Constraint {
        let newbits = mapping.len();
        let mut new = Constraint::new(self.delta, newbits);
        for line in &self.lines {
            new.add_reduce(line.anymap(mapping));
        }
        new
    }

    /// Performs the universal step on the current constraints.
    /// `sets` contains the allowed sets for the new constraints,
    /// that is, not all possible sets of labels are tested,
    /// but only the ones contained in `sets`.
    pub fn new_constraint_forall(&self, sets: &Vec<BigNum>) -> Constraint {
        let delta = self.delta as usize;
        let bits = self.bits as usize;

        // either use a bitset or a hashset, depending on the size of the problem
        if delta * bits > 32 {
            self.new_constraint_forall_best::<HashSet<BigNum>>(sets)
        } else {
            self.new_constraint_forall_best::<BigBitSet>(sets)
        }
    }

    /// Compute the new constraints.
    /// Each new line group now represents a set.
    /// The obtained constraints are the maximal constraints satisfying that
    /// any choice over the sets is satisfied by the original constraints.
    /// New constraints are computed by enumerating all non-allowed lines and taking all possible supersets of them,
    /// and then taking the complement.
    /// `sets` indicates which sets are allowed as a result, that is, not all possible sets of labels are tested,
    /// but only the ones contained in `sets`.
    fn new_constraint_forall_best<T>(&self, sets: &Vec<BigNum>) -> Constraint
    where
        T: LineSet,
    {
        let delta = self.delta;
        let bits = self.bits;

        let mut bad = Constraint::new(delta, bits);
        for line in Line::forall_single(delta, bits, self.mask).filter(|line| !self.satisfies(line))
        {
            bad.add(line);
        }
        let bad: T = bad.superlines_over(sets);

        let mut newconstraints = Constraint::new(delta, bits);
        if self.permutations == Some(true) {
            newconstraints.permutations = Some(true);
        } else {
            newconstraints.permutations = None;
        }

        for line in Line::forall_over(delta, bits, sets) {
            if !bad.contains(line) {
                newconstraints.add_reduce(line);
            }
        }
        newconstraints
    }

    /// Given some sets,
    /// computes the adjacency matrix of the graph obtained by putting an edge from A to B
    /// if B strictly includes A
    fn set_inclusion_adj(sets: &Vec<BigNum>) -> Vec<Vec<usize>> {
        let mut v = vec![vec![]; sets.len()];
        for (i, &r) in sets.iter().enumerate() {
            for (j, &x) in sets.iter().enumerate() {
                if x != r && x.is_superset(r) {
                    v[i].push(j);
                }
            }
        }
        v
    }

    /// Creates a mapping between a set and its position in the adj matrix of the graph described in `set_inclusion_adj`.
    /// A plain array is used instead of a HashMap to make things faster.
    fn sets_adj_map(sets: &Vec<BigNum>, bits: usize) -> Vec<usize> {
        let mut v = vec![0; 1 << bits];
        for (i, x) in sets.iter().enumerate() {
            v[x.as_usize()] = i;
        }
        v
    }

    /// Given some lines and some allowed sets,
    /// it computes a set of all possible lines that include at least one given line.
    /// The groups of the new lines should be contained in the allowed sets.
    fn superlines_over<T>(&self, sets: &Vec<BigNum>) -> T
    where
        T: LineSet,
    {
        let succ = Self::set_inclusion_adj(sets);
        let map = Self::sets_adj_map(sets, self.bits);
        let mut h = T::new(self.delta, self.bits);
        for &line in self.lines.iter() {
            if !h.contains(line) {
                Self::superlines_add(line, &mut h, sets, &succ, &map);
            }
        }
        h
    }

    /// Recursive helper for `superlines_over`.
    fn superlines_add<T>(
        line: Line,
        h: &mut T,
        sets: &Vec<BigNum>,
        succ: &Vec<Vec<usize>>,
        map: &Vec<usize>,
    ) where
        T: LineSet,
    {
        h.insert(line);
        for (i, group) in line.groups().enumerate() {
            //this will panic if group can not fit an usize
            let pos = map[group.as_usize()];
            for &sup in &succ[pos] {
                let newgroup = sets[sup];
                let newline = line.with_group(i, newgroup);
                if !h.contains(newline) {
                    Self::superlines_add(newline, h, sets, &succ, &map);
                }
            }
        }
    }

    /// Returns an iterator over all possible choices over the constraints.
    pub fn choices_iter(&self) -> impl Iterator<Item = Line> + '_ {
        // If the current constraints are the left side of the result of a speedup, things can be made fast
        // otherwise, just do forall and check for sat
        let is_easy = self.lines.iter().all(|line| line.is_action());
        if is_easy {
            Either::Left(self.lines.iter().cloned())
        } else {
            Either::Right(
                Line::forall_single(self.delta, self.bits, self.mask)
                    .filter(move |line| self.satisfies(line)),
            )
        }
    }
}
