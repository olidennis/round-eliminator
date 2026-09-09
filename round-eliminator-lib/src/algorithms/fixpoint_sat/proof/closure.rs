//! Bounded deterministic certificate accelerator, independent of diagram SAT.
//!
//! Bootstrap by ordinary finite saturation; then repeatedly combine with each
//! bootstrap configuration whose terms form a chain in the universal order.
//! Finally saturate using compatibility profiles against subterms of promising
//! proved configurations. Profiles are search heuristics, NOT proof equalities.
//! Every result is replayed from original active configurations and checked by
//! the unchanged nonexistence oracle before publication.

use super::*;
use itertools::Itertools;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

mod algebra;
use algebra::{Algebra, Id, JoinMeet, Row};

const BUDGET: &str = "Certificate closure budget reached";
const TIME_LIMIT: Duration = Duration::from_secs(60);
const MAX_PROOFS: usize = 100_000;
const MAX_ACTIVE: usize = 4_000;
const MAX_EXPANDED: usize = 4_096;

#[derive(Clone)]
struct Parents {
    ids: [usize; 2],
    permutation: Row,
    pivot: usize,
    // Sorted output coordinate -> unsorted left-parent coordinate.
    order: Row,
}

#[derive(Clone)]
struct Proven {
    terms: Row,
    parents: Option<Parents>,
    depth: usize,
    size: usize,
    score: usize,
}

struct Engine<'a> {
    algebra: Algebra,
    labels: Vec<Label>,
    degree: usize,
    proofs: Vec<Proven>,
    ids: HashMap<Row, usize>,
    originals: Row,
    active: Row,
    seen: HashSet<Row>,
    permutations: Vec<Row>,
    finite: bool,
    use_observer: bool,
    best: usize,
    views: Vec<(usize, usize)>,
    found: Option<usize>,
    control: &'a SearchControl,
    deadline: Instant,
}

impl<'a> Engine<'a> {
    fn new(original: &Problem, control: &'a SearchControl) -> Result<Option<Self>, String> {
        let labels = original.labels().into_iter().sorted().collect_vec();
        let Degree::Finite(degree) = original.active.degree else {
            return Ok(None);
        };
        // This optional factorial-time accelerator never limits other workers.
        if labels.len() > 32 || !(1..=5).contains(&degree) {
            return Ok(None);
        }
        // Bound expansion BEFORE all_choices allocates the grouped inputs.
        let choices = original.active.lines.iter().fold(0usize, |sum, line| {
            sum.saturating_add(line.parts.iter().fold(1usize, |count, part| {
                count.saturating_mul(part.group.len().saturating_pow(part.gtype.value() as u32))
            }))
        });
        if choices > 512 {
            return Ok(None);
        }
        let mut oracle = NonexistenceOracle::new(original);
        let mut order = vec![0u32; labels.len()];
        let mut adjacent = order.clone();
        for (a, &x) in labels.iter().enumerate() {
            for (b, &y) in labels.iter().enumerate() {
                if oracle.terms_precede(&Term::Terminal(x), &Term::Terminal(y)) {
                    order[a] |= 1 << b;
                }
                if oracle.atomic_compatibility(x, y) {
                    adjacent[a] |= 1 << b;
                }
            }
        }
        let mut engine = Self {
            algebra: Algebra::new(order, adjacent),
            labels,
            degree,
            proofs: Vec::new(),
            ids: HashMap::new(),
            originals: Vec::new(),
            active: Vec::new(),
            seen: HashSet::new(),
            permutations: (0..degree).permutations(degree).collect(),
            finite: true,
            use_observer: false,
            best: 0,
            views: Vec::new(),
            found: None,
            control,
            deadline: Instant::now() + TIME_LIMIT,
        };
        for input in input_terms(original) {
            control.check()?;
            let terms = input
                .iter()
                .map(|t| {
                    let Term::Terminal(l) = t else { unreachable!() };
                    engine.labels.binary_search(l).unwrap()
                })
                .collect();
            engine.add(Proven {
                terms,
                parents: None,
                depth: 0,
                size: degree,
                score: 0,
            })?;
        }
        engine.originals = engine.active.clone();
        Ok(Some(engine))
    }

    fn check(&self) -> Result<(), String> {
        self.control.check()?;
        if Instant::now() >= self.deadline
            || self.proofs.len() >= MAX_PROOFS
            || self.active.len() >= MAX_ACTIVE
            || self.algebra.over_budget()
        {
            return Err(BUDGET.into());
        }
        Ok(())
    }

    fn value(&mut self, t: Id) -> Id {
        if self.use_observer {
            self.algebra.observe(t)
        } else {
            self.algebra.terms[t].default as Id
        }
    }

    fn value_operation(&mut self, a: Id, b: Id, op: JoinMeet) -> Id {
        if self.use_observer {
            self.algebra.observer.operation(a, b, op)
        } else if op == JoinMeet::Join {
            a & b
        } else {
            a | b
        }
    }

    fn key(&mut self, row: &[Id]) -> Row {
        let mut row = row.iter().map(|&t| self.value(t)).collect_vec();
        row.sort_unstable();
        row
    }

    fn dominates(&mut self, a: &[Id], b: &[Id], universal: bool) -> bool {
        // An occurrence-preserving perfect matching, not set inclusion.
        let mut choices = Vec::with_capacity(self.degree);
        for &x in b {
            let mut mask = 0u32;
            for (j, &y) in a.iter().enumerate() {
                let yes = if self.finite && !universal {
                    let (x, y) = (self.value(x), self.value(y));
                    if self.use_observer {
                        self.algebra.observer.precedes(x, y)
                    } else {
                        x & y == y
                    }
                } else {
                    self.algebra.precedes(x, y)
                };
                if yes {
                    mask |= 1 << j;
                }
            }
            if mask == 0 {
                return false;
            }
            choices.push(mask);
        }
        choices.sort_by_key(|m| m.count_ones());
        fn matching(choices: &[u32], used: u32) -> bool {
            let Some((&first, rest)) = choices.split_first() else {
                return true;
            };
            let mut available = first & !used;
            while available != 0 {
                let bit = 1 << available.trailing_zeros();
                available ^= bit;
                if matching(rest, used | bit) {
                    return true;
                }
            }
            false
        }
        matching(&choices, 0)
    }

    fn retain_maximal(&mut self, row: &[Id]) -> bool {
        for i in 0..self.active.len() {
            let old = self.proofs[self.active[i]].terms.clone();
            if self.dominates(&old, row, false) {
                return false;
            }
        }
        let mut retained = Vec::new();
        for i in 0..self.active.len() {
            let id = self.active[i];
            let old = self.proofs[id].terms.clone();
            if !self.dominates(row, &old, false) {
                retained.push(id);
            }
        }
        self.active = retained;
        true
    }

    fn add(&mut self, mut proof: Proven) -> Result<(), String> {
        self.check()?;
        if proof.size > MAX_EXPANDED || proof.depth > 16 {
            return Ok(());
        }
        let mut order = (0..self.degree).collect_vec();
        order.sort_by_key(|&i| proof.terms[i]);
        proof.terms = order.iter().map(|&i| proof.terms[i]).collect();
        if let Some(p) = &mut proof.parents {
            p.order = order;
        }
        let row = proof.terms.clone();
        let key = if self.finite {
            self.key(&row)
        } else {
            row.clone()
        };
        if !self.seen.insert(key) {
            return Ok(());
        }
        if !self.finite && !self.retain_maximal(&row) {
            return Ok(());
        }
        for i in 0..self.degree {
            for j in i..self.degree {
                proof.score += usize::from(self.algebra.compatible(row[i], row[j]));
            }
        }
        self.best = self.best.max(proof.score);
        let cert = proof.score == self.degree * (self.degree + 1) / 2;
        let id = if self.finite {
            if let Some(&id) = self.ids.get(&row) {
                id
            } else {
                let id = self.proofs.len();
                self.proofs.push(proof);
                self.ids.insert(row.clone(), id);
                self.consider(id)?;
                id
            }
        } else {
            // Keep the local universal antichain's provenance independently of
            // previous closures; only the per-pass `seen` set suppresses it.
            let id = self.proofs.len();
            self.proofs.push(proof);
            self.consider(id)?;
            id
        };
        if cert {
            self.found = Some(id);
        }
        if self.finite && !self.retain_maximal(&row) {
            return Ok(());
        }
        self.active.push(id);
        Ok(())
    }

    fn combine_pair(&mut self, a: usize, b: usize) -> Result<(), String> {
        self.check()?;
        let (left, right) = (self.proofs[a].terms.clone(), self.proofs[b].terms.clone());
        let depth = 1 + self.proofs[a].depth.max(self.proofs[b].depth);
        let size = self.degree + self.proofs[a].size + self.proofs[b].size;
        if depth > 16 || size > MAX_EXPANDED {
            return Ok(());
        }
        for index in 0..self.permutations.len() {
            let perm = self.permutations[index].clone();
            let mut base = Vec::new();
            let mut meets = Vec::new();
            let mut joins = Vec::new();
            if self.finite {
                for k in 0..self.degree {
                    let (x, y) = (self.value(left[k]), self.value(right[perm[k]]));
                    meets.push(self.value_operation(x, y, JoinMeet::Meet));
                    joins.push(self.value_operation(x, y, JoinMeet::Join));
                }
            }
            for pivot in 0..self.degree {
                if self.finite {
                    let mut key = meets.clone();
                    key[pivot] = joins[pivot];
                    key.sort_unstable();
                    if self.seen.contains(&key) {
                        continue;
                    }
                }
                if base.is_empty() {
                    for k in 0..self.degree {
                        base.push(
                            self.algebra
                                .operation(left[k], right[perm[k]], JoinMeet::Meet),
                        );
                    }
                }
                let mut row = base.clone();
                row[pivot] =
                    self.algebra
                        .operation(left[pivot], right[perm[pivot]], JoinMeet::Join);
                self.add(Proven {
                    terms: row,
                    parents: Some(Parents {
                        ids: [a, b],
                        permutation: perm.clone(),
                        pivot,
                        order: Vec::new(),
                    }),
                    depth,
                    size,
                    score: 0,
                })?;
                if self.found.is_some() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn saturate(&mut self) -> Result<(), String> {
        let mut done = HashSet::new();
        loop {
            self.check()?;
            if self.found.is_some() {
                return Ok(());
            }
            let mut frontier = self.active.clone();
            frontier.sort_unstable();
            let mut changed = false;
            for (j, &b) in frontier.iter().enumerate() {
                for &a in &frontier[..=j] {
                    if done.insert((a, b)) {
                        changed = true;
                        self.combine_pair(a, b)?;
                    }
                    if self.found.is_some() {
                        return Ok(());
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }

    /// Exact sufficient final-step test. Compatibility out of a meet is AND,
    /// so all nonpivot terms must be a clique, regardless of their matching.
    fn consider(&mut self, id: usize) -> Result<(), String> {
        if self.found.is_some() {
            return Ok(());
        }
        let left = self.proofs[id].terms.clone();
        let mut pivots = HashSet::new();
        for pivot in 0..self.degree {
            if !pivots.insert(left[pivot]) {
                continue;
            }
            let rest = (0..self.degree).filter(|&i| i != pivot).collect_vec();
            if rest.iter().any(|&i| {
                rest.iter()
                    .any(|&j| !self.algebra.compatible(left[i], left[j]))
            }) {
                continue;
            }
            for v in 0..self.views.len() {
                if v % 128 == 0 {
                    self.check()?;
                }
                let (other, other_pivot) = self.views[v];
                let right = self.proofs[other].terms.clone();
                let remaining = (0..self.degree).filter(|&i| i != other_pivot).collect_vec();
                if rest.iter().any(|&i| {
                    remaining
                        .iter()
                        .any(|&j| !self.algebra.compatible(left[i], right[j]))
                }) {
                    continue;
                }
                let join = self
                    .algebra
                    .operation(left[pivot], right[other_pivot], JoinMeet::Join);
                if !self.algebra.compatible(join, join)
                    || rest
                        .iter()
                        .any(|&i| !self.algebra.compatible(join, left[i]))
                    || remaining
                        .iter()
                        .any(|&i| !self.algebra.compatible(join, right[i]))
                {
                    continue;
                }
                let size = self.degree + self.proofs[id].size + self.proofs[other].size;
                if size > MAX_EXPANDED {
                    continue;
                }
                let mut perm = vec![0; self.degree];
                perm[pivot] = other_pivot;
                let mut row = left.clone();
                row[pivot] = join;
                for (&i, &j) in rest.iter().zip(&remaining) {
                    perm[i] = j;
                    row[i] = self.algebra.operation(left[i], right[j], JoinMeet::Meet);
                }
                let mut order = (0..self.degree).collect_vec();
                order.sort_by_key(|&i| row[i]);
                let terms = order.iter().map(|&i| row[i]).collect();
                let depth = 1 + self.proofs[id].depth.max(self.proofs[other].depth);
                self.found = Some(self.proofs.len());
                self.proofs.push(Proven {
                    terms,
                    parents: Some(Parents {
                        ids: [id, other],
                        permutation: perm,
                        pivot,
                        order,
                    }),
                    depth,
                    size,
                    score: self.degree * (self.degree + 1) / 2,
                });
                return Ok(());
            }
            self.views.push((id, pivot));
        }
        Ok(())
    }

    fn chain_closures(&mut self, eh: &mut EventHandler) -> Result<(), String> {
        self.saturate()?;
        if self.found.is_some() {
            return Ok(());
        }
        self.finite = false;
        self.active.clear();
        self.seen.clear();
        for id in 0..self.proofs.len() {
            self.check()?;
            let row = self.proofs[id].terms.clone();
            if self.retain_maximal(&row) {
                self.active.push(id);
            }
        }
        let bootstrap = self.active.clone();
        let mut hubs = Vec::new();
        for &id in &bootstrap {
            let row = self.proofs[id].terms.clone();
            if row.iter().all(|&a| {
                row.iter()
                    .all(|&b| self.algebra.precedes(a, b) || self.algebra.precedes(b, a))
            }) {
                hubs.push(id);
            }
        }
        hubs.sort_by_key(|&id| std::cmp::Reverse(self.proofs[id].score));
        let total = hubs.len();
        for (index, b) in hubs.into_iter().enumerate() {
            eh.notify("Proof: closure reusable-fragment saturation", index, total);
            self.active = bootstrap.clone();
            self.seen = self
                .active
                .iter()
                .map(|&id| self.proofs[id].terms.clone())
                .collect();
            let mut done = HashSet::new();
            for _ in 0..8 {
                let frontier = self.active.clone();
                let mut changed = false;
                for a in frontier {
                    if done.insert(a) {
                        changed = true;
                        self.combine_pair(a, b)?;
                    }
                    if self.found.is_some() {
                        return Ok(());
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        Ok(())
    }

    fn search(&mut self, eh: &mut EventHandler) -> Result<(), String> {
        self.chain_closures(eh)?;
        if self.found.is_some() {
            return Ok(());
        }
        let mut seeds = (0..self.proofs.len())
            .filter(|&id| self.proofs[id].score + 1 >= self.best)
            .collect_vec();
        seeds.sort_by_key(|&id| std::cmp::Reverse(self.proofs[id].score));
        self.finite = true;
        self.use_observer = true;
        let mut pass = 0;
        let mut observers = HashSet::new();
        while pass < seeds.len() {
            self.check()?;
            let seed = seeds[pass];
            pass += 1;
            let mut selected = BTreeSet::new();
            for &t in &self.proofs[seed].terms {
                self.algebra.subterms(t, &mut selected);
            }
            selected.extend(0..self.labels.len());
            let selected = selected.into_iter().collect_vec();
            if selected.len() > 128
                || !observers.insert((selected.clone(), self.proofs[seed].terms.clone()))
            {
                continue;
            }
            self.algebra.set_observers(&selected);
            eh.notify("Proof: closure compatibility-profile saturation", pass, 0);
            self.active = self.originals.clone();
            self.active.push(seed);
            self.seen.clear();
            for id in self.active.clone() {
                let row = self.proofs[id].terms.clone();
                let key = self.key(&row);
                self.seen.insert(key);
            }
            let before = self.proofs.len();
            self.saturate()?;
            if self.found.is_some() {
                return Ok(());
            }
            seeds.extend(
                (before..self.proofs.len()).filter(|&id| self.proofs[id].score + 1 >= self.best),
            );
        }
        Ok(())
    }

    fn derivation(&self, root: usize) -> Result<guided::Derivation, String> {
        fn append(
            e: &Engine<'_>,
            id: usize,
            ids: &mut HashMap<usize, usize>,
            dag: &mut guided::Derivation,
        ) -> Result<(), String> {
            e.control.check()?;
            if ids.contains_key(&id) {
                return Ok(());
            }
            let p = &e.proofs[id];
            let step = if let Some(source) = &p.parents {
                for &parent in &source.ids {
                    append(e, parent, ids, dag)?;
                }
                guided::Step::Combine {
                    parents: [ids[&source.ids[0]], ids[&source.ids[1]]],
                    permutations: [
                        source.order.clone(),
                        source
                            .order
                            .iter()
                            .map(|&i| source.permutation[i])
                            .collect(),
                    ],
                    pivot: source
                        .order
                        .iter()
                        .position(|&i| i == source.pivot)
                        .unwrap(),
                }
            } else {
                guided::Step::Input(p.terms.iter().map(|&t| e.labels[t]).collect())
            };
            ids.insert(id, dag.steps.len());
            dag.steps.push(step);
            Ok(())
        }
        let mut dag = guided::Derivation::default();
        append(self, root, &mut HashMap::new(), &mut dag)?;
        Ok(dag)
    }
}

pub(crate) fn run(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
) -> Result<CertificateSearchOutcome, String> {
    control.check()?;
    // A bounded grammar request must not silently search bigger derivations.
    if options.max_steps.is_some() {
        return Ok(CertificateSearchOutcome::Inconclusive { steps: 0 });
    }
    let Some(mut engine) = Engine::new(original, control)? else {
        return Ok(CertificateSearchOutcome::Inconclusive { steps: 0 });
    };
    eh.notify("Proof: starting deterministic expression closure", 0, 0);
    match engine.search(eh) {
        Err(error) if error == BUDGET => {
            eh.notify(
                "Proof: closure budget reached (other searches continue)",
                engine.proofs.len(),
                0,
            );
            return Ok(CertificateSearchOutcome::Inconclusive { steps: 0 });
        }
        other => other?,
    }
    if let Some(root) = engine.found {
        let dag = engine.derivation(root)?;
        let replay = dag.replay(&input_terms(original), engine.degree, control)?;
        let terms = replay.last().ok_or("Closure certificate replay is empty")?;
        let certificate = NonexistenceOracle::new(original)
            .check(terms)
            .ok_or("Closure certificate failed independent universal verification")?;
        control.check()?;
        let steps = dag
            .steps
            .iter()
            .filter(|s| matches!(s, guided::Step::Combine { .. }))
            .count();
        eh.notify(
            "Proof: closure certificate independently verified",
            steps,
            0,
        );
        Ok(CertificateSearchOutcome::Found {
            certificate,
            steps,
            shared_lines: 0,
        })
    } else {
        Ok(CertificateSearchOutcome::Inconclusive { steps: 0 })
    }
}

#[cfg(test)]
mod tests;
