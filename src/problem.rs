#![allow(dead_code)]

use crate::bignum::BigNum;
use crate::constraint::Constraint;
use crate::line::Line;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

/// A problem is represented by its left and right constraints, where left is the active side.
/// All the following is then optional.
/// We may have a mapping from labels to their string representation, represented by a list of tuples (String,number).
/// We may have a mapping from the current labels to the sets of the previous step.
/// We may have the mapping of the string representation of the problem of the previous step.
/// We may have computed if the current problem is trivial.
/// We may have computed the strength diagram for the labels on the right side, represented by a vector of directed edges.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Problem {
    pub left: Constraint,
    pub right: Constraint,
    pub map_text_label: Option<Vec<(String, usize)>>,
    pub map_label_oldset: Option<Vec<(usize, BigNum)>>,
    pub map_text_oldlabel: Option<Vec<(String, usize)>>,
    pub is_trivial: Option<bool>,
    pub diagram: Option<Vec<(usize, usize)>>,
}

impl Problem {
    /// Constructor a problem.
    pub fn new(
        left: Constraint,
        right: Constraint,
        map_text_label: Option<Vec<(String, usize)>>,
        map_label_oldset: Option<Vec<(usize, BigNum)>>,
        map_text_oldlabel: Option<Vec<(String, usize)>>,
        diagram: Option<Vec<(usize, usize)>>,
        is_trivial: Option<bool>,
    ) -> Self {
        Self {
            left,
            right,
            map_text_label,
            map_label_oldset,
            map_text_oldlabel,
            diagram,
            is_trivial,
        }
    }

    /// Construct a problem starting from left and right constraints.
    pub fn from_constraints(left: Constraint, right: Constraint) -> Self {
        Self::new(left, right, None, None, None, None, None)
    }

    /// Construct a problem starting from a text representation (where there are left and right constraint separated by an empty line).
    pub fn from_line_separated_text(text: &str) -> Self {
        let mut lines = text.lines();
        let left: String = lines
            .by_ref()
            .take_while(|line| !line.is_empty())
            .join("\n");
        let right: String = lines.join("\n");
        Self::from_text(&left, &right)
    }

    /// Construct a problem starting from a text representation of the left and right constarints.
    pub fn from_text(left: &str, right: &str) -> Self {
        let map_text_label = Self::create_map_text_label(left, right);
        let hm = Self::map_to_hashmap(&map_text_label);
        let left = Constraint::from_text(left, &hm);
        let right = Constraint::from_text(right, &hm);
        let mut problem = Self::new(left, right, Some(map_text_label), None, None, None, None);
        problem.compute_triviality();
        problem.compute_diagram_edges();
        problem
    }

    /// Given a text representation of left and right constraints,
    /// extract the list of string labels, and create a list of pairs containing
    /// pairs `(s,x)` where `s` is the string representation of the label number `x`
    fn create_map_text_label(left: &str, right: &str) -> Vec<(String, usize)> {
        let pleft = Constraint::string_to_vec(left);
        let pright = Constraint::string_to_vec(right);

        pleft
            .into_iter()
            .chain(pright.into_iter())
            .flat_map(|v| v.into_iter())
            .flat_map(|v| v.into_iter())
            .unique()
            .sorted()
            .enumerate()
            .map(|(i, c)| (c, i))
            .collect()
    }

    /// Creates a mapping from label numbers to their string representation
    pub fn map_label_text(&self) -> HashMap<usize,String> {
        Self::map_to_inv_hashmap(self.map_text_label.as_ref().unwrap())
    }

    /// Accessory function to convert a list of pairs `(a,b)` to a map mapping `a` to `b`
    fn map_to_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<A, B>
    where
        A: Hash + Eq + Clone,
        B: Clone,
    {
        map.iter().map(|(a, b)| (a.clone(), b.clone())).collect()
    }

    /// Accessory function to convert a list of pairs `(a,b)` to a map mapping `b` to `a`
    fn map_to_inv_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<B, A>
    where
        B: Hash + Eq + Clone,
        A: Clone,
    {
        map.iter().map(|(a, b)| (b.clone(), a.clone())).collect()
    }

    /// Returns a new problem where the label `from` has been replaced with label `to`.
    /// The new problem is strictly easier if there is a diagram edge from `from` to `to`.
    pub fn replace(&self, from: usize, to: usize) -> Problem {
        let left = self.left.replace(from, to);
        let right = self.right.replace(from, to);
        let map_label_oldset = self
            .map_label_oldset
            .as_ref()
            .map(|map| map.iter().cloned().filter(|&(l, _)| l != from).collect());
        let map_text_oldlabel = self.map_text_oldlabel.clone();
        let map_text_label = self
            .map_text_label
            .as_ref()
            .map(|map| map.iter().cloned().filter(|&(_, l)| l != from).collect());
        Problem::new(
            left,
            right,
            map_text_label,
            map_label_oldset,
            map_text_oldlabel,
            None, //TODO: do not recompute all diagram if (from,to) is an edge of the diagram
            None,
        )
    }

    /// Make the problem harder, by keeping only labels satisfying the bitmask `keepmask`.
    pub fn harden(&self, keepmask: BigNum) -> Problem {
        let left = self.left.harden(keepmask);
        let right = self.right.harden(keepmask);
        let map_label_oldset = self.map_label_oldset.as_ref().map(|map| {
            map.iter()
                .cloned()
                .filter(|&(l, _)| !((BigNum::one() << l) & keepmask).is_zero())
                .collect()
        });
        let map_text_oldlabel = self.map_text_oldlabel.clone();
        let map_text_label = self.map_text_label.as_ref().map(|map| {
            map.iter()
                .cloned()
                .filter(|&(_, l)| !((BigNum::one() << l) & keepmask).is_zero())
                .collect()
        });
        Problem::new(
            left,
            right,
            map_text_label,
            map_label_oldset,
            map_text_oldlabel,
            None, //TODO: do not recompute all diagram if (from,to) is an edge of the diagram
            None,
        )
    }

    /// Computes if the current problem is 0 rounds solvable, saving the result
    pub fn compute_triviality(&mut self) {
        // add_permutations should be a no-op if this is called from speedup()
        // and in this way it always works
        // and cloning right makes this function side-effect free on the constraints
        if self.is_trivial.is_some() {
            return;
        }
        let right = self.right.clone();
        self.right.add_permutations();
        assert!(self.left.bits == self.right.bits);
        let bits = self.left.bits;
        let delta_r = self.right.delta;

        let is_trivial = self.left.choices_iter().any(|action| {
            let mask = action.mask();
            Line::forall_single(delta_r, bits, mask).all(|x| self.right.satisfies(&x))
        });

        self.right = right;
        self.is_trivial = Some(is_trivial);
    }

    /// If the current problem is T >0 rounds solvable, return a problem that is exactly T-1 rounds solvable,
    /// such that a solution of the new problem can be converted in 1 round to a solution for the origina problem,
    /// and a solution for the original problem can be converted in 0 rounds to a solution for the new problem.
    pub fn speedup(&mut self) -> Self {
        self.compute_diagram_edges();
        let mut left = self.left.clone();
        let mut right = self.right.clone();
        left.add_permutations();
        right.add_permutations();
        let allowed_sets = self.allowed_sets_for_speedup();

        let newleft_before_renaming = right.new_constraint_forall(&allowed_sets);

        let map_label_oldset: Vec<_> = newleft_before_renaming.sets().enumerate().collect();
        let hm_oldset_label = Self::map_to_inv_hashmap(&map_label_oldset);

        let newleft = newleft_before_renaming.renamed(&hm_oldset_label);
        let newright = left.new_constraint_exist(&hm_oldset_label);

        let mut result = Self::new(
            newleft,
            newright,
            None,
            Some(map_label_oldset),
            self.map_text_label.clone(),
            None,
            None,
        );
        result.left.remove_permutations();
        result.compute_triviality();
        result.right.remove_permutations();
        result.compute_diagram_edges();
        result
    }

    /// Computes the strength diagram for the labels on the right constraints.
    /// We put an edge from A to B if each time A can be used then also B can be used.
    pub fn compute_diagram_edges(&mut self) {
        if self.diagram.is_some() {
            return;
        }
        let diag = if self.map_label_oldset.is_some() {
            self.get_diagram_edges_from_oldsets()
        } else {
            self.get_diagram_edges_from_rightconstraints()
        };
        self.diagram = Some(diag);
    }

    /// One way to compute the diagram is using set inclusion,
    /// if this problem is the result of a speedup.
    fn get_diagram_edges_from_oldsets(&self) -> Vec<(usize, usize)> {
        let mut result = vec![];
        let map_label_oldset = self.map_label_oldset.as_ref().unwrap();

        for &(label, oldset) in map_label_oldset.iter() {
            let candidates: Vec<_> = map_label_oldset
                .iter()
                .cloned()
                .filter(|&(_, otheroldset)| {
                    oldset != otheroldset && otheroldset.is_superset(oldset)
                })
                .collect();
            let mut right = candidates.clone();
            for &(_, set) in &candidates {
                right.retain(|&(_, rset)| rset == set || !rset.is_superset(set));
            }
            for (otherlabel, _) in right {
                result.push((label, otherlabel));
            }
        }
        result
    }

    /// Returns an iterator over the possible labels.
    pub fn labels(&self) -> impl Iterator<Item = usize> + '_ {
        assert!(self.left.mask == self.right.mask);
        let mask = self.left.mask;
        (0..mask.bits()).filter(move |&i| mask.bit(i))
    }

    /// Returns the number of labels.
    pub fn num_labels(&self) -> usize {
        assert!(self.left.mask == self.right.mask);
        let mask = self.left.mask;
        mask.count_ones() as usize
    }

    /// If this problem is not the result of a speedup,
    /// we need to compute the diagram by looking at the right constraints.
    /// We put an edge from A to B if each time A can be used also B can be used.
    fn get_diagram_edges_from_rightconstraints(&mut self) -> Vec<(usize, usize)> {
        let mut right = self.right.clone();
        right.add_permutations();
        let num_labels = self.num_labels();
        let mut adj = vec![vec![]; num_labels];
        for x in self.labels() {
            for y in self.labels() {
                let is_left = x != y
                    && right
                        .choices_iter()
                        .all(|line| right.satisfies(&line.replace_fast(x, y)));
                if is_left {
                    adj[x].push(y);
                }
            }
        }
        let mut result = vec![];
        for x in self.labels() {
            for &y in &adj[x] {
                let is_direct = adj[x]
                    .iter()
                    .filter(|&&t| t != y)
                    .all(|&t| adj[t].iter().find(|&&x| x == y).is_none());
                if is_direct {
                    result.push((x, y));
                }
            }
        }
        result
    }

    /// Returns an iterator over all possible sets of labels.
    fn all_possible_sets(&self) -> impl Iterator<Item = BigNum> {
        assert!(self.left.bits == self.right.bits);
        assert!(self.left.mask == self.right.mask);
        let bits = self.left.bits;
        let mask = self.left.mask;
        (1..(1 << bits))
            .map(|x| BigNum::from(x))
            .filter(move |&x| mask.is_superset(x))
    }

    /// Returns an iterator over all possible sets of labels that are actually needed for the speedup step.
    /// This currently means all possible right closed subsets of the diagram plus all possible singletons.
    fn allowed_sets_for_speedup(&self) -> Vec<BigNum> {
        let m = self.diagram_adj();
        self.all_possible_sets()
            .filter(|&x| x.count_ones() == 1 || Problem::is_rightclosed(x, &m))
            .collect()
    }

    /// Return the adjacency list of the diagram.
    pub fn diagram_adj(&self) -> Vec<Vec<usize>> {
        assert!(self.left.bits == self.right.bits);
        let bits = self.left.bits;
        let diag = self.diagram.as_ref().unwrap();

        let mut m = vec![vec![]; bits as usize];
        for &(x, y) in diag {
            m[x].push(y);
        }
        m
    }

    /// Returns true if the given set of labels is a right closed subset of the diagram.
    fn is_rightclosed(set: BigNum, m: &Vec<Vec<usize>>) -> bool {
        for x in set.one_bits() {
            for &t in &m[x] {
                if !set.bit(t) {
                    return false;
                }
            }
        }
        true
    }

    /// Assign a text representation to the labels.
    /// If there are at most 62 labels, single chars are used,
    /// otherwise each label i gets the string "<i>".
    pub fn assign_chars(&mut self) {
        if self.map_text_label.is_none() {
            self.map_text_label = Some(
                self.labels()
                    .map(|i| {
                        if self.num_labels() <= 62 {
                            let i = i as u8;
                            let c = match i {
                                0..=25 => (b'A' + i) as char,
                                26..=51 => (b'a' + i - 26) as char,
                                52..=61 => (b'0' + i - 52) as char,
                                _ => (b'z' + 1 + i - 62) as char,
                            };
                            (format!("{}", c), i as usize)
                        } else {
                            (format!("<{}>", i), i as usize)
                        }
                    })
                    .collect(),
            );
        }
    }

    /// Returns a simple representation of the problem,
    /// where all possible optional things are computed (except of the mapping to the previous problem if it does not exist).
    pub fn as_result(&mut self) -> ResultProblem {
        self.assign_chars();
        let map = self.map_label_text();

        let left = self.left.to_vec(&map);
        let right = self.right.to_vec(&map);

        let mapping = match (
            self.map_label_oldset.as_ref(),
            self.map_text_oldlabel.as_ref(),
        ) {
            (Some(lo), Some(to)) => {
                let oldmap = Self::map_to_inv_hashmap(to);
                let mut v = vec![];
                for (l, o) in lo {
                    let old = o.one_bits().map(|x| oldmap[&x].to_owned()).collect();
                    let new = map[l].to_owned();
                    v.push((old, new));
                }
                Some(v)
            }
            _ => None,
        };

        self.compute_diagram_edges();
        let diagram = self.diagram.as_ref().unwrap();
        let diagram = diagram
            .iter()
            .map(|(a, b)| (map[a].to_owned(), map[b].to_owned()))
            .collect();

        self.compute_triviality();
        let is_trivial = self.is_trivial.unwrap();

        ResultProblem {
            left,
            right,
            mapping,
            diagram,
            is_trivial,
        }
    }
}

/// Simple representation of the problem.
/// Constraints are represented as vectors of lines,
/// where each line is represented as a vector of groups,
/// and each group as a vector of strings.
/// `mapping` represent a text mapping between sets of old labels and new labels.
/// `diagram` consists of a vector of edges, where each edge is represented by the text representation of the label.
/// `is_trivial` is true if and only if the problem is 0 rounds solvable.
pub struct ResultProblem {
    pub left: Vec<Vec<Vec<String>>>,
    pub right: Vec<Vec<Vec<String>>>,
    pub mapping: Option<Vec<(Vec<String>, String)>>,
    pub diagram: Vec<(String, String)>,
    pub is_trivial: bool,
}

impl std::fmt::Display for ResultProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut r = String::new();
        if let Some(mapping) = &self.mapping {
            r += "Mapping\n";
            for (o, l) in mapping {
                let s: String = o.iter().join("");
                r += &format!("{} <- {}\n", l, s);
            }
        }

        let left = self
            .left
            .iter()
            .map(|x| x.iter().map(|t| t.iter().join("")).join(" "))
            .join("\n");
        let right = self
            .right
            .iter()
            .map(|x| x.iter().map(|t| t.iter().join("")).join(" "))
            .join("\n");

        r += "\nLeft (Active)\n";
        r += &format!("{}\n", left);
        r += "\nRight (Passive)\n";
        r += &format!("{}\n", right);

        r += "\nDiagram\n";
        for (s1, s2) in self.diagram.iter() {
            r += &format!("{} -> {}\n", s1, s2);
        }

        r += "\nThe problem is ";
        if !self.is_trivial {
            r += "NOT ";
        }
        r += "zero rounds solvable.\n";

        write!(f, "{}", r)
    }
}
