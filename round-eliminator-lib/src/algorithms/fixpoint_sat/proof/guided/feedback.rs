//! Bounded, deterministic proof feedback and internal-context repair.
//!
//! Every retained item carries a whole-configuration DAG back to original
//! inputs. Compatibility profiles are scheduling heuristics, NOT identities:
//! several syntactically different proofs with the same profile are retained.
//! Nothing here can establish global UNSAT. The unrestricted worker is separate.

use super::*;
use std::collections::BTreeMap;

const POOL_SIZE: usize = 96;
const PER_PROFILE: usize = 4;
const MAX_DAG: usize = 96;
const SEEDS: usize = 8;
const CONFLICTS: u32 = 2_000;

fn selected(row: &[Lit], model: &Assignment) -> Result<usize, String> {
    let indices: Vec<_> = row
        .iter()
        .enumerate()
        .filter_map(|(i, &l)| (model.lit_value(l) == TernaryVal::True).then_some(i))
        .collect();
    if indices.len() != 1 {
        return Err("Invalid repair model selector".into());
    }
    Ok(indices[0])
}

// Compact a reachable sub-DAG, preserving ports and sharing. In particular,
// do not reconstruct a topology from commutative/idempotent term strings.
fn append(
    dag: &Derivation,
    root: usize,
    out: &mut Derivation,
    intern: &mut HashMap<Step, usize>,
) -> Result<usize, String> {
    if root >= dag.steps.len() {
        return Err("Missing repair proof root".into());
    }
    let mut needed = HashSet::new();
    let mut pending = vec![root];
    while let Some(i) = pending.pop() {
        if !needed.insert(i) {
            continue;
        }
        if let Step::Combine { parents, .. } = &dag.steps[i] {
            if parents.iter().any(|&p| p >= i) {
                return Err("Cyclic repair proof".into());
            }
            pending.extend(parents);
        }
    }
    let mut ids = HashMap::new();
    for (i, step) in dag.steps.iter().enumerate().take(root + 1) {
        if !needed.contains(&i) {
            continue;
        }
        let step = match step {
            Step::Input(v) => Step::Input(v.clone()),
            Step::Combine {
                parents,
                permutations,
                pivot,
            } => Step::Combine {
                parents: parents.map(|p| ids[&p]),
                permutations: permutations.clone(),
                pivot: *pivot,
            },
        };
        let id = if let Some(&id) = intern.get(&step) {
            id
        } else {
            let id = out.steps.len();
            intern.insert(step.clone(), id);
            out.steps.push(step);
            id
        };
        ids.insert(i, id);
    }
    Ok(ids[&root])
}

fn compact(dag: &Derivation, root: usize) -> Result<Derivation, String> {
    let mut out = Derivation::default();
    let id = append(dag, root, &mut out, &mut HashMap::new())?;
    // Reachable, topologically ordered roots are last, except a duplicate
    // idempotent step. Retain an explicit identity in that unusual case.
    if id + 1 != out.steps.len() {
        return compact(&out, id);
    }
    Ok(out)
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Atom {
    Label(Label),
    Expr(bool, usize, usize),
}

// Same acyclic OR/AND recurrence as ProofEncoding; used only for ranking.
// Certificates still go through NonexistenceOracle::check after DAG replay.
struct Compatibility {
    nodes: Vec<Atom>,
    ids: HashMap<Atom, usize>,
    cache: HashMap<(usize, usize), bool>,
    ground: HashMap<(Label, Label), bool>,
    labels: Vec<Label>,
    cancelled: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Compatibility {
    fn new(original: &Problem) -> Self {
        let mut oracle = NonexistenceOracle::new(original);
        let mut labels = original.labels();
        labels.sort_unstable();
        let mut ground = HashMap::new();
        for &a in &labels {
            for &b in &labels {
                ground.insert((a, b), oracle.atomic_compatibility(a, b));
            }
        }
        Self {
            nodes: Vec::new(),
            ids: HashMap::new(),
            cache: HashMap::new(),
            ground,
            labels,
            cancelled: None,
        }
    }

    fn intern(&mut self, term: &Term) -> usize {
        let key = match term {
            Term::Terminal(l) => Atom::Label(*l),
            Term::Expr(a, b, op) => {
                let (a, b) = (self.intern(a), self.intern(b));
                if a == b {
                    return a;
                }
                Atom::Expr(*op == Operation::Union, a.min(b), a.max(b))
            }
        };
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(key);
        self.ids.insert(key, id);
        id
    }

    fn compatible(&mut self, a: usize, b: usize) -> bool {
        // An interrupted ranking computation is discarded at its caller's
        // checkpoint. It must not postpone GUI STOP behind a large recursion.
        if self
            .cancelled
            .as_ref()
            .is_some_and(|t| t.load(std::sync::atomic::Ordering::Relaxed))
        {
            return false;
        }
        let key = (a.min(b), a.max(b));
        if let Some(&value) = self.cache.get(&key) {
            return value;
        }
        let value = match (self.nodes[a], self.nodes[b]) {
            (Atom::Label(a), Atom::Label(b)) => self.ground[&(a, b)],
            _ => self.decompose(a, b) || self.decompose(b, a),
        };
        if self.cache.len() < 250_000 {
            self.cache.insert(key, value);
        }
        value
    }

    fn decompose(&mut self, a: usize, b: usize) -> bool {
        match self.nodes[a] {
            Atom::Label(_) => false,
            Atom::Expr(true, x, y) => self.compatible(x, b) || self.compatible(y, b),
            Atom::Expr(false, x, y) => self.compatible(x, b) && self.compatible(y, b),
        }
    }

    fn profile(&mut self, terms: &[Term], depth: usize) -> (Vec<usize>, usize) {
        if self.nodes.len() > 32_000 || self.cache.len() > 200_000 {
            self.nodes.clear();
            self.ids.clear();
            self.cache.clear();
        }
        let mut ordered = terms.to_vec();
        ordered.sort();
        let ids: Vec<_> = ordered.iter().map(|t| self.intern(t)).collect();
        let probes: Vec<_> = self
            .labels
            .clone()
            .into_iter()
            .map(|l| self.intern(&Term::Terminal(l)))
            .collect();
        let mut profile = vec![depth.min(12)];
        let mut score = 0;
        for (i, &a) in ids.iter().enumerate() {
            for &b in &ids[..=i] {
                let c = usize::from(self.compatible(a, b));
                score += c;
                profile.push(c);
            }
            for &b in &probes {
                profile.push(usize::from(self.compatible(a, b)));
            }
        }
        (profile, score)
    }
}

struct Fragment {
    dag: Derivation,
    terms: Vec<Term>,
    key: Vec<Term>,
    profile: Vec<usize>,
    score: usize,
    size: usize,
    pinned: bool,
}

pub(super) struct Engine<'a> {
    pool: BTreeMap<usize, Fragment>,
    next_id: usize,
    turn: usize,
    inputs: Vec<Vec<Term>>,
    compatibility: Compatibility,
    pub(super) accepted: usize,
    pub(super) repairs: usize,
    max_score: usize,
    control: &'a SearchControl,
    saved: VecDeque<Saved<'a>>,
    default_seed: Option<(Derivation, Vec<(usize, Vec<Term>)>, usize)>,
}

struct Saved<'a> {
    job: Job<'a>,
    score: usize,
    turn: usize,
    conflicts: u32,
}

struct Recipe {
    seeds: Vec<Vec<Term>>,
    provenance: Vec<Derivation>,
    template: Option<Derivation>,
    holes: Vec<usize>,
    steps: usize,
}

pub(super) struct Task<'a> {
    job: Option<Job<'a>>,
    recipe: Recipe,
    score: usize,
    turn: usize,
    budget: u32,
}

pub(super) struct Completed<'a> {
    job: Option<Job<'a>>,
    found: Vec<(Derivation, bool)>,
    score: usize,
    turn: usize,
    budget: u32,
}

impl<'a> Task<'a> {
    pub(super) fn execute(
        self,
        original: &Problem,
        worker: usize,
        control: &'a SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Completed<'a>, String> {
        let mut done = Completed {
            job: None,
            found: Vec::new(),
            score: self.score,
            turn: self.turn,
            budget: self.budget,
        };
        let mut job = if let Some(job) = self.job {
            job
        } else {
            let recipe = self.recipe;
            let Some(job) = Job::build(
                original,
                &recipe.seeds,
                recipe.provenance,
                recipe.template.as_ref(),
                &recipe.holes,
                recipe.steps,
                control,
            )?
            else {
                eh.notify("Proof: feedback neighborhood too large (skipped)", 0, 0);
                return Ok(done);
            };
            job
        };
        job.encoding.circuit.worker = worker;
        done.found = match job.solve(self.score, self.turn, self.budget, control, eh) {
            Err(e) if e == "Proof neighborhood variable budget reached" => {
                eh.notify("Proof: feedback goal budget reached (skipped)", 0, 0);
                return Ok(done);
            }
            result => result?,
        };
        done.job = Some(job);
        Ok(done)
    }
}

impl<'a> Engine<'a> {
    pub(super) fn new(original: &Problem, control: &'a SearchControl) -> Result<Self, String> {
        let inputs = input_terms(original);
        let mut engine = Self {
            pool: BTreeMap::new(),
            next_id: 0,
            turn: 0,
            inputs: inputs.clone(),
            compatibility: Compatibility::new(original),
            accepted: 0,
            repairs: 0,
            max_score: 0,
            control,
            saved: VecDeque::new(),
            default_seed: None,
        };
        if inputs.first().is_some_and(|t| t.len() > 6) {
            return Ok(engine);
        }
        for terms in inputs {
            let labels = terms
                .iter()
                .map(|t| match t {
                    Term::Terminal(l) => *l,
                    _ => unreachable!(),
                })
                .collect();
            engine.retain(
                Derivation {
                    steps: vec![Step::Input(labels)],
                },
                terms,
                true,
            );
        }
        Ok(engine)
    }

    fn retain(&mut self, dag: Derivation, terms: Vec<Term>, pinned: bool) -> bool {
        if dag.steps.len() > MAX_DAG {
            return false;
        }
        let mut key = terms.clone();
        key.sort();
        if self.pool.values().any(|f| f.key == key) {
            return false;
        }
        let mut depths: Vec<usize> = Vec::new();
        for step in &dag.steps {
            depths.push(match step {
                Step::Input(_) => 0,
                Step::Combine { parents, .. } => 1 + depths[parents[0]].max(depths[parents[1]]),
            });
        }
        let (profile, score) = self.compatibility.profile(&terms, *depths.last().unwrap());
        let size = terms.iter().map(tree_size).sum();
        let same: Vec<_> = self
            .pool
            .iter()
            .filter(|(_, f)| !f.pinned && f.profile == profile)
            .map(|(&id, _)| id)
            .collect();
        let evict = if same.len() >= PER_PROFILE {
            // Keep the smallest representative, rotate the other syntactically
            // distinct representatives. Profile equality is never term equality.
            let smallest = *same.iter().min_by_key(|&&i| self.pool[&i].size).unwrap();
            same.into_iter().find(|&id| id != smallest)
        } else if self.pool.len() >= POOL_SIZE {
            // Alternate breadth/age and score; do not require monotone progress.
            if self.accepted % 2 == 0 {
                self.pool.iter().find(|(_, f)| !f.pinned).map(|(&id, _)| id)
            } else {
                self.pool
                    .iter()
                    .filter(|(_, f)| !f.pinned)
                    .min_by_key(|(&id, f)| (f.score, std::cmp::Reverse(f.size), id))
                    .map(|(&id, _)| id)
            }
        } else {
            None
        };
        if let Some(id) = evict {
            self.pool.remove(&id);
        }
        if self.pool.len() >= POOL_SIZE {
            return false;
        }
        self.pool.insert(
            self.next_id,
            Fragment {
                dag,
                terms,
                key,
                profile,
                score,
                size,
                pinned,
            },
        );
        self.next_id += 1;
        self.accepted += usize::from(!pinned);
        self.max_score = self.max_score.max(score);
        true
    }

    pub(super) fn import(
        &mut self,
        dag: &Derivation,
        control: &SearchControl,
    ) -> Result<(), String> {
        if self.inputs.is_empty() || self.inputs[0].len() > 6 {
            return Ok(());
        }
        self.compatibility.cancelled = Some(control.cancelled.clone());
        let values = dag.replay_indexed(&self.inputs, self.inputs[0].len(), control)?;
        // Import at most 24 evenly distributed reachable fragments per hint;
        // avoid letting one large game consume the entire scheduling slice.
        let stride = (values.len() / 24).max(1);
        for i in (0..values.len()).rev().step_by(stride).take(24) {
            control.check()?;
            if let Some(terms) = &values[i] {
                self.retain(compact(dag, i)?, terms.clone(), false);
                control.check()?;
            }
        }
        Ok(())
    }

    pub(super) fn seed_default(
        &mut self,
        dag: &Derivation,
        control: &SearchControl,
    ) -> Result<(), String> {
        let values = dag.replay_indexed(&self.inputs, self.inputs[0].len(), control)?;
        let roots: Vec<_> = values
            .into_iter()
            .enumerate()
            .filter_map(|(i, terms)| {
                if matches!(dag.steps[i], Step::Input(_)) {
                    None
                } else {
                    terms.map(|t| (i, t))
                }
            })
            .collect();
        if !roots.is_empty() {
            self.default_seed = Some((dag.clone(), roots, 0));
        }
        Ok(())
    }

    fn rotate_default_seed(&mut self, control: &SearchControl) -> Result<(), String> {
        // Keep all bootstrap roots outside the rotating working pool. Revisit
        // them in small slices, not a one-time sample of 24 from a large DAG.
        let Some((dag, roots, cursor)) = self.default_seed.as_mut() else {
            return Ok(());
        };
        let mut next = Vec::new();
        for _ in 0..roots.len().min(4) {
            control.check()?;
            let (root, terms) = &roots[*cursor % roots.len()];
            next.push((compact(dag, *root)?, terms.clone()));
            *cursor = (*cursor + 1) % roots.len();
        }
        self.compatibility.cancelled = Some(control.cancelled.clone());
        for (dag, terms) in next {
            self.retain(dag, terms, false);
        }
        Ok(())
    }

    pub(super) fn enabled(&self, options: &CertificateSearchOptions) -> bool {
        // Bounded API calls must finish even when feedback keeps generating
        // new fragments. This is an accelerator work cap, not an UNSAT bound.
        self.inputs.first().is_some_and(|t| t.len() <= 6)
            && options.max_steps != Some(0)
            && options
                .max_steps
                .map_or(true, |n| self.turn < n.saturating_mul(8))
    }

    pub(super) fn next_task(
        &mut self,
        options: &CertificateSearchOptions,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Option<Task<'a>>, String> {
        if !self.enabled(options) {
            return Ok(self.next_cached(0, eh));
        }
        control.check()?;
        self.rotate_default_seed(control)?;
        let turn = self.turn;
        self.turn += 1;
        if turn % 2 == 0 {
            if let Some(task) = self.next_cached(0, eh) {
                return Ok(Some(task));
            }
        }
        let derived: Vec<_> = self
            .pool
            .iter()
            .filter(|(_, f)| !f.pinned)
            .map(|(&id, _)| id)
            .collect();
        let repair = turn % 3 == 1 && !derived.is_empty();
        let template_id = if repair {
            if (turn / 3) % 3 != 1 {
                let mut ranked = derived.clone();
                ranked.sort_by_key(|id| {
                    (
                        std::cmp::Reverse(self.pool[id].score),
                        self.pool[id].size,
                        *id,
                    )
                });
                Some(ranked[(turn / 6) % ranked.len().min(8)])
            } else {
                Some(derived[(turn / 3) % derived.len()])
            }
        } else {
            None
        };
        let mut seeds: Vec<_> = self
            .pool
            .iter()
            .filter(|(_, f)| f.pinned)
            .map(|(&id, _)| id)
            .take(SEEDS)
            .collect();
        if !derived.is_empty() {
            if turn % 3 == 0 && seeds.len() < SEEDS {
                if let Some(&id) = derived
                    .iter()
                    .filter(|&&id| Some(id) != template_id)
                    .min_by_key(|&&id| {
                        (std::cmp::Reverse(self.pool[&id].score), self.pool[&id].size)
                    })
                {
                    seeds.push(id);
                }
            }
            // Rotate through the whole pool, including low-scoring fragments.
            for offset in 0..derived.len() {
                let id = derived[(turn + offset) % derived.len()];
                if Some(id) != template_id && !seeds.contains(&id) {
                    seeds.push(id);
                }
                if seeds.len() >= SEEDS {
                    break;
                }
            }
        }
        let seed_dags: Vec<_> = seeds.iter().map(|id| self.pool[id].dag.clone()).collect();
        let seed_terms: Vec<_> = seeds.iter().map(|id| self.pool[id].terms.clone()).collect();
        let score = template_id.map_or_else(
            || {
                seeds
                    .iter()
                    .map(|id| self.pool[id].score)
                    .max()
                    .unwrap_or(0)
            },
            |id| self.pool[&id].score,
        );
        let steps = (1 + (turn / 3) % 3).min(options.max_steps.unwrap_or(3));
        let template = template_id.map(|id| self.pool[&id].dag.clone());
        let holes = if let Some(dag) = &template {
            let internal: Vec<_> = dag
                .steps
                .iter()
                .enumerate()
                .filter_map(|(i, s)| matches!(s, Step::Combine { .. }).then_some(i))
                .collect();
            if internal.is_empty() {
                return Ok(None);
            }
            let pos = if (turn / 3) % 3 != 1 {
                internal.len().saturating_sub(steps + (turn / 9) % 3)
            } else {
                (turn / 3) % internal.len()
            };
            let mut holes = vec![internal[pos]];
            if steps > 1 && internal.len() > 1 {
                holes.push(internal[(pos + 1) % internal.len()]);
            }
            if steps > 2 && internal.len() > 2 {
                holes.push(internal[(pos + 2) % internal.len()]);
            }
            self.repairs += 1;
            holes
        } else {
            Vec::new()
        };
        eh.notify(
            if repair {
                "Proof: repairing internal branches"
            } else {
                "Proof: growing reusable fragments"
            },
            if repair { holes.len() } else { steps },
            self.pool.len(),
        );
        let budget = options.conflict_limit.unwrap_or(CONFLICTS).min(CONFLICTS);
        Ok(Some(Task {
            job: None,
            recipe: Recipe {
                seeds: seed_terms,
                provenance: seed_dags,
                template,
                holes,
                steps,
            },
            score,
            turn,
            budget,
        }))
    }

    pub(super) fn saved_variables(&self) -> usize {
        self.saved
            .iter()
            .map(|s| s.job.encoding.circuit.next_var as usize)
            .sum()
    }

    pub(super) fn saved_count(&self) -> usize {
        self.saved.len()
    }

    pub(super) fn next_cached(
        &mut self,
        minimum: usize,
        eh: &mut EventHandler,
    ) -> Option<Task<'a>> {
        let pos = self
            .saved
            .iter()
            .position(|s| s.job.encoding.circuit.next_var as usize >= minimum)?;
        let saved = self.saved.remove(pos).unwrap();
        eh.notify(
            "Proof: retrying cached repair/feedback",
            saved.conflicts as usize,
            self.pool.len(),
        );
        Some(Task {
            job: Some(saved.job),
            score: saved.score,
            turn: saved.turn,
            budget: saved.conflicts,
            // No discarded fresh recipe is built or cloned on a hot retry.
            recipe: Recipe {
                seeds: Vec::new(),
                provenance: Vec::new(),
                template: None,
                holes: Vec::new(),
                steps: 0,
            },
        })
    }

    #[cfg(test)]
    pub(super) fn tick(
        &mut self,
        original: &Problem,
        options: &CertificateSearchOptions,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<(Option<CertificateSearchOutcome>, Vec<Derivation>), String> {
        let Some(task) = self.next_task(options, control, eh)? else {
            return Ok((None, Vec::new()));
        };
        let done = task.execute(original, 2, self.control, eh)?;
        self.accept(done, options, oracle, control, eh)
    }

    pub(super) fn accept(
        &mut self,
        done: Completed<'a>,
        options: &CertificateSearchOptions,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<(Option<CertificateSearchOutcome>, Vec<Derivation>), String> {
        let Completed {
            job: Some(job),
            found,
            score,
            turn: job_turn,
            budget,
        } = done
        else {
            return Ok((None, Vec::new()));
        };
        let mut feedback = Vec::new();
        for (dag, complete) in found {
            let values = dag.replay_indexed(&self.inputs, self.inputs[0].len(), control)?;
            let Some(terms) = values.last().and_then(|v| v.as_ref()) else {
                continue;
            };
            if let Some(certificate) = oracle.check(terms) {
                return Ok((
                    Some(CertificateSearchOutcome::Found {
                        certificate,
                        steps: dag
                            .steps
                            .iter()
                            .filter(|s| matches!(s, Step::Combine { .. }))
                            .count(),
                        shared_lines: job
                            .provenance
                            .iter()
                            .filter(|d| d.steps.iter().any(|s| matches!(s, Step::Combine { .. })))
                            .count(),
                    }),
                    Vec::new(),
                ));
            }
            if complete {
                return Err("Repair SAT disagrees with the nonexistence oracle".into());
            }
            // Feed successful partial SAT derivations back, including useful
            // intermediate nodes. They need not improve their parents' score.
            let before = self.accepted;
            self.import(&dag, control)?;
            if self.accepted > before {
                feedback.push(dag);
            }
        }
        let retry_limit = options.conflict_limit.unwrap_or(32_000).min(32_000);
        // The pool already reserved the whole job's maximum size. Converting
        // that reservation into its actual cached size cannot exceed the shared
        // budget; there is no separate cache cap that discards learned clauses.
        if !job.complete_exhausted && budget < retry_limit {
            self.saved.push_back(Saved {
                job,
                score,
                turn: job_turn,
                conflicts: (budget * 2).min(retry_limit),
            });
        }
        eh.notify(
            format!(
                "Proof: feedback pool (best {}/{})",
                self.max_score,
                self.inputs[0].len() * (self.inputs[0].len() + 1) / 2
            ),
            self.accepted,
            self.pool.len(),
        );
        Ok((None, feedback))
    }
}

struct Job<'a> {
    encoding: ProofEncoding<'a>,
    provenance: Vec<Derivation>,
    root: usize,
    complete_exhausted: bool,
}

impl<'a> Job<'a> {
    fn build(
        original: &Problem,
        seeds: &[Vec<Term>],
        provenance: Vec<Derivation>,
        template: Option<&Derivation>,
        holes: &[usize],
        steps: usize,
        control: &'a SearchControl,
    ) -> Result<Option<Self>, String> {
        match Self::build_inner(original, seeds, provenance, template, holes, steps, control) {
            Err(e) if e == "Proof neighborhood variable budget reached" => Ok(None),
            result => result,
        }
    }

    fn build_inner(
        original: &Problem,
        seeds: &[Vec<Term>],
        mut provenance: Vec<Derivation>,
        template: Option<&Derivation>,
        holes: &[usize],
        steps: usize,
        control: &'a SearchControl,
    ) -> Result<Option<Self>, String> {
        let mut fixed = HashSet::new();
        let mut pending: Vec<_> = seeds.iter().flatten().collect();
        while let Some(t) = pending.pop() {
            if fixed.insert(t) {
                if let Term::Expr(a, b, _) = t {
                    pending.extend([a.as_ref(), b.as_ref()]);
                }
            }
        }
        if fixed.len() > MAX_FIXED_NODES {
            return Ok(None);
        }
        let mut encoding = ProofEncoding::new(original, seeds, control)?;
        encoding.circuit.variable_limit = Some(150_000);
        if let Some(dag) = template {
            if dag.steps.len() > MAX_DAG {
                return Ok(None);
            }
            // Add raw ordered input leaves before any combination. Their
            // provenance is checked by final original-configuration replay.
            let mut input_ids = HashMap::new();
            for step in &dag.steps {
                if let Step::Input(labels) = step {
                    if input_ids.contains_key(labels) {
                        continue;
                    }
                    let terms: Vec<_> = labels.iter().copied().map(Term::Terminal).collect();
                    let nodes = terms
                        .iter()
                        .map(|t| encoding.fixed_term(t))
                        .collect::<Result<Vec<_>, _>>()?;
                    let id = encoding.tuples.len();
                    encoding.tuples.push(Tuple {
                        nodes,
                        source: Source::Leaf(terms),
                        pivot: 0,
                    });
                    provenance.push(Derivation {
                        steps: vec![step.clone()],
                    });
                    input_ids.insert(labels.clone(), id);
                }
            }
            let mut mapped = Vec::new();
            for (i, step) in dag.steps.iter().enumerate() {
                control.check()?;
                let id = match step {
                    Step::Input(labels) => input_ids[labels],
                    Step::Combine {
                        parents,
                        permutations,
                        pivot,
                    } => {
                        if parents.iter().any(|&p| p >= i) {
                            return Err("Invalid repair template".into());
                        }
                        if holes.contains(&i) {
                            encoding.step_at(*pivot)?;
                            encoding.tuples.len() - 1
                        } else {
                            encoding.wired_step(
                                parents.map(|p| mapped[p]),
                                permutations.clone(),
                                *pivot,
                            )?
                        }
                    }
                };
                mapped.push(id);
                if encoding.circuit.next_var > 150_000 {
                    return Ok(None);
                }
            }
            let root = *mapped.last().ok_or("Empty repair template")?;
            Ok(Some(Self {
                encoding,
                provenance,
                root,
                complete_exhausted: false,
            }))
        } else {
            for _ in 0..steps {
                encoding.step()?;
                if encoding.circuit.next_var > 150_000 {
                    return Ok(None);
                }
            }
            let root = encoding.tuples.len() - 1;
            Ok(Some(Self {
                encoding,
                provenance,
                root,
                complete_exhausted: false,
            }))
        }
    }

    fn decode(&self, model: &Assignment) -> Result<Derivation, String> {
        let mut dag = Derivation::default();
        let mut intern = HashMap::new();
        let mut roots = Vec::new();
        for (i, tuple) in self.encoding.tuples.iter().enumerate().take(self.root + 1) {
            self.encoding.circuit.control.check()?;
            let id = match &tuple.source {
                Source::Leaf(_) => {
                    let proof = self
                        .provenance
                        .get(i)
                        .ok_or("Missing repair seed provenance")?;
                    append(proof, proof.steps.len() - 1, &mut dag, &mut intern)?
                }
                Source::Combine(parents) => {
                    let mut ids = [0; 2];
                    let mut perms = [Vec::new(), Vec::new()];
                    for side in 0..2 {
                        let p = selected(&parents[side].tuple, model)?;
                        if p >= roots.len() {
                            return Err("Cyclic repair model".into());
                        }
                        ids[side] = roots[p];
                        perms[side] = parents[side]
                            .permutation
                            .iter()
                            .map(|r| selected(r, model))
                            .collect::<Result<Vec<_>, _>>()?;
                    }
                    let step = Step::Combine {
                        parents: ids,
                        permutations: perms,
                        pivot: tuple.pivot,
                    };
                    if let Some(&id) = intern.get(&step) {
                        id
                    } else {
                        let id = dag.steps.len();
                        intern.insert(step.clone(), id);
                        dag.steps.push(step);
                        id
                    }
                }
            };
            roots.push(id);
        }
        compact(&dag, roots[self.root])
    }

    fn at_least(&mut self, literals: &[Lit], count: usize) -> Result<Lit, String> {
        let truth = self.encoding.circuit.truth;
        let mut dp = vec![!truth; count + 1];
        dp[0] = truth;
        for &lit in literals {
            for k in (1..=count).rev() {
                let with = self.encoding.circuit.and(vec![lit, dp[k - 1]])?;
                dp[k] = self.encoding.circuit.or(vec![dp[k], with])?;
            }
        }
        Ok(dp[count])
    }

    fn solve(
        &mut self,
        score: usize,
        turn: usize,
        conflicts: u32,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Vec<(Derivation, bool)>, String> {
        // Reserve room for the bounded cardinality/partial-goal circuitry.
        self.encoding.circuit.variable_limit = Some(152_000);
        let pairs = self.encoding.pair_goals(self.root);
        eh.notify(
            format!(
                "Proof: repair/feedback SAT ({} variables)",
                self.encoding.circuit.next_var
            ),
            conflicts as usize,
            0,
        );
        let complete = self.encoding.circuit.and(pairs.clone())?;
        let improvement = self.at_least(&pairs, (score + 1).min(pairs.len()))?;
        let exploratory = if turn % 2 == 0 {
            pairs[turn % pairs.len()]
        } else {
            self.at_least(&pairs, score.saturating_sub(1).max(1))?
        };
        let mut results = Vec::new();
        for (index, goal) in [complete, improvement, exploratory].into_iter().enumerate() {
            if index == 0 && self.complete_exhausted {
                continue;
            }
            self.encoding
                .circuit
                .solver
                .set_limit(Limit::Conflicts(i64::from(conflicts)));
            match control.solve(
                self.encoding.circuit.worker,
                &mut self.encoding.circuit.solver,
                Some(&[goal]),
            )? {
                SolverResult::Sat => {
                    let model = self
                        .encoding
                        .circuit
                        .solver
                        .full_solution()
                        .map_err(|e| e.to_string())?;
                    let dag = self.decode(&model)?;
                    let is_complete = pairs
                        .iter()
                        .all(|&p| model.lit_value(p) == TernaryVal::True);
                    eh.notify(
                        if is_complete {
                            "Proof: repair complete model (replaying)"
                        } else {
                            "Proof: retaining partial SAT derivation"
                        },
                        pairs
                            .iter()
                            .filter(|&&p| model.lit_value(p) == TernaryVal::True)
                            .count(),
                        pairs.len(),
                    );
                    results.push((dag, is_complete));
                    if is_complete {
                        break;
                    }
                    // Exclude this assignment's synthesis choices, not its
                    // semantic compatibility profile, to obtain another proof.
                    let mut block = Vec::new();
                    for tuple in self.encoding.tuples.iter().take(self.root + 1) {
                        if let Source::Combine(parents) = &tuple.source {
                            for parent in parents {
                                for row in std::iter::once(&parent.tuple).chain(&parent.permutation)
                                {
                                    let lit = row[selected(row, &model)?];
                                    if lit != self.encoding.circuit.truth {
                                        block.push(!lit);
                                    }
                                }
                            }
                        }
                    }
                    if block.is_empty() {
                        break;
                    }
                    self.encoding.circuit.clause(block)?;
                }
                SolverResult::Unsat => {
                    if index == 0 {
                        self.complete_exhausted = true;
                    }
                }
                SolverResult::Interrupted => {
                    eh.notify("Proof: repair/feedback budget reached (inconclusive)", 0, 0);
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests;
