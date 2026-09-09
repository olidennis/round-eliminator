use super::*;

#[derive(Clone, Debug)]
pub(super) struct Input {
    pub nodes: Vec<Vec<usize>>,
    pub edges: BTreeSet<[usize; 2]>,
    pub base: Vec<Label>,
    pub names: Vec<String>,
}

pub(super) fn edge(a: usize, b: usize) -> [usize; 2] {
    [a.min(b), a.max(b)]
}

fn insert(
    nodes: &mut BTreeSet<Vec<usize>>,
    mut row: Vec<usize>,
    budget: &Budget,
) -> Result<(), String> {
    row.sort_unstable();
    nodes.insert(row);
    if nodes.len() > budget.options.max_configurations {
        return Err(LIMIT.into());
    }
    Ok(())
}

fn products(
    groups: &[Vec<usize>],
    budget: &Budget,
    eh: &EventHandler,
    mut accept: impl FnMut(Vec<usize>) -> Result<(), String>,
) -> Result<(), String> {
    if groups.iter().any(Vec::is_empty) {
        return Ok(());
    }
    let mut digits = vec![0; groups.len()];
    loop {
        budget.check(eh)?;
        accept(groups.iter().zip(&digits).map(|(g, &i)| g[i]).collect())?;
        let mut i = 0;
        loop {
            if i == digits.len() {
                return Ok(());
            }
            digits[i] += 1;
            if digits[i] < groups[i].len() {
                break;
            }
            digits[i] = 0;
            i += 1;
        }
    }
}

impl Input {
    pub fn new(p: &Problem, budget: &Budget, eh: &EventHandler) -> Result<Self, String> {
        let base = p.labels();
        let ids: BTreeMap<_, _> = base.iter().enumerate().map(|(i, &l)| (l, i)).collect();
        let names: BTreeMap<_, _> = p.mapping_label_text.iter().cloned().collect();
        let mut nodes = BTreeSet::new();
        for line in &p.active.lines {
            let mut groups = Vec::new();
            for part in &line.parts {
                let GroupType::Many(n) = part.gtype else {
                    return Err("Infinite node multiplicity is unsupported".into());
                };
                for _ in 0..n {
                    groups.push(part.group.iter().map(|l| ids[l]).collect());
                }
            }
            if groups.len() != p.active.finite_degree() {
                return Err("Inconsistent node degree".into());
            }
            products(&groups, budget, eh, |row| insert(&mut nodes, row, budget))?;
        }
        let mut edges = BTreeSet::new();
        for (i, &a) in base.iter().enumerate() {
            for (j, &b) in base.iter().enumerate().skip(i) {
                budget.check(eh)?;
                if p.passive.includes(&edge_line([a, b])) {
                    edges.insert([i, j]);
                }
            }
        }
        let names = base
            .iter()
            .map(|l| {
                names
                    .get(l)
                    .map(|n| format!("{n} [label {l}]"))
                    .ok_or_else(|| "Missing original label name".to_string())
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            nodes: nodes.into_iter().collect(),
            edges,
            base,
            names,
        })
    }

    fn relevant(&self, graph: &Subgraph, a: usize, b: usize) -> bool {
        match graph {
            Subgraph::All => true,
            Subgraph::Pairs(pairs) => pairs.contains(&pair(self.base[a], self.base[b])),
        }
    }

    pub fn step(
        &self,
        step: &Step,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        match step {
            Step::Mis(graph) => self.mis(graph, stage, budget, eh),
            Step::Coloring => self.coloring(stage, budget, eh),
            Step::Exchange => self.exchange(stage, budget, eh),
        }
    }

    fn finish(
        &self,
        nodes: BTreeSet<Vec<usize>>,
        edges: BTreeSet<[usize; 2]>,
        base: Vec<Label>,
        names: Vec<String>,
        budget: &Budget,
    ) -> Result<Self, String> {
        if names.len() > budget.options.max_states
            || edges.len() > 500_000
            || names.iter().any(|s| s.len() > 4096)
            || names.iter().map(String::len).sum::<usize>() > 2_000_000
        {
            return Err(LIMIT.into());
        }
        Ok(Self {
            nodes: nodes.into_iter().collect(),
            edges,
            base,
            names,
        })
    }

    fn mis(
        &self,
        graph: &Subgraph,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        if self.names.len().saturating_mul(3) > budget.options.max_states {
            return Err(LIMIT.into());
        }
        // 0 = selected (I), 1 = unselected (U), 2 = parent port (P).
        // All ports of a node carry the same membership; P is one U-port
        // pointing to a selected neighbor THROUGH AN EDGE OF THIS SUBGRAPH.
        let mut possible_parent = BTreeSet::new();
        for &[a, b] in &self.edges {
            if self.relevant(graph, a, b) {
                possible_parent.extend([a, b]);
            }
        }
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            budget.check(eh)?;
            insert(&mut nodes, row.iter().map(|&s| 3 * s).collect(), budget)?;
            for (i, &s) in row.iter().enumerate() {
                if possible_parent.contains(&s) {
                    insert(
                        &mut nodes,
                        row.iter()
                            .enumerate()
                            .map(|(j, &t)| 3 * t + if i == j { 2 } else { 1 })
                            .collect(),
                        budget,
                    )?;
                }
            }
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &self.edges {
            budget.check(eh)?;
            for x in 0..3 {
                for y in 0..3 {
                    let allowed = if self.relevant(graph, a, b) {
                        matches!((x, y), (0, 1) | (1, 0) | (0, 2) | (2, 0) | (1, 1))
                    } else {
                        x != 2 && y != 2
                    };
                    if allowed {
                        edges.insert(edge(3 * a + x, 3 * b + y));
                    }
                }
            }
        }
        let base = self.base.iter().flat_map(|&l| [l; 3]).collect();
        let names = self
            .names
            .iter()
            .flat_map(|s| ["I", "U", "P"].map(|r| format!("{s}; MIS{stage}={r}")))
            .collect();
        self.finish(nodes, edges, base, names, budget)
    }

    fn coloring(&self, stage: usize, budget: &Budget, eh: &EventHandler) -> Result<Self, String> {
        let colors = self.nodes.first().map_or(1, |r| r.len() + 1);
        if self.names.len().saturating_mul(colors) > budget.options.max_states {
            return Err(LIMIT.into());
        }
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            for c in 0..colors {
                budget.check(eh)?;
                insert(
                    &mut nodes,
                    row.iter().map(|&s| s * colors + c).collect(),
                    budget,
                )?;
            }
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &self.edges {
            for x in 0..colors {
                for y in 0..colors {
                    budget.check(eh)?;
                    if x != y {
                        edges.insert(edge(a * colors + x, b * colors + y));
                    }
                }
            }
        }
        let base = self.base.iter().flat_map(|&l| vec![l; colors]).collect();
        let names = self
            .names
            .iter()
            .flat_map(|s| (0..colors).map(move |c| format!("{s}; color{stage}={}", c + 1)))
            .collect();
        self.finish(nodes, edges, base, names, budget)
    }

    fn exchange(&self, stage: usize, budget: &Budget, eh: &EventHandler) -> Result<Self, String> {
        let mut ids = BTreeMap::new();
        let mut groups = vec![vec![]; self.names.len()];
        let mut base = vec![];
        let mut names = vec![];
        for &[a, b] in &self.edges {
            budget.check(eh)?;
            for (s, t) in [(a, b), (b, a)] {
                if ids.contains_key(&(s, t)) {
                    continue;
                }
                let id = names.len();
                ids.insert((s, t), id);
                groups[s].push(id);
                base.push(self.base[s]);
                names.push(format!(
                    "{}; seen{stage}=({})",
                    self.names[s], self.names[t]
                ));
                if names.len() > budget.options.max_states {
                    return Err(LIMIT.into());
                }
            }
        }
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            let choices = row.iter().map(|&s| groups[s].clone()).collect::<Vec<_>>();
            products(&choices, budget, eh, |r| insert(&mut nodes, r, budget))?;
        }
        let edges = self
            .edges
            .iter()
            .map(|&[a, b]| edge(ids[&(a, b)], ids[&(b, a)]))
            .collect();
        self.finish(nodes, edges, base, names, budget)
    }
}
