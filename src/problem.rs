#![allow(dead_code)]

use crate::bignum::BigNum;
use crate::constraint::Constraint;
use crate::line::Line;
use itertools::Itertools;
use std::collections::HashMap;
use std::hash::Hash;
use serde::{Deserialize,Serialize};


#[derive(Clone, Debug, Eq, PartialEq,Deserialize,Serialize)]
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
    pub fn new(
        left: Constraint,
        right: Constraint,
        map_text_label: Option<Vec<(String, usize)>>,
        map_label_oldset: Option<Vec<(usize, BigNum)>>,
        map_text_oldlabel: Option<Vec<(String, usize)>>,
        diagram: Option<Vec<(usize, usize)>>,
    ) -> Self {
        Self {
            left,
            right,
            map_text_label,
            map_label_oldset,
            map_text_oldlabel,
            diagram,
            is_trivial: None,
        }
    }

    pub fn from_constraints(left: Constraint, right: Constraint) -> Self {
        Self::new(left, right, None, None, None, None)
    }

    pub fn from_line_separated_text(text : &str) -> Self {
        let mut lines = text.lines();
        let left : String = lines.by_ref().take_while(|line|!line.is_empty()).join("\n");
        let right : String = lines.join("\n");
        Self::from_text(&left,&right)
    }

    pub fn from_text(left: &str, right: &str) -> Self {
        let map_text_label = Self::create_map_text_label(left, right);
        let hm = Self::map_to_hashmap(&map_text_label);
        let left = Constraint::from_text(left, &hm);
        let right = Constraint::from_text(right, &hm);
        let mut problem = Self::new(left, right, Some(map_text_label), None, None, None);
        problem.compute_triviality();
        problem.compute_diagram_edges();
        problem
    }

    fn create_map_text_label(left: &str, right: &str) -> Vec<(String, usize)> {
        let pleft = Constraint::string_to_vec(left);
        let pright = Constraint::string_to_vec(right);

        pleft.into_iter()
            .chain(pright.into_iter())
            .flat_map(|v|v.into_iter())
            .flat_map(|v|v.into_iter())
            .unique()
            .sorted()
            .enumerate()
            .map(|(i, c)| (c, i))
            .collect()
    }

    fn map_to_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<A, B>
    where
        A: Hash + Eq + Clone,
        B: Clone,
    {
        map.iter().map(|(a, b)| (a.clone(), b.clone())).collect()
    }

    fn map_to_inv_hashmap<A, B>(map: &Vec<(A, B)>) -> HashMap<B, A>
    where
        B: Hash + Eq + Clone,
        A: Clone,
    {
        map.iter().map(|(a, b)| (b.clone(), a.clone())).collect()
    }

    pub fn relax(&self, from: usize, to: usize) -> Problem {
        let left = self.left.relax(from, to);
        let right = self.right.relax(from, to);
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
            None, //TODO: do not recompute all diagram
        )
    }

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
            None, //TODO: do not recompute all diagram
        )
    }

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
        );
        result.left.remove_permutations();
        result.compute_triviality();
        result.right.remove_permutations();
        result.compute_diagram_edges();
        result
    }

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

    pub fn labels(&self) -> impl Iterator<Item = usize> + '_ {
        assert!(self.left.mask == self.right.mask);
        let mask = self.left.mask;
        (0..mask.bits()).filter(move |&i| mask.bit(i))
    }

    pub fn num_labels(&self) -> usize {
        assert!(self.left.mask == self.right.mask);
        let mask = self.left.mask;
        mask.count_ones() as usize
    }

    fn get_diagram_edges_from_rightconstraints(&mut self) -> Vec<(usize, usize)> {
        let mut right = self.right.clone();
        right.add_permutations();
        let num_labels = self.num_labels();
        let mut adj = vec![vec![]; num_labels];
        for x in self.labels() {
            for y in self.labels() {
                let is_left = x != y && right
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

    fn all_possible_sets(&self) -> impl Iterator<Item = BigNum> {
        assert!(self.left.bits == self.right.bits);
        assert!(self.left.mask == self.right.mask);
        let bits = self.left.bits;
        let mask = self.left.mask;
        (1..(1 << bits))
            .map(|x| BigNum::from(x))
            .filter(move |&x| mask.is_superset(x))
    }

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

    fn allowed_sets_for_speedup(&self) -> Vec<BigNum> {
        let m = self.diagram_adj();
        self.all_possible_sets()
            .filter(|&x| Problem::is_rightclosed_or_singleton(x, &m))
            .collect()
    }

    fn is_rightclosed_or_singleton(set: BigNum, m: &Vec<Vec<usize>>) -> bool {
        if set.count_ones() == 1 {
            return true;
        }

        for x in set.one_bits() {
            for &t in &m[x] {
                if !set.bit(t) {
                    return false;
                }
            }
        }
        true
    }

    pub fn assign_chars(&mut self) {
        if self.map_text_label.is_none() {
            self.map_text_label = Some(self.labels().map(|i|{
                if self.num_labels() <= 62 {
                    let i = i as u8;
                    let c = match i {
                        0..=25 => { (b'A' + i) as char }
                        26..=51 => { (b'a' + i -26) as char }
                        52..=61 => { (b'0' + i -52) as char }
                        _ => { (b'z' +1 + i -62) as char }
                    };
                    (format!("{}",c),i as usize)
                } else {
                    (format!("<{}>",i),i as usize)
                }
            }).collect());
        }
	}

    pub fn as_result(&mut self) -> ResultProblem {
        self.assign_chars();
        let map = Self::map_to_inv_hashmap(self.map_text_label.as_ref().unwrap());

        let left = self.left.to_vec(&map);
        let right = self.right.to_vec(&map);

        let mapping = match (self.map_label_oldset.as_ref(),self.map_text_oldlabel.as_ref()){
            (Some(lo),Some(to)) => {
                let oldmap = Self::map_to_inv_hashmap(to);
                let mut v = vec![];
                for (l,o) in lo {
                    let old = o.one_bits().map(|x|oldmap[&x].to_owned()).collect();
                    let new = map[l].to_owned();
                    v.push((old,new));
                }
                Some(v)
            }
            _ => None
        };

        self.compute_diagram_edges();
        let diagram = self.diagram.as_ref().unwrap();
        let diagram = diagram.iter().map(|(a,b)|(map[a].to_owned(),map[b].to_owned())).collect();

        self.compute_triviality();
        let is_trivial = self.is_trivial.unwrap();

        ResultProblem{left,right,mapping,diagram,is_trivial}
    }
}


pub struct ResultProblem {
    pub left : Vec<Vec<Vec<String>>>,
    pub right : Vec<Vec<Vec<String>>>,
    pub mapping : Option<Vec<(Vec<String>,String)>>,
    pub diagram : Vec<(String,String)>,
    pub is_trivial : bool
}

impl std::fmt::Display for ResultProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        let mut r = String::new();
        if let Some(mapping) = &self.mapping {
            r += "Mapping\n";
            for (o,l) in mapping {
                let s : String = o.iter().join("");
                r += &format!("{} <- {}\n",l,s);
            }
        }

        let left = self.left.iter().map(|x|x.iter().map(|t|t.iter().join("")).join(" ")).join("\n");
        let right = self.right.iter().map(|x|x.iter().map(|t|t.iter().join("")).join(" ")).join("\n");

        r += "\nLeft (Active)\n";
        r += &format!("{}\n",left);
        r += "\nRight (Passive)\n";
        r += &format!("{}\n",right);

        r += "\nDiagram\n";
        for (s1, s2) in self.diagram.iter() {
            r += &format!("{} -> {}\n",s1,s2);
        }

		r += "\nThe problem is ";
		if !self.is_trivial {
			r += "NOT ";
		}
		r += "zero rounds solvable.\n";

        write!(f,"{}",r)
    }
}