#![allow(dead_code)]

use crate::bignum::BigNum;
use crate::constraint::Constraint;
use crate::line::Line;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use log::trace;

/// A problem is represented by its left and right constraints, where left is the active side.
/// All the following is then optional.
/// We may have a mapping from labels to their string representation, represented by a list of tuples (String,number).
/// We may have a mapping from the current labels to the sets of the previous step.
/// We may have the mapping of the string representation of the problem of the previous step.
/// We may have computed if the current problem is trivial.
/// We may have computed the strength diagram for the labels on the right side, represented by a vector of directed edges.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Hash)]
pub struct Problem {
    pub left: Constraint,
    pub right: Constraint,
    pub map_text_label: Vec<(String, usize)>,
    pub is_trivial: bool,
    pub diagram: Vec<(usize, usize)>,
    pub reachable: Vec<(usize, usize)>,
    pub map_label_oldset: Option<Vec<(usize, BigNum)>>,
    pub map_text_oldlabel: Option<Vec<(String, usize)>>,
    pub coloring: usize,
    pub mergeable: Vec<BigNum>,
}

pub enum DiagramType {
    Fast,
    Accurate,
}

impl Problem {
    /// Constructor a problem.
    pub fn new(
        left: Constraint,
        right: Constraint,
        map_text_label: Option<Vec<(String, usize)>>,
        map_label_oldset: Option<Vec<(usize, BigNum)>>,
        map_text_oldlabel: Option<Vec<(String, usize)>>,
        diagramtype: DiagramType,
    ) -> Result<Self, String> {
        if left.lines.len() == 0 || right.lines.len() == 0 {
            return Err("Empty constraints!".into());
        }
        let mut p = Self {
            left,
            right,
            map_text_label: vec![],
            map_label_oldset,
            map_text_oldlabel,
            diagram: vec![],
            reachable: vec![],
            is_trivial: false,
            coloring: 0,
            mergeable: vec![],
        };
        if let Some(map) = map_text_label {
            p.map_text_label = map;
        } else {
            p.assign_chars();
        }
        trace!("computing triviality");
        p.compute_triviality();
        trace!("computing independent lines");
        p.compute_independent_lines();
        trace!("computing diagram");
        p.compute_diagram_edges(diagramtype);
        trace!("computing mergeable");
        p.compute_mergeable();
        trace!("done");
        Ok(p)
    }

    /// Check if the constraints are well formed, that is, the same set of labels appears on both sides
    pub fn has_same_labels_left_right(&self) -> bool {
        self.left.real_mask() == self.right.real_mask()
    }

    /// Construct a problem starting from left and right constraints.
    pub fn from_constraints(left: Constraint, right: Constraint) -> Result<Self, String> {
        Self::new(left, right, None, None, None, DiagramType::Accurate)
    }

    /// Construct a problem starting from a text representation (where there are left and right constraint separated by an empty line).
    pub fn from_line_separated_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let left: String = lines
            .by_ref()
            .take_while(|line| !line.is_empty())
            .join("\n");
        let right: String = lines.join("\n");
        Self::from_text(&left, &right)
    }

    /// Construct a problem starting from a text representation of the left and right constarints.
    pub fn from_text(left: &str, right: &str) -> Result<Self, String> {
        let map_text_label = Self::create_map_text_label(left, right);
        let hm = Self::map_to_hashmap(&map_text_label);
        let left = Constraint::from_text(left, &hm)?;
        let right = Constraint::from_text(right, &hm)?;
        let problem = Self::new(
            left,
            right,
            Some(map_text_label),
            None,
            None,
            DiagramType::Accurate,
        )?;
        if !problem.has_same_labels_left_right() {
            return Err("Left and right constraints have different sets of labels!".into());
        }
        Ok(problem)
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
    pub fn map_label_text(&self) -> HashMap<usize, String> {
        Self::map_to_inv_hashmap(&self.map_text_label)
    }

    /// Accessory function to convert a list of pairs `(a,b)` to a map mapping `a` to `b`
    pub fn map_to_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<A, B>
    where
        A: Hash + Eq + Clone,
        B: Clone,
    {
        map.iter().map(|(a, b)| (a.clone(), b.clone())).collect()
    }

    /// Accessory function to convert a list of pairs `(a,b)` to a map mapping `b` to `a`
    pub fn map_to_inv_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<B, A>
    where
        B: Hash + Eq + Clone,
        A: Clone,
    {
        map.iter().map(|(a, b)| (b.clone(), a.clone())).collect()
    }

    /// Returns a new problem where the label `from` has been replaced with label `to`.
    /// The new problem is easier than the original one
    pub fn replace(&self, from: usize, to: usize, diagramtype: DiagramType) -> Problem {
        let left = self.left.replace(from, to);
        let right = self.right.replace(from, to);
        let map_label_oldset = self
            .map_label_oldset
            .as_ref()
            .map(|map| map.iter().cloned().filter(|&(l, _)| l != from).collect());
        let map_text_oldlabel = self.map_text_oldlabel.clone();
        let map_text_label = self
            .map_text_label
            .iter()
            .cloned()
            .filter(|&(_, l)| l != from)
            .collect();
        let p = Problem::new(
            left,
            right,
            Some(map_text_label),
            map_label_oldset,
            map_text_oldlabel,
            diagramtype,
        )
        .unwrap();
        p
    }

    /// Allow the label `to` each time `from` is present, in the right constraints
    /// The new problem is easier than the original one
    /// This adds an arrow from `from` to `to` in the diagram
    pub fn relax_add_arrow(&self, from: usize, to: usize, diagramtype: DiagramType) -> Problem {
        let left = self.left.clone();
        let right = self.right.imply(from, to);
        let map_label_oldset = self.map_label_oldset.clone();
        let map_text_oldlabel = self.map_text_oldlabel.clone();
        let map_text_label = self.map_text_label.clone();
        let p = Problem::new(
            left,
            right,
            Some(map_text_label),
            map_label_oldset,
            map_text_oldlabel,
            diagramtype,
        ).unwrap();
        p
    }

    /// Make the problem harder, by keeping only labels satisfying the bitmask `keepmask`.
    /// All predecessors of a removed label are added, this is done in order to get the easiest possible
    /// problem obtainable while removing a label
    /// diagramtype tells if we should recompute the diagram in an accurate manner, or if
    /// we should get an approximate one from the original problem
    pub fn harden(
        &self,
        mut keepmask: BigNum,
        diagramtype: DiagramType,
        usepred: bool,
    ) -> Option<Problem> {
        let mut left = self.left.clone();
        if usepred {
            let remove = !keepmask & self.left.mask;
            let ones = remove.count_ones();
            for unrelax in remove.one_bits() {
                let pred = self.predecessors(unrelax, ones == 1);
                left = left.replace_with_group(unrelax, pred);
            }
        }

        // if, by making the problem harder, we get different sets of labels on the two sides,
        // repeat the operation, until we get the same set of labels
        let mut right = self.right.clone();
        loop {
            left = left.harden(keepmask);
            right = right.harden(keepmask);
            keepmask = left.real_mask() & right.real_mask();
            if left.real_mask() == keepmask && right.real_mask() == keepmask {
                break;
            }
        }

        // actually, it should be that either both are zero or none are zero
        if left.lines.len() == 0 || right.lines.len() == 0 {
            return None;
        }

        let map_label_oldset = self.map_label_oldset.as_ref().map(|map| {
            map.iter()
                .cloned()
                .filter(|&(l, _)| !((BigNum::one() << l) & keepmask).is_zero())
                .collect()
        });
        let map_text_oldlabel = self.map_text_oldlabel.clone();
        let map_text_label = self
            .map_text_label
            .iter()
            .cloned()
            .filter(|&(_, l)| !((BigNum::one() << l) & keepmask).is_zero())
            .collect();

        let p = Problem::new(
            left,
            right,
            Some(map_text_label),
            map_label_oldset,
            map_text_oldlabel,
            diagramtype,
        )
        .unwrap();

        Some(p)
    }

    /// computes a map from a label to its (direct and indirect) predecessors
    pub fn map_label_predecesors(&self) -> Vec<BigNum> {
        let mut v = vec![BigNum::zero();self.max_label()+1];
        for x in self.labels() {
            v[x] = self.predecessors(x, false);
        }
        v
    }

    /// Computes all direct predecessors of a given label
    pub fn predecessors(&self, lab: usize, immediate : bool) -> BigNum {
        let what = if immediate { &self.diagram } else { &self.reachable };
        what
            .iter()
            .cloned()
            .filter(|&(_, b)| b == lab)
            .map(|(a, _)| BigNum::one() << a)
            .fold(BigNum::zero(), |a, b| a | b)
    }

    /// Computes if the current problem is 0 rounds solvable, saving the result
    pub fn compute_triviality(&mut self) {
        // add_permutations should be a no-op if this is called from speedup()
        // and in this way it always works
        // and cloning right makes this function side-effect free on the constraints
        let mut right = self.right.clone();
        right.add_permutations();
        assert!(self.left.bits == right.bits);
        let bits = self.left.bits;
        let delta_r = right.delta;

        let is_trivial = self.left.choices_iter().any(|action| {
            let mask = action.mask();
            Line::forall_single(delta_r, bits, mask).all(|x| right.satisfies(&x))
        });

        self.is_trivial = is_trivial;
    }

    /// Computes the number of independent actions. If that number is x, then given an x coloring it is possible to solve the problem in 0 rounds.
    pub fn compute_independent_lines(&mut self) {
        let mut right = self.right.clone();
        right.add_permutations();
        assert!(self.left.bits == right.bits);
        let bits = self.left.bits;
        let delta_r = right.delta;
        if delta_r != 2 {
            return;
        }
        let mut edges = vec![];
        // it could be improved by a factor 2 (also later)...
        for l1 in self.left.choices_iter() {
            let m1 = l1.mask();
            'outer: for l2 in self.left.choices_iter() {
                let m2 = l2.mask();
                for p1 in m1.one_bits() {
                    for p2 in m2.one_bits() {
                        let num = ((BigNum::one() << p1) << bits) | (BigNum::one() << p2);
                        let line = Line::from(2, bits, num);
                        if !right.satisfies(&line) {
                            continue 'outer;
                        }
                    }
                }
                edges.push((l1, l2));
            }
        }
        if edges.is_empty() {
            self.coloring = 1;
            return;
        }

        let map: HashMap<_, _> = edges
            .iter()
            .cloned()
            .chain(edges.iter().map(|&(a, b)| (b, a)))
            .map(|(a, _)| a)
            .unique()
            .enumerate()
            .map(|(i, x)| (x, i))
            .collect();
        let n = map.len();
        let mut adj_l = vec![vec![]; n];
        let mut adj_m = vec![vec![false; n]; n];
        for (a, b) in edges {
            let a = map[&a];
            let b = map[&b];
            adj_l[a].push(b);
            adj_m[a][b] = true;
        }

        self.coloring = 2;
        for sz in 3..=n {
            let valid: Vec<_> = adj_l
                .iter()
                .enumerate()
                .filter(|(_, l)| l.len() >= sz - 1)
                .map(|(i, _)| i)
                .collect();
            if valid.len() < sz {
                break;
            }
            use permutator::copy::Combination;
            'search: for set in valid.combination(sz) {
                for &x in &set {
                    for &y in &set {
                        if x != y && !adj_m[x][y] {
                            continue 'search;
                        }
                    }
                }
                self.coloring = sz;
                break;
            }
            if self.coloring != sz {
                break;
            }
        }
    }

    /// If the current problem is T >0 rounds solvable, return a problem that is exactly T-1 rounds solvable,
    /// such that a solution of the new problem can be converted in 1 round to a solution for the origina problem,
    /// and a solution for the original problem can be converted in 0 rounds to a solution for the new problem.
    pub fn speedup(&self, diagramtype: DiagramType) -> Result<Self, String> {
        let mut left = self.left.clone();
        let mut right = self.right.clone();

        trace!("1) adding permutations");
        left.add_permutations();
        right.add_permutations();

        trace!("2) starting forall");
        let mut newleft_before_renaming = right.new_constraint_forall(&self.map_label_predecesors());

        trace!("3) removing permutations forall");
        newleft_before_renaming.add_permutations();
        newleft_before_renaming.remove_permutations();

        let map_label_oldset: Vec<_> = newleft_before_renaming.sets().enumerate().collect();
        let hm_oldset_label = Self::map_to_inv_hashmap(&map_label_oldset);

        trace!("4) checking size");

        let newbits = hm_oldset_label.len();
        if newbits * std::cmp::max(self.left.delta, self.right.delta) > BigNum::MAX.bits() {
            return Err(format!("The currently configured limit for delta*labels is {}, but in order to represent the result of this speedup a limit of {}*{} is required.",BigNum::MAX.bits(),newbits,std::cmp::max(self.left.delta, self.right.delta)));
        }

        trace!("5) starting exists");
        let newleft = newleft_before_renaming.renamed(&hm_oldset_label);
        let mut newright = left.new_constraint_exist(&hm_oldset_label);

        trace!("6) removing permutations exists");
        newright.remove_permutations();
        trace!("7) creating new problem");

        Self::new(
            newleft,
            newright,
            None,
            Some(map_label_oldset),
            Some(self.map_text_label.clone()),
            diagramtype,
        )
    }

    /// Computes the strength diagram for the labels on the right constraints.
    /// We put an edge from A to B if each time A can be used then also B can be used.
    pub fn compute_diagram_edges(&mut self, diagramtype: DiagramType) {
        match (diagramtype, self.map_label_oldset.is_some()) {
            (DiagramType::Fast, true) => self.compute_diagram_edges_from_oldsets(),
            _ => self.compute_diagram_edges_from_rightconstraints(),
        }
    }

    /// One way to compute the diagram is using set inclusion,
    /// if this problem is the result of a speedup.
    /// With this method, some edges may be missing.
    fn compute_diagram_edges_from_oldsets(&mut self) {
        let mut result = vec![];
        let mut reachable = vec![];
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
            for (otherlabel, _) in candidates {
                reachable.push((label, otherlabel));
            }
        }
        self.reachable = reachable;
        self.diagram = result;
    }

    /// Returns an iterator over the possible labels.
    pub fn labels(&self) -> impl Iterator<Item = usize> + Clone +  '_ {
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

    /// Returns the largest label. It may be different from num_labels()-1.
    pub fn max_label(&self) -> usize {
        self.labels().max().unwrap()
    }

    /// If this problem is not the result of a speedup, or if we really want all edges,
    /// we need to compute the diagram by looking at the right constraints.
    /// We put an edge from A to B if each time A can be used also B can be used.
    pub fn compute_diagram_edges_from_rightconstraints(&mut self) {
        let mut right = self.right.clone();
        right.add_permutations();

        let num_labels = self.max_label() + 1;
        let mut mat = vec![vec![false; num_labels]; num_labels];
        for x in self.labels() {
            for y in self.labels() {
                if x != y {
                    mat[x][y] = true;
                }
            }
        }
        //self.right does not contain permutations, right contains permutations
        for valid in self.right.choices_iter() {
            let mut done = vec![false;num_labels];
            for i in 0..self.right.delta {
                let label = valid.group(i).one_bits().next().unwrap();
                if done[label] {
                    continue;
                }
                done[label] = true;
                for x in self.labels() {
                    if !mat[label][x] {
                        continue;
                    }
                    let test = valid.with_group(i,BigNum::one()<<x);
                    if !right.satisfies(&test) {
                        mat[label][x] = false;
                    }
                }
            }
        }

        let adj : Vec<Vec<usize>> = mat.into_iter().map(|v|v.into_iter().enumerate().filter(|&(_,x)|x).map(|(i,_)|i).collect()).collect();

        let mut reachable = vec![];

        let mut result = vec![];
        let is_direct = |x: usize, y: usize| adj[y].iter().find(|&&t| x == t).is_none();

        for x in self.labels() {
            for &y in &adj[x] {
                let should_keep = adj[x]
                    .iter()
                    .filter(|&&t| t != y && is_direct(x, t))
                    .all(|&t| {
                        adj[t]
                            .iter()
                            .filter(|&&w| is_direct(t, w))
                            .find(|&&w| w == y)
                            .is_none()
                    });
                if should_keep {
                    result.push((x, y));
                }
                reachable.push((x, y));
            }
        }
        self.reachable = reachable;
        self.diagram = result;
    }

    /// Returns an iterator over all possible sets of labels.
    pub fn all_possible_sets(&self) -> impl Iterator<Item = BigNum> {
        assert!(self.left.bits == self.right.bits);
        assert!(self.left.mask == self.right.mask);
        let bits = self.left.bits;
        let mask = self.left.mask;
        // otherwise it's too slow anyway
        assert!(bits < 64);
        (1..(1u64 << bits))
            .map(|x| BigNum::from(x))
            .filter(move |&x| mask.is_superset(x))
    }

    /// Returns an iterator over all possible sets of labels that are actually needed for the speedup step.
    /// This currently means all possible right closed subsets of the diagram plus all possible singletons.
    fn allowed_sets_for_speedup(&self) -> Vec<BigNum> {
        //let m = self.diagram_adj();
        //let test : Vec<_> = self.all_possible_sets().filter(|&x| x.count_ones() == 1 || Problem::is_rightclosed(x, &m)).unique().sorted().collect();

        let rcs = self.right_closed_subsets();
        let result = self
            .labels()
            .map(|i| BigNum::one() << i)
            .chain(rcs.into_iter())
            .unique()
            .sorted()
            .collect();

        //assert!(test == result);
        result
    }

    fn rcs_helper(&self, right: &Vec<BigNum>, result: &mut Vec<BigNum>, added: BigNum, max: usize) {
        for x in self.labels() {
            let toadd = (BigNum::one() << x) | right[x];
            if x >= max && !added.bit(x) && (added == BigNum::zero() || !toadd.is_superset(added)) {
                let new = added | toadd;
                result.push(new);
                self.rcs_helper(right, result, new, x + 1);
            }
        }
    }

    pub fn right_closed_subsets(&self) -> Vec<BigNum> {
        let mut right = vec![BigNum::zero(); self.max_label() + 1];
        for &(a, b) in self.reachable.iter() {
            right[a] = right[a] | (BigNum::one() << b);
        }
        let mut result = vec![];
        self.rcs_helper(&right, &mut result, BigNum::zero(), 0);
        result.into_iter().unique().sorted().collect()
    }

    /// Return the adjacency list of the diagram.
    pub fn diagram_adj(&self) -> Vec<Vec<usize>> {
        assert!(self.left.bits == self.right.bits);
        let bits = self.left.bits;
        let diag = &self.diagram;

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
    /// otherwise each label i gets the string "(i)".
    pub fn assign_chars(&mut self) {
        self.map_text_label = self
            .labels()
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
                    (format!("({})", i), i as usize)
                }
            })
            .collect()
    }

    pub fn compute_mergeable(&mut self) {
        let adj: Vec<BigNum> = self
            .diagram_adj()
            .iter()
            .map(|v| {
                v.iter()
                    .map(|&c| BigNum::one() << c)
                    .fold(BigNum::zero(), |r, x| r | x)
            })
            .collect();

        let mut result = vec![];
        for i in 0..adj.len() {
            let mut set = BigNum::zero();
            for j in adj[i].one_bits() {
                if adj[j].bit(i) {
                    set = set | (BigNum::one() << j);
                }
            }
            if set != BigNum::zero() {
                set = set | (BigNum::one() << i);
                result.push(set);
            }
        }

        let result = result.into_iter().unique().sorted().collect();
        self.mergeable = result;
    }

    /// Returns a simple representation of the problem,
    /// where all possible optional things are computed (except of the mapping to the previous problem if it does not exist).
    pub fn as_result(&self) -> ResultProblem {
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

        let diagram = self
            .diagram
            .iter()
            .map(|(a, b)| (map[a].to_owned(), map[b].to_owned()))
            .collect();

        let is_trivial = self.is_trivial;
        let coloring = self.coloring;

        let mergeable = self
            .mergeable
            .iter()
            .map(|v| v.one_bits().map(|x| map[&x].to_owned()).collect())
            .collect();

        ResultProblem {
            left,
            right,
            mapping,
            diagram,
            is_trivial,
            coloring,
            mergeable,
        }
    }

    /// Return the pairs of labels that are not reachable one from the other on the diagram
    pub fn unreachable_pairs(&self) -> Vec<(usize, usize)> {
        let reachable : std::collections::HashSet<_> = self.reachable.iter().cloned().collect();
        self.labels().cartesian_product(self.labels())
            .filter(|&(a,b)|a != b)
            .filter(|&(a,b)| !reachable.contains(&(a,b)) && !reachable.contains(&(b,a)) ).collect()
    }

}

/// Simple representation of the problem.
/// Constraints are represented as vectors of lines,
/// where each line is represented as a vector of groups,
/// and each group as a vector of strings.
/// `mapping` represent a text mapping between sets of old labels and new labels.
/// `diagram` consists of a vector of edges, where each edge is represented by the text representation of the label.
/// `is_trivial` is true if and only if the problem is 0 rounds solvable.
#[derive(Deserialize, Serialize)]
pub struct ResultProblem {
    pub left: Vec<Vec<Vec<String>>>,
    pub right: Vec<Vec<Vec<String>>>,
    pub mapping: Option<Vec<(Vec<String>, String)>>,
    pub diagram: Vec<(String, String)>,
    pub is_trivial: bool,
    pub coloring: usize,
    pub mergeable: Vec<Vec<String>>,
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

        if self.coloring >= 2 {
            r += &format!(
                "\nThe problem is zero rounds solvable given a {} coloring.\n",
                self.coloring
            );
        }

        if !self.mergeable.is_empty() {
            r += "The following labels could be merged without changing the complexity of the problem:\n";
            for v in self.mergeable.iter() {
                r += &v.join(" ");
                r += "\n";
            }
        }

        write!(f, "{}", r)
    }

}
