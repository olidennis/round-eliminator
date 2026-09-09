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
                    if edges.len() > 500_000 { return Err(LIMIT.into()); }
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
            Step::Prune => self.prune(budget, eh),
            Step::NodeContext => self.node_context(stage, budget, eh),
            Step::Matching(graph) => self.matching(graph, stage, budget, eh),
            Step::GreedyColoring(graph) => self.greedy_coloring(graph, stage, budget, eh),
            Step::PriorityMis { graph, order } => {
                self.priority_mis(graph, order, stage, budget, eh)
            }
            Step::RulingSet(graph) => self.ruling_set(graph, stage, budget, eh),
            Step::RepairPairs(pairs) => super::repair::transform(self, pairs, budget, eh),
            Step::FindMis(_) => Err("Unresolved MIS search is not a certificate".into()),
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
                        if edges.len() > 500_000 { return Err(LIMIT.into()); }
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
                        if edges.len() > 500_000 { return Err(LIMIT.into()); }
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

    fn prune(&self, budget: &Budget, eh: &EventHandler) -> Result<Self, String> {
        let mut nodes = self.nodes.clone();
        let mut edges = self.edges.clone();
        loop {
            budget.check(eh)?;
            let used: BTreeSet<_> = nodes.iter().flatten().copied().collect();
            edges.retain(|[a, b]| used.contains(a) && used.contains(b));
            let supported: BTreeSet<_> = edges.iter().flatten().copied().collect();
            let before = nodes.len();
            nodes.retain(|row| row.iter().all(|s| supported.contains(s)));
            if nodes.len() == before {
                break;
            }
        }
        let used: BTreeSet<_> = nodes.iter().flatten().copied().collect();
        let ids: BTreeMap<_, _> = used.iter().enumerate().map(|(i, &s)| (s, i)).collect();
        self.finish(
            nodes
                .iter()
                .map(|r| r.iter().map(|s| ids[s]).collect())
                .collect(),
            edges.iter().map(|[a, b]| edge(ids[a], ids[b])).collect(),
            used.iter().map(|&s| self.base[s]).collect(),
            used.iter().map(|&s| self.names[s].clone()).collect(),
            budget,
        )
    }

    fn matching(
        &self,
        graph: &Subgraph,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        // U=unmatched, H/T=matched port at head/tail, h/t=other ports of that node.
        if self.names.len().saturating_mul(5) > budget.options.max_states {
            return Err(LIMIT.into());
        }
        let possible: BTreeSet<_> = self
            .edges
            .iter()
            .filter(|&&[a, b]| self.relevant(graph, a, b))
            .flatten()
            .copied()
            .collect();
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            insert(&mut nodes, row.iter().map(|&s| 5 * s).collect(), budget)?;
            for (i, &s) in row.iter().enumerate() {
                budget.check(eh)?;
                if !possible.contains(&s) {
                    continue;
                }
                for (selected, other) in [(1, 2), (3, 4)] {
                    insert(
                        &mut nodes,
                        row.iter()
                            .enumerate()
                            .map(|(j, &t)| 5 * t + if i == j { selected } else { other })
                            .collect(),
                        budget,
                    )?;
                }
            }
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &self.edges {
            let relevant = self.relevant(graph, a, b);
            for x in 0..5 {
                for y in 0..5 {
                    budget.check(eh)?;
                    let allowed = if x == 1 || x == 3 || y == 1 || y == 3 {
                        relevant && matches!((x, y), (1, 3) | (3, 1))
                    } else {
                        !relevant || x != 0 || y != 0
                    };
                    if allowed {
                        edges.insert(edge(5 * a + x, 5 * b + y));
                        if edges.len() > 500_000 { return Err(LIMIT.into()); }
                    }
                }
            }
        }
        let base = self.base.iter().flat_map(|&l| [l; 5]).collect();
        let names = self
            .names
            .iter()
            .flat_map(|s| ["U", "H", "h", "T", "t"].map(|r| format!("{s}; match{stage}={r}")))
            .collect();
        self.finish(nodes, edges, base, names, budget)
    }

    fn priority_mis(
        &self,
        graph: &Subgraph,
        order: &[Vec<Label>],
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        let mut ids = BTreeMap::new();
        let mut old = vec![];
        let mut ranks = vec![];
        let mut base = vec![];
        let mut names = vec![];
        let mut nodes = BTreeSet::new();
        let possible: BTreeSet<_> = self
            .edges
            .iter()
            .filter(|&&[a, b]| self.relevant(graph, a, b))
            .flatten()
            .copied()
            .collect();
        for row in &self.nodes {
            let mut original: Vec<_> = row.iter().map(|&s| self.base[s]).collect();
            original.sort();
            let rank = order
                .iter()
                .position(|r| r == &original)
                .ok_or("Node configuration missing from priority order")?;
            let mut lifted = vec![];
            for &s in row {
                let id = *ids.entry((s, rank)).or_insert_with(|| {
                    let id = old.len();
                    old.push(s);
                    ranks.push(rank);
                    for role in ["I", "U", "P"] {
                        base.push(self.base[s]);
                        names.push(format!(
                            "{}; priorityMIS{stage}[{rank}]={role}",
                            self.names[s]
                        ));
                    }
                    id
                });
                if names.len() > budget.options.max_states {
                    return Err(LIMIT.into());
                }
                lifted.push(id);
            }
            insert(&mut nodes, lifted.iter().map(|&s| 3 * s).collect(), budget)?;
            for (i, &s) in row.iter().enumerate() {
                if !possible.contains(&s) {
                    continue;
                }
                insert(
                    &mut nodes,
                    lifted
                        .iter()
                        .enumerate()
                        .map(|(j, &t)| 3 * t + if i == j { 2 } else { 1 })
                        .collect(),
                    budget,
                )?;
            }
        }
        let mut edges = BTreeSet::new();
        for a in 0..old.len() {
            for b in a..old.len() {
                budget.check(eh)?;
                if !self.edges.contains(&edge(old[a], old[b])) {
                    continue;
                }
                let relevant = self.relevant(graph, old[a], old[b]);
                for x in 0..3 {
                    for y in 0..3 {
                        let allowed = if relevant {
                            match (x, y) {
                                (0, 1) | (1, 0) | (1, 1) => true,
                                (0, 2) => ranks[a] <= ranks[b],
                                (2, 0) => ranks[b] <= ranks[a],
                                _ => false,
                            }
                        } else {
                            x != 2 && y != 2
                        };
                        if allowed {
                            edges.insert(edge(3 * a + x, 3 * b + y));
                            if edges.len() > 500_000 { return Err(LIMIT.into()); }
                        }
                    }
                }
            }
        }
        self.finish(nodes, edges, base, names, budget)
    }

    fn greedy_coloring(
        &self,
        graph: &Subgraph,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        let colors = self.nodes.first().map_or(1, |r| r.len() + 1);
        let mut ids = BTreeMap::new();
        let mut names = vec![];
        let mut base = vec![];
        let mut relevant = BTreeSet::new();
        let mut external = BTreeSet::new();
        for &[a, b] in &self.edges {
            if self.relevant(graph, a, b) {
                relevant.extend([a, b]);
            } else {
                external.extend([a, b]);
            }
        }
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            for c in 0..=row.iter().filter(|s| relevant.contains(s)).count() {
                let choices: Vec<Vec<_>> = row
                    .iter()
                    .map(|s| {
                        (0..=colors)
                            .filter(|&d| {
                                if d == colors {
                                    external.contains(s)
                                } else {
                                    c != d && relevant.contains(s)
                                }
                            })
                            .collect()
                    })
                    .collect();
                products(&choices, budget, eh, |neighbors| {
                    if (0..c).all(|d| neighbors.contains(&d)) {
                        let mut out = vec![];
                        for (&s, d) in row.iter().zip(neighbors) {
                            let id = *ids.entry((s, c, d)).or_insert_with(|| {
                                let id = names.len();
                                base.push(self.base[s]);
                                names.push(format!(
                                    "{}; greedy{stage}={c}, neighbor={}",
                                    self.names[s],
                                    if d == colors {
                                        "outside".into()
                                    } else {
                                        d.to_string()
                                    }
                                ));
                                id
                            });
                            if names.len() > budget.options.max_states {
                                return Err(LIMIT.into());
                            }
                            out.push(id);
                        }
                        insert(&mut nodes, out, budget)?;
                    }
                    Ok(())
                })?;
            }
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &self.edges {
            for c in 0..colors {
                for d in 0..colors {
                    budget.check(eh)?;
                    let states = if self.relevant(graph, a, b) {
                        if c == d {
                            continue;
                        }
                        (ids.get(&(a, c, d)), ids.get(&(b, d, c)))
                    } else {
                        (ids.get(&(a, c, colors)), ids.get(&(b, d, colors)))
                    };
                    if let (Some(&s), Some(&t)) = states {
                        edges.insert(edge(s, t));
                        if edges.len() > 500_000 { return Err(LIMIT.into()); }
                    }
                }
            }
        }
        self.finish(nodes, edges, base, names, budget)
    }

    fn node_context(
        &self,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        let input = self.prune(budget, eh)?;
        let mut groups = vec![vec![]; input.names.len()];
        let mut base = vec![];
        let mut names = vec![];
        let mut nodes = BTreeSet::new();
        for row in &input.nodes {
            budget.check(eh)?;
            let context = row
                .iter()
                .map(|&s| input.names[s].as_str())
                .collect::<Vec<_>>()
                .join(" | ");
            let mut ids = BTreeMap::new();
            for &s in row {
                if ids.contains_key(&s) {
                    continue;
                }
                let id = names.len();
                ids.insert(s, id);
                groups[s].push(id);
                base.push(input.base[s]);
                names.push(format!("{}; node{stage}=({context})", input.names[s]));
                if names.len() > budget.options.max_states {
                    return Err(LIMIT.into());
                }
            }
            insert(&mut nodes, row.iter().map(|s| ids[s]).collect(), budget)?;
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &input.edges {
            for &s in &groups[a] {
                for &t in &groups[b] {
                    budget.check(eh)?;
                    edges.insert(edge(s, t));
                    if edges.len() > 500_000 { return Err(LIMIT.into()); }
                    if edges.len() > 500_000 {
                        return Err(LIMIT.into());
                    }
                }
            }
        }
        self.finish(nodes, edges, base, names, budget)
    }

    fn ruling_set(
        &self,
        graph: &Subgraph,
        stage: usize,
        budget: &Budget,
        eh: &EventHandler,
    ) -> Result<Self, String> {
        let mut relevant = BTreeSet::new();
        let mut external = BTreeSet::new();
        for &[a, b] in &self.edges {
            if self.relevant(graph, a, b) {
                relevant.extend([a, b]);
            } else {
                external.extend([a, b]);
            }
        }
        let mut ids = BTreeMap::new();
        let mut base = vec![];
        let mut names = vec![];
        let mut nodes = BTreeSet::new();
        for row in &self.nodes {
            for c in 0..3 {
                let choices: Vec<Vec<_>> = row
                    .iter()
                    .map(|s| {
                        (0..4)
                            .filter(|&d| {
                                if d == 3 {
                                    external.contains(s)
                                } else {
                                    relevant.contains(s)
                                        && match c {
                                            0 => d == 1,
                                            1 => true,
                                            _ => d != 0,
                                        }
                                }
                            })
                            .collect()
                    })
                    .collect();
                products(&choices, budget, eh, |neighbors| {
                    let legal = match c {
                        0 => true,
                        1 => neighbors.iter().filter(|&&d| d == 0).count() == 1,
                        _ => neighbors.contains(&1),
                    };
                    if !legal {
                        return Ok(());
                    }
                    let mut out = vec![];
                    for (&s, d) in row.iter().zip(neighbors) {
                        let id = *ids.entry((s, c, d)).or_insert_with(|| {
                            let id = names.len();
                            base.push(self.base[s]);
                            names.push(format!(
                                "{}; ruling{stage}={c}, neighbor={}",
                                self.names[s],
                                if d == 3 {
                                    "outside".into()
                                } else {
                                    d.to_string()
                                }
                            ));
                            id
                        });
                        if names.len() > budget.options.max_states {
                            return Err(LIMIT.into());
                        }
                        out.push(id);
                    }
                    insert(&mut nodes, out, budget)
                })?;
            }
        }
        let mut edges = BTreeSet::new();
        for &[a, b] in &self.edges {
            for c in 0..3 {
                for d in 0..3 {
                    budget.check(eh)?;
                    let states = if self.relevant(graph, a, b) {
                        (ids.get(&(a, c, d)), ids.get(&(b, d, c)))
                    } else {
                        (ids.get(&(a, c, 3)), ids.get(&(b, d, 3)))
                    };
                    if let (Some(&s), Some(&t)) = states {
                        edges.insert(edge(s, t));
                    }
                }
            }
        }
        self.finish(nodes, edges, base, names, budget)
    }
}
