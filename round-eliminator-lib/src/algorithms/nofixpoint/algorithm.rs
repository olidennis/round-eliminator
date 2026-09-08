//! Native runner for the disabled certificate-to-algorithm helper.
//! Same ordering games, but constant-folded Boolean formulas and lazy cycle
//! exclusions replace the cubic tournament-transitivity materialization.

use super::*;
use rustsat::solvers::{PhaseLit, Solve, SolverResult};
use rustsat::types::{Assignment, TernaryVal};
use rustsat_minisat::{core::Minisat, Limit};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Options {
    pub time_limit: Option<Duration>,
    pub try_simple_orders: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            time_limit: Some(Duration::from_secs(180)),
            try_simple_orders: true,
        }
    }
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Stats {
    pub priorities: usize,
    pub games: usize,
    pub variables: u32,
    pub clauses: usize,
    pub comparisons: usize,
    pub cycle_cuts: usize,
    pub sat_models: usize,
    pub simple_orders: usize,
    #[serde(default)]
    pub best_simple_games: usize,
    pub elapsed_seconds: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Algorithm {
    pub certificate: String,
    /// Original readable Loop output, for provenance. The schedule indexes
    /// the synchronized `certificate`, not these possibly reduced trees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_certificate: Option<String>,
    pub degree: usize,
    pub colors: usize,
    pub arrows_per_expression: usize,
    /// Flattened [color][port/expression][inorder arrow position]. Higher
    /// priorities act first. The vector is a permutation of 0..priorities.
    pub ranks: Vec<usize>,
    pub stats: Stats,
}

#[derive(Debug, serde::Serialize)]
pub enum Outcome {
    Found(Algorithm),
    /// No ordering for THIS scheme/chosen reconstruction, not a lower bound.
    NoSchedule(Stats),
    Inconclusive(Stats),
}

#[derive(Clone)]
enum Node {
    Leaf(Label),
    Branch {
        arrow: Arrow,
        position: usize,
        children: [usize; 2],
        height: usize,
    },
}

struct Tree {
    nodes: Vec<Node>,
    root: usize,
}

impl Tree {
    fn new(expr: &Expr<Label>) -> Self {
        fn visit(expr: &Expr<Label>, nodes: &mut Vec<Node>, next: &mut usize) -> (usize, usize) {
            let node = match expr {
                Expr::Base(label, false) => Node::Leaf(*label),
                Expr::Base(_, true) => unreachable!("Input expressions are unmirrored"),
                Expr::Left(a, b) | Expr::Right(a, b) => {
                    let (a, ha) = visit(a, nodes, next);
                    let position = *next;
                    *next += 1;
                    let (b, hb) = visit(b, nodes, next);
                    Node::Branch {
                        arrow: if expr.is_left() {
                            Arrow::Left
                        } else {
                            Arrow::Right
                        },
                        position,
                        children: [a, b],
                        height: 1 + ha.max(hb),
                    }
                }
            };
            let height = match &node {
                Node::Leaf(_) => 0,
                Node::Branch { height, .. } => *height,
            };
            let id = nodes.len();
            nodes.push(node);
            (id, height)
        }
        let mut nodes = Vec::new();
        let (root, _) = visit(expr, &mut nodes, &mut 0);
        Self { nodes, root }
    }
}

struct Prepared {
    trees: Vec<Tree>,
    degree: usize,
    colors: usize,
    arrows: usize,
    labels: HashMap<Label, usize>,
    compatible: Vec<Vec<bool>>,
    precedences: Vec<(usize, usize)>,
}

fn line(labels: impl IntoIterator<Item = Label>) -> Line {
    let mut result = Line {
        parts: labels
            .into_iter()
            .map(|label| Part {
                group: Group::from(vec![label]),
                gtype: GroupType::Many(1),
            })
            .collect(),
    };
    result.normalize();
    result
}

impl Prepared {
    fn new(problem: &Problem, exprs: &[Expr<Label>]) -> Result<Self, String> {
        // Synchronised columns must really be projections of one derivation.
        // A shortened summary expression is not sufficient for this extractor.
        fn check_shape(problem: &Problem, terms: Vec<&Expr<Label>>) -> Result<(), String> {
            if terms.iter().all(|e| matches!(e, Expr::Base(_, false))) {
                let labels = terms.iter().map(|e| match e {
                    Expr::Base(l, _) => *l,
                    _ => unreachable!(),
                });
                if !problem.active.includes_single_line(&line(labels)) {
                    return Err("Certificate has a non-input active leaf configuration".into());
                }
                return Ok(());
            }
            let mut children = [Vec::new(), Vec::new()];
            let mut joins = 0;
            for term in terms {
                match term {
                    Expr::Left(a,b) | Expr::Right(a,b) => {
                        joins += usize::from(matches!(term, Expr::Right(..)));
                        children[0].push(a.as_ref()); children[1].push(b.as_ref());
                    }
                    _ => return Err("Extractor needs the unshortened Original expressions with a common tree shape".into()),
                }
            }
            if joins != 1 {
                return Err("Each derivation column must have exactly one join".into());
            }
            for child in children {
                check_shape(problem, child)?;
            }
            Ok(())
        }
        if exprs.is_empty() || exprs.len() != problem.active.finite_degree() {
            return Err("Wrong certificate degree".into());
        }
        check_shape(problem, exprs.iter().collect())?;
        let labels: Vec<_> = problem.labels();
        let compatible = labels
            .iter()
            .map(|&a| {
                labels
                    .iter()
                    .map(|&b| problem.passive.includes(&line([a, b])))
                    .collect()
            })
            .collect();
        let mut result = Self {
            degree: exprs.len(),
            colors: exprs.len() + 1,
            arrows: exprs[0].number_of_arrows(),
            trees: exprs.iter().map(Tree::new).collect(),
            labels: labels
                .into_iter()
                .enumerate()
                .map(|(i, l)| (l, i))
                .collect(),
            compatible,
            precedences: Vec::new(),
        };
        for color in 0..result.colors {
            for (port, tree) in result.trees.iter().enumerate() {
                for node in &tree.nodes {
                    if let Node::Branch {
                        position, children, ..
                    } = node
                    {
                        for &child in children {
                            if let Node::Branch {
                                position: inner, ..
                            } = tree.nodes[child]
                            {
                                result.precedences.push((
                                    result.event(color, port, inner),
                                    result.event(color, port, *position),
                                ));
                            }
                        }
                    }
                }
            }
            let columns: Vec<_> = exprs.iter().map(Expr::arrows_inorder_visit).collect();
            for position in 0..result.arrows {
                let join = (0..result.degree)
                    .find(|&p| columns[p][position] == Arrow::Right)
                    .unwrap();
                for port in 0..result.degree {
                    if port != join {
                        result.precedences.push((
                            result.event(color, port, position),
                            result.event(color, join, position),
                        ));
                    }
                }
            }
        }
        Ok(result)
    }

    fn event(&self, color: usize, port: usize, position: usize) -> usize {
        (color * self.degree + port) * self.arrows + position
    }

    fn priorities(&self) -> usize {
        self.colors * self.degree * self.arrows
    }

    // Independent concrete evaluator: no SAT literals or order-variable map.
    fn wins(&self, ranks: &[usize], c1: usize, p1: usize, c2: usize, p2: usize) -> bool {
        fn visit(
            p: &Prepared,
            ranks: &[usize],
            sides: [(usize, usize); 2],
            i: usize,
            j: usize,
            memo: &mut [Option<bool>],
        ) -> bool {
            let [(c1, p1), (c2, p2)] = sides;
            let t1 = &p.trees[p1];
            let t2 = &p.trees[p2];
            let key = i * t2.nodes.len() + j;
            if let Some(value) = memo[key] {
                return value;
            }
            let value = match (&t1.nodes[i], &t2.nodes[j]) {
                (Node::Leaf(a), Node::Leaf(b)) => p.compatible[p.labels[a]][p.labels[b]],
                (a, b) => {
                    let first = match (a, b) {
                        (Node::Leaf(_), _) => false,
                        (_, Node::Leaf(_)) => true,
                        (Node::Branch { position: a, .. }, Node::Branch { position: b, .. }) => {
                            ranks[p.event(c1, p1, *a)] > ranks[p.event(c2, p2, *b)]
                        }
                    };
                    let Node::Branch {
                        arrow, children, ..
                    } = (if first { a } else { b })
                    else {
                        unreachable!()
                    };
                    let mut child = |x| {
                        if first {
                            visit(p, ranks, sides, x, j, memo)
                        } else {
                            visit(p, ranks, sides, i, x, memo)
                        }
                    };
                    if *arrow == Arrow::Right {
                        child(children[0]) || child(children[1])
                    } else {
                        child(children[0]) && child(children[1])
                    }
                }
            };
            memo[key] = Some(value);
            value
        }
        let t1 = &self.trees[p1];
        let t2 = &self.trees[p2];
        visit(
            self,
            ranks,
            [(c1, p1), (c2, p2)],
            t1.root,
            t2.root,
            &mut vec![None; t1.nodes.len() * t2.nodes.len()],
        )
    }

    fn verify(&self, ranks: &[usize]) -> Result<(), String> {
        let mut sorted = ranks.to_vec();
        sorted.sort_unstable();
        if sorted != (0..self.priorities()).collect::<Vec<_>>() {
            return Err("Priorities are not a permutation".into());
        }
        if self.precedences.iter().any(|&(a, b)| ranks[a] >= ranks[b]) {
            return Err("Priority order violates a tree/column constraint".into());
        }
        for c1 in 0..self.colors {
            for c2 in c1 + 1..self.colors {
                for p1 in 0..self.degree {
                    for p2 in 0..self.degree {
                        if !self.wins(ranks, c1, p1, c2, p2) {
                            return Err(format!(
                                "Losing edge game: colors {c1},{c2}, ports {p1},{p2}"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn simple_order(&self, variant: usize) -> Vec<usize> {
        let mut events = Vec::new();
        for color in 0..self.colors {
            for (port, tree) in self.trees.iter().enumerate() {
                for node in &tree.nodes {
                    if let Node::Branch {
                        height,
                        arrow,
                        position,
                        ..
                    } = node
                    {
                        let c = if variant & 1 == 0 {
                            color
                        } else {
                            self.colors - 1 - color
                        };
                        let p = if variant & 2 == 0 {
                            port
                        } else {
                            self.degree - 1 - port
                        };
                        let pos = if variant & 4 == 0 {
                            *position
                        } else {
                            self.arrows - 1 - position
                        };
                        let join = usize::from(*arrow == Arrow::Right);
                        let tail = match variant / 8 {
                            0 => [pos, join, c, p],
                            1 => [c, pos, join, p],
                            2 => [join, c, pos, p],
                            _ => [c, join, pos, p],
                        };
                        events.push(((*height, tail), self.event(color, port, *position)));
                    }
                }
            }
        }
        events.sort_unstable();
        let mut ranks = vec![0; self.priorities()];
        for (rank, (_, id)) in events.into_iter().enumerate() {
            ranks[id] = rank;
        }
        ranks
    }
}

struct Encoding {
    solver: Minisat,
    next: u32,
    truth: Lit,
    gates: HashMap<Vec<Lit>, Lit>,
    comparisons: HashMap<(usize, usize), Lit>,
    ordered_pairs: Vec<(usize, usize, Lit)>,
    clauses: usize,
}

impl Encoding {
    fn new() -> Result<Self, String> {
        let mut result = Self {
            solver: Minisat::default(),
            next: 1,
            truth: Lit::new(0, false),
            gates: HashMap::new(),
            comparisons: HashMap::new(),
            ordered_pairs: Vec::new(),
            clauses: 0,
        };
        result
            .solver
            .add_clause([result.truth].into_iter().collect())
            .map_err(|e| e.to_string())?;
        result.clauses = 1;
        Ok(result)
    }
    fn fresh(&mut self) -> Lit {
        let lit = Lit::new(self.next, false);
        self.next += 1;
        lit
    }
    fn clause(&mut self, mut lits: Vec<Lit>) -> Result<(), String> {
        if lits.contains(&self.truth) {
            return Ok(());
        }
        lits.retain(|l| *l != !self.truth);
        lits.sort_unstable();
        lits.dedup();
        if lits.iter().any(|l| lits.binary_search(&!*l).is_ok()) {
            return Ok(());
        }
        self.clauses += 1;
        self.solver
            .add_clause(lits.into_iter().collect())
            .map_err(|e| e.to_string())
    }
    fn or(&mut self, mut lits: Vec<Lit>) -> Result<Lit, String> {
        if lits.contains(&self.truth) {
            return Ok(self.truth);
        }
        lits.retain(|l| *l != !self.truth);
        lits.sort_unstable();
        lits.dedup();
        if lits.iter().any(|l| lits.binary_search(&!*l).is_ok()) {
            return Ok(self.truth);
        }
        match lits.len() {
            0 => return Ok(!self.truth),
            1 => return Ok(lits[0]),
            _ => {}
        }
        if let Some(&lit) = self.gates.get(&lits) {
            return Ok(lit);
        }
        let result = self.fresh();
        for &lit in &lits {
            self.clause(vec![!lit, result])?;
        }
        self.clause(lits.iter().copied().chain([!result]).collect())?;
        self.gates.insert(lits, result);
        Ok(result)
    }
    fn and(&mut self, lits: Vec<Lit>) -> Result<Lit, String> {
        Ok(!self.or(lits.into_iter().map(|l| !l).collect())?)
    }
    fn branch(&mut self, arrow: Arrow, a: Lit, b: Lit) -> Result<Lit, String> {
        if arrow == Arrow::Right {
            self.or(vec![a, b])
        } else {
            self.and(vec![a, b])
        }
    }
    fn less(&mut self, a: usize, b: usize) -> Lit {
        if a == b {
            return !self.truth;
        }
        if a > b {
            return !self.less(b, a);
        }
        if let Some(&lit) = self.comparisons.get(&(a, b)) {
            return lit;
        }
        let lit = self.fresh();
        self.comparisons.insert((a, b), lit);
        self.ordered_pairs.push((a, b, lit));
        lit
    }
    fn game(
        &mut self,
        p: &Prepared,
        c1: usize,
        p1: usize,
        c2: usize,
        p2: usize,
    ) -> Result<Lit, String> {
        let t1 = &p.trees[p1];
        let t2 = &p.trees[p2];
        let width = t2.nodes.len();
        let mut values = vec![!self.truth; t1.nodes.len() * width];
        for (i, a) in t1.nodes.iter().enumerate() {
            for (j, b) in t2.nodes.iter().enumerate() {
                let left = match a {
                    Node::Leaf(_) => None,
                    Node::Branch {
                        arrow, children, ..
                    } => Some(self.branch(
                        *arrow,
                        values[children[0] * width + j],
                        values[children[1] * width + j],
                    )?),
                };
                let right = match b {
                    Node::Leaf(_) => None,
                    Node::Branch {
                        arrow, children, ..
                    } => Some(self.branch(
                        *arrow,
                        values[i * width + children[0]],
                        values[i * width + children[1]],
                    )?),
                };
                values[i * width + j] = match (a, b, left, right) {
                    (Node::Leaf(a), Node::Leaf(b), _, _) => {
                        if p.compatible[p.labels[a]][p.labels[b]] {
                            self.truth
                        } else {
                            !self.truth
                        }
                    }
                    (_, _, Some(a), None) | (_, _, None, Some(a)) => a,
                    (_, _, Some(a), Some(b)) if a == b => a,
                    (
                        Node::Branch { position: a, .. },
                        Node::Branch { position: b, .. },
                        Some(left),
                        Some(right),
                    ) => {
                        let lt = self.less(p.event(c1, p1, *a), p.event(c2, p2, *b));
                        let first = self.and(vec![!lt, left])?;
                        let second = self.and(vec![lt, right])?;
                        self.or(vec![first, second])?
                    }
                    _ => unreachable!(),
                };
            }
        }
        Ok(values[t1.root * width + t2.root])
    }

    // SAT modulo acyclicity. Only compared events need edges. Acyclic partial
    // orders have total extensions, exactly the old tournament requirement.
    fn order_or_cycles(
        &self,
        model: &Assignment,
        count: usize,
    ) -> Result<Vec<usize>, Vec<Vec<usize>>> {
        let mut edges = vec![Vec::new(); count];
        for &(a, b, lit) in &self.ordered_pairs {
            if model.lit_value(lit) == TernaryVal::True {
                edges[a].push((b, lit));
            } else {
                edges[b].push((a, !lit));
            }
        }
        let mut state = vec![0; count];
        let mut parent = vec![None; count];
        let mut finished = Vec::new();
        let mut cuts = Vec::new();
        for root in 0..count {
            if state[root] != 0 {
                continue;
            }
            state[root] = 1;
            let mut pending = vec![(root, 0)];
            while let Some((a, next)) = pending.last_mut() {
                let a = *a;
                if *next == edges[a].len() {
                    state[a] = 2;
                    finished.push(a);
                    pending.pop();
                    continue;
                }
                let (b, lit) = edges[a][*next];
                *next += 1;
                if state[b] == 0 {
                    state[b] = 1;
                    parent[b] = Some((a, lit));
                    pending.push((b, 0));
                } else if state[b] == 1 {
                    let mut cut = vec![a];
                    let mut node = a;
                    while node != b {
                        let (previous, _) = parent[node].unwrap();
                        cut.push(previous);
                        node = previous;
                    }
                    cut.reverse();
                    cuts.push(cut);
                    if cuts.len() == 256 {
                        return Err(cuts);
                    }
                }
            }
        }
        if !cuts.is_empty() {
            return Err(cuts);
        }
        finished.reverse();
        let mut ranks = vec![0; count];
        for (rank, id) in finished.into_iter().enumerate() {
            ranks[id] = rank;
        }
        Ok(ranks)
    }

    fn exclude_cycles(&mut self, cycles: Vec<Vec<usize>>) -> Result<(), String> {
        for cycle in cycles {
            // Triangulate a directed cycle through shared comparison variables.
            // Unlike a long cycle clause, these implications also propagate
            // useful transitive shortcuts into other edge games.
            let anchor = cycle[0];
            for i in 1..cycle.len() - 1 {
                let ab = self.less(anchor, cycle[i]);
                let bc = self.less(cycle[i], cycle[i + 1]);
                let ac = self.less(anchor, cycle[i + 1]);
                self.clause(vec![!ab, !bc, ac])?;
            }
        }
        Ok(())
    }
}

fn prepare(problem: &Problem, certificate: &str) -> Result<Prepared, String> {
    let terms = crate::algorithms::fixpoint_sat::certificate_cnf::validated_certificate_terms(
        problem,
        certificate,
    )
    .map_err(|e| e.to_string())?;
    let exprs: Vec<_> = terms.iter().map(|t| t.to_expr().as_expr()).collect();
    Prepared::new(problem, &exprs)
}

/// Restore only commutativity/idempotency reductions, then verify the full
/// active derivation and universal certificate. None means budget exhaustion.
/// Saving this text avoids repeating reconstruction when verifying a schedule.
pub fn normalize_certificate(
    problem: &Problem,
    certificate: &str,
    time_limit: Option<Duration>,
    eh: &mut EventHandler,
) -> Result<Option<String>, String> {
    let started = Instant::now();
    eh.notify("Algorithm: reconstructing synchronized certificate", 0, 0);
    let terms = crate::algorithms::fixpoint_sat::certificate_cnf::reconstructed_certificate_terms(
        problem,
        certificate,
        &mut |states| {
            eh.notify(
                "Algorithm: reconstructing synchronized certificate",
                states,
                200_000,
            );
            !eh.is_cancelled() && !time_limit.is_some_and(|limit| started.elapsed() >= limit)
        },
    )
    .map_err(|e| e.to_string())?;
    if eh.is_cancelled() {
        return Err("Algorithm extraction cancelled".into());
    }
    let Some(terms) = terms else { return Ok(None) };
    let exprs: Vec<_> = terms.iter().map(|t| t.to_expr().as_expr()).collect();
    let prepared = Prepared::new(problem, &exprs)?;
    let names: HashMap<_, _> = problem.mapping_label_text.iter().cloned().collect();
    let text = format!(
        "Original expressions:\n{}\n",
        exprs
            .iter()
            .map(|e| e.convert(&names).to_string())
            .join("\n")
    );
    eh.notify(
        "Algorithm: synchronized certificate verified (arrows per expression)",
        prepared.arrows,
        0,
    );
    Ok(Some(text))
}

/// Independently verify a saved algorithm against its certificate and problem.
pub fn verify(problem: &Problem, algorithm: &Algorithm) -> Result<(), String> {
    let p = prepare(problem, &algorithm.certificate)?;
    if algorithm.degree != p.degree
        || algorithm.colors != p.colors
        || algorithm.arrows_per_expression != p.arrows
    {
        return Err("Algorithm dimensions do not match the certificate".into());
    }
    p.verify(&algorithm.ranks)
}

pub fn extract(
    problem: &Problem,
    certificate: &str,
    options: &Options,
    eh: &mut EventHandler,
) -> Result<Outcome, String> {
    let started = Instant::now();
    let Some(normalized) = normalize_certificate(problem, certificate, options.time_limit, eh)?
    else {
        eh.notify(
            "Algorithm: certificate reconstruction budget reached (inconclusive)",
            0,
            0,
        );
        return Ok(Outcome::Inconclusive(Stats {
            elapsed_seconds: started.elapsed().as_secs_f64(),
            ..Default::default()
        }));
    };
    let p = prepare(problem, &normalized)?;
    let mut stats = Stats {
        priorities: p.priorities(),
        games: p.colors * (p.colors - 1) / 2 * p.degree * p.degree,
        ..Default::default()
    };
    let found = |ranks, stats| {
        Outcome::Found(Algorithm {
            certificate: normalized.clone(),
            source_certificate: Some(certificate.into()),
            degree: p.degree,
            colors: p.colors,
            arrows_per_expression: p.arrows,
            ranks,
            stats,
        })
    };
    let expired = || {
        options
            .time_limit
            .is_some_and(|limit| started.elapsed() >= limit)
    };
    let mut preferred = None;
    if options.try_simple_orders {
        for variant in 0..32 {
            if eh.is_cancelled() {
                return Err("Algorithm extraction cancelled".into());
            }
            let ranks = p.simple_order(variant);
            stats.simple_orders += 1;
            let mut wins = 0;
            for c1 in 0..p.colors {
                for c2 in c1 + 1..p.colors {
                    for p1 in 0..p.degree {
                        for p2 in 0..p.degree {
                            wins += usize::from(p.wins(&ranks, c1, p1, c2, p2));
                        }
                    }
                }
            }
            if wins > stats.best_simple_games || preferred.is_none() {
                stats.best_simple_games = wins;
                preferred = Some(ranks.clone());
            }
            if wins == stats.games && p.verify(&ranks).is_ok() {
                stats.elapsed_seconds = started.elapsed().as_secs_f64();
                return Ok(found(ranks, stats));
            }
        }
        eh.notify(
            "Algorithm: best simple schedule, winning games",
            stats.best_simple_games,
            stats.games,
        );
    }
    eh.notify(
        "Algorithm: encoding priority games",
        stats.priorities,
        stats.games,
    );
    let mut encoding = Encoding::new()?;
    for &(a, b) in &p.precedences {
        let lit = encoding.less(a, b);
        encoding.clause(vec![lit])?;
    }
    let mut completed = 0;
    for c1 in 0..p.colors {
        for c2 in c1 + 1..p.colors {
            for p1 in 0..p.degree {
                for p2 in 0..p.degree {
                    if eh.is_cancelled() {
                        return Err("Algorithm extraction cancelled".into());
                    }
                    if expired() {
                        stats.elapsed_seconds = started.elapsed().as_secs_f64();
                        return Ok(Outcome::Inconclusive(stats));
                    }
                    let game = encoding.game(&p, c1, p1, c2, p2)?;
                    encoding.clause(vec![game])?;
                    completed += 1;
                    eh.notify("Algorithm: encoding edge games", completed, stats.games);
                }
            }
        }
    }
    if let Some(ranks) = preferred {
        for &(a, b, lit) in &encoding.ordered_pairs {
            encoding
                .solver
                .phase_lit(if ranks[a] < ranks[b] { lit } else { !lit })
                .map_err(|e| e.to_string())?;
        }
    }
    loop {
        stats.variables = encoding.next;
        stats.clauses = encoding.clauses;
        stats.comparisons = encoding.ordered_pairs.len();
        stats.elapsed_seconds = started.elapsed().as_secs_f64();
        if eh.is_cancelled() {
            return Err("Algorithm extraction cancelled".into());
        }
        if expired() {
            return Ok(Outcome::Inconclusive(stats));
        }
        eh.notify(
            "Algorithm: solving priority SAT, cycle exclusions",
            stats.cycle_cuts,
            0,
        );
        encoding.solver.set_limit(Limit::Conflicts(10_000));
        match encoding.solver.solve().map_err(|e| e.to_string())? {
            SolverResult::Interrupted => continue,
            SolverResult::Unsat => {
                stats.elapsed_seconds = started.elapsed().as_secs_f64();
                return Ok(Outcome::NoSchedule(stats));
            }
            SolverResult::Sat => {}
        }
        stats.sat_models += 1;
        let model = encoding.solver.full_solution().map_err(|e| e.to_string())?;
        match encoding.order_or_cycles(&model, p.priorities()) {
            Ok(ranks) => {
                p.verify(&ranks)?;
                stats.elapsed_seconds = started.elapsed().as_secs_f64();
                eh.notify(
                    "Algorithm: priority schedule independently verified",
                    stats.games,
                    stats.games,
                );
                return Ok(found(ranks, stats));
            }
            Err(cuts) => {
                stats.cycle_cuts += cuts.len();
                encoding.exclude_cycles(cuts)?;
            }
        }
    }
}

#[cfg(test)]
mod tests;
