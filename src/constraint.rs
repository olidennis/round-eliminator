#![allow(dead_code)]

use crate::bignum::BigNum;
use crate::line::Line;
use crate::lineset::BigBitSet;
use crate::lineset::LineSet;

use either::Either;
use itertools::Itertools;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Constraint {
    pub lines: Vec<Line>,
    pub delta: u8,
    pub bits: u8,
    pub mask: BigNum,
    permutations: Option<bool>,
}

impl Constraint {
    /// creates an empty set of constraints, where delta is `delta` and the number of possible labels is `bits`
    pub fn new(delta: u8, bits: u8) -> Self {
        Constraint {
            lines: vec![],
            delta,
            bits,
            mask: (BigNum::one() << bits) - BigNum::one(),
            permutations: None,
        }
    }

    /// make the constraints harder, by keeping only labels satisfying the bitmask `keepmask`
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

    /// make the constraints easier, by allowing `to` instead of `from`
    /// the problem gets easier assuming there is a diagram arrow from `from` to `to`
    pub fn relax(&self, from: usize, to: usize) -> Constraint {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        for line in self.lines.iter() {
            let newline = line.relax(from, to);
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

    /// add a line to the constraints
    /// if some old line is included in the current one, remove the old one
    /// if some old line includes the current one, do nothing
    /// does not change self.permutations
    pub fn add_reduce(&mut self, newline: Line) {
        let l1 = self.lines.len();
        self.lines.retain(|oldline| !newline.includes(oldline));
        let l2 = self.lines.len();
        if l1 != l2 || self.lines.iter().all(|oldline| !oldline.includes(&newline)) {
            self.add(newline);
        }
    }

    /// add a line to the constraints, no check is performed
    pub fn add(&mut self, newline: Line) {
        self.lines.push(newline);
    }

    /// add all possible permutations of the current lines
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

    /// discard lines such that there are no two lines where one is a permutation of the other
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

    /// returns true if `v` is included in at least one line of the constraints
    pub fn satisfies(&self, v: &Line) -> bool {
        self.lines.iter().any(|line| line.includes(v))
    }

    /// given some constraint such that their groups represent sets, rename those sets such that
    /// each possible group of each line value gets a single bit in the new lines
    pub fn renamed(&self, mapping: &HashMap<BigNum, usize>) -> Constraint {
        let newbits = mapping.len() as u8;
        let mut new = Constraint::new(self.delta, newbits);
        for line in self.lines.iter() {
            new.add_reduce(line.renamed(mapping));
        }
        new
    }

    pub fn parse(text : &str) -> Vec<Vec<Vec<String>>> {
        text.lines().map(|line|Line::parse(line)).collect()
    }

    /// create constraints starting from their string representation
    pub fn from_text(text: &str, mapping: &HashMap<String, usize>) -> Constraint {
        let parsed = Constraint::parse(text);

        let first = Line::from_parsed(&parsed[0], mapping);
        let delta = first.delta;
        let bits = first.bits;

        let mut c = Constraint::new(delta, bits);
        for line in &parsed {
            let line = Line::from_parsed(line, mapping);
            assert!(line.delta == delta);
            assert!(line.bits == bits);
            c.add_reduce(line);
        }
        c
    }

    /// creates a character representation of the constraints
    pub fn to_text(&self, mapping: &HashMap<usize, String>) -> String {
        self.lines
            .iter()
            .map(|line| line.to_text(mapping))
            .join("\n")
    }

    /// return the unique groups appearing among the lines of the constraints
    pub fn sets(&self) -> impl Iterator<Item = BigNum> {
        self.lines
            .iter()
            .flat_map(|line| line.groups())
            .unique()
            .sorted()
    }

    /// perform the existential step on the current constraints
    pub fn new_constraint_exist(&self, mapping: &HashMap<BigNum, usize>) -> Constraint {
        let newbits = mapping.len() as u8;
        let mut new = Constraint::new(self.delta, newbits);
        for line in &self.lines {
            new.add_reduce(line.anymap(mapping));
        }
        new
    }

    /// perform the universal step on the current constraints
    /// `sets` contains the allowed sets
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

    /// compute the new constraints
    /// each new line group now represents a set
    /// the new constraints are the maximal constraints satisfying that
    /// any choice over the sets is satisfied by the original constraints
    /// new constraints are computing by enumerating all non-allowed lines and taking all possible supersets of them
    /// and then taking the complement
    /// `sets` indicates which sets are allowed as a result
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

    /// given some sets
    /// compute the adjacency matrix
    /// there is an edge between sets a and b if b strictly includes a
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

    /// create a mapping between a set and its position in the adj matrix
    /// a plain array is used instead of a HashMap to make things faster
    fn sets_adj_map(sets: &Vec<BigNum>, bits: u8) -> Vec<usize> {
        let mut v = vec![0; 1 << bits];
        for (i, x) in sets.iter().enumerate() {
            v[x.as_u64() as usize] = i;
        }
        v
    }

    /// given some lines and some allowed sets
    /// it computes a set of all possible lines that include at least one given line
    /// the groups of the new lines should be contained in the allowed sets
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

    /// recursive helper for superlines_over
    pub fn superlines_add<T>(
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
            let pos = map[group.as_u64() as usize];
            for &sup in &succ[pos] {
                let newgroup = sets[sup];
                let newline = line.with_group(i, newgroup);
                if !h.contains(newline) {
                    Self::superlines_add(newline, h, sets, &succ, &map);
                }
            }
        }
    }

    /// returns an iterator over all possible choices over the constraints
    /// if the current constraints are the left side of the result of a speedup, things can be made fast
    /// otherwise, just do forall and check for sat
    pub fn choices_iter(&self) -> impl Iterator<Item = Line> + '_ {
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
