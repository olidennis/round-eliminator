// This is a conversion into Rust of the C code of Cliquer (https://users.aalto.fi/~pat/cliquer.html)

use std::collections::{HashMap, HashSet, BTreeSet};

use itertools::Itertools;
use log::trace;

pub struct Graph {
    n: usize,
    adj: Vec<Vec<usize>>,
    m: Vec<Vec<bool>>,
}

impl Graph {
    pub fn from_adj(adj: Vec<Vec<usize>>) -> Self {
        // assumes adj contains all bidirectional edges
        let n = adj.len();
        let mut m = vec![vec![false; n]; n];
        for (a, v) in adj.iter().enumerate() {
            for &b in v.iter() {
                if a != b {
                    m[a][b] = true;
                    m[b][a] = true;
                }
            }
        }
        Self { n, adj, m }
    }

    pub fn max_clique(&self) -> Vec<usize> {
        let n = self.n;
        let mut c = vec![0; n];
        let v = self.ordering();
        let mut max = 0;
        let mut candidates = vec![];
        let mut best = vec![];
        for i in (0..n).rev() {
            let vi = v[i];
            let si = v[i..].iter().cloned();
            let u: Vec<_> = si.filter(|&x| self.m[vi][x]).collect();
            let mut found = false;
            candidates.push(vi);
            self.clique_rec(&u, 1, &c, &mut max, &mut found, &mut candidates, &mut best);
            candidates.pop();
            c[vi] = max;
        }
        best
    }

    #[allow(clippy::too_many_arguments)]
    fn clique_rec(
        &self,
        mut u: &[usize],
        size: usize,
        c: &[usize],
        max: &mut usize,
        found: &mut bool,
        candidates: &mut Vec<usize>,
        best: &mut Vec<usize>,
    ) {
        if u.is_empty() && size > *max {
            *max = size;
            *found = true;
            *best = candidates.clone();
        }
        while !u.is_empty() {
            if size + u.len() <= *max {
                return;
            }
            let vi = u[0];
            if size + c[vi] <= *max {
                return;
            }
            u = &u[1..];
            let newu: Vec<_> = u.iter().cloned().filter(|&x| self.m[vi][x]).collect();
            candidates.push(vi);
            self.clique_rec(&newu, size + 1, c, max, found, candidates, best);
            candidates.pop();
            if *found {
                return;
            }
        }
    }

    pub fn ordering(&self) -> Vec<usize> {
        let n = self.n;

        let mut order = vec![];
        let mut remaining = vec![true; n];
        let mut degrees: Vec<_> = self.adj.iter().map(|v| v.len()).collect();
        let mut ncols = 0;

        while remaining.iter().any(|&x| x) {
            ncols += 1;
            let mut active = remaining.clone();
            while let Some(max) = degrees
                .iter()
                .cloned()
                .enumerate()
                .filter(|&(i, _)| active[i])
                .max_by_key(|&(_, x)| x)
            {
                let max = max.0;
                for &x in &self.adj[max] {
                    active[x] = false;
                    degrees[x] -= 1;
                }
                order.push(max);
                active[max] = false;
                remaining[max] = false;
            }
        }
        trace!("Upper bound on clique size: {}", ncols);
        assert!(order.len() == n);
        order
    }
}



pub struct HyperGraph {
    nodes: usize,
    adj: HashMap<usize,Vec<BTreeSet<usize>>>,
    m: HashSet<BTreeSet<usize>>,
    rank : usize
}

impl HyperGraph {

    pub fn from_hyperedges(vh: Vec<Vec<usize>>) -> Self {
        let nodes = vh.iter().flat_map(|h|h.iter()).max().map(|&x|x+1).unwrap_or(0);
        let rank = if !vh.is_empty() { vh[0].len() } else {2};

        let mut adj = HashMap::<_,Vec<BTreeSet<_>>>::new();
        let mut m = HashSet::<BTreeSet<_>>::new();

        for h in vh {
            let sh : BTreeSet<_> = h.iter().cloned().collect();
            for v in h {
                adj.entry(v).or_default().push(sh.clone());
            }
            m.insert(sh);
        }
        
        Self { nodes, adj, m, rank }
    }

    pub fn max_clique(&self) -> Vec<usize> {
        let mut nodes : Vec<usize> = self.adj.keys().cloned().collect();
        let mut best_set = vec![];
        for i in self.rank..=self.nodes {
            let mut next_nodes = HashSet::new();
            for set in nodes.iter().cloned().combinations(i) {
                if self.is_clique(&set) {
                    next_nodes.extend(set.iter().cloned());
                    best_set = set;
                }
            }
            if best_set.len() != i {
                break;
            }
            nodes = next_nodes.into_iter().collect();
        } 
        best_set
    }

    pub fn is_clique(&self, v : &Vec<usize>) -> bool {
        for h in v.iter().cloned().combinations(self.rank) {
            if !self.m.contains(&h.into_iter().collect()) {
                return false;
            }
        }
        true
    }

}