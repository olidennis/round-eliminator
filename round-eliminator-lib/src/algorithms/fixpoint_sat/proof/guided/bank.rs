//! Bounded fragment storage and lazy, breadth-first cross-block scheduling.
use super::*;

// Circle-method pairings: every round covers every block, and a complete
// sweep covers every unordered pair. No quadratic queue of term snapshots.
struct Sweep {
    blocks: usize,
    round: usize,
    pair: usize,
    revision: u64,
}

impl Sweep {
    fn next(&mut self) -> Option<(usize, usize)> {
        if self.blocks == 0 {
            return None;
        }
        if self.blocks == 1 {
            if self.round != 0 {
                return None;
            }
            self.round = 1;
            return Some((0, 0));
        }
        let n = self.blocks + self.blocks % 2;
        if self.round == n - 1 {
            return None;
        }
        let r = self.round;
        let i = self.pair;
        self.pair += 1;
        if self.pair == n / 2 {
            self.pair = 0;
            self.round += 1;
        }
        let (a, b) = if i == 0 {
            (n - 1, r)
        } else {
            ((r + i) % (n - 1), (r + n - 1 - i) % (n - 1))
        };
        // A dummy partner gives the otherwise unpaired block a solo turn.
        Some(if a == self.blocks { (b, b) } else { (a, b) })
    }
}

pub(super) struct Bank {
    pub(super) tuples: Vec<Vec<Term>>,
    pub(super) known: HashSet<Vec<Term>>,
    pub(super) inputs: usize,
    pub(super) fragment_limit: usize,
    pinned: usize,
    replace_next: usize,
    revision: u64,
    versions: Vec<u64>,
    used: Vec<u64>,
    blocks: Vec<u64>,
    focus: VecDeque<usize>,
    focused: HashSet<usize>,
    focus_turn: usize,
    prefer_focus: bool,
    sweep: Sweep,
    // Only the latest versions of each block pair: bounded by archive size.
    tried: HashMap<(usize, usize), (u64, u64)>,
    pub(super) rotations: usize,
    pub(super) new_selected: usize,
}

impl Bank {
    pub(super) fn new(inputs: Vec<Vec<Term>>) -> Self {
        let n = inputs.len();
        let mut result = Self {
            inputs: n,
            pinned: n,
            replace_next: n,
            known: inputs.iter().cloned().collect(),
            tuples: inputs,
            fragment_limit: MAX_FRAGMENTS,
            revision: 0,
            versions: Vec::new(),
            used: Vec::new(),
            blocks: Vec::new(),
            focus: VecDeque::new(),
            focused: HashSet::new(),
            focus_turn: 0,
            prefer_focus: true,
            sweep: Sweep {
                blocks: 0,
                round: 0,
                pair: 0,
                revision: 0,
            },
            tried: HashMap::new(),
            rotations: 0,
            new_selected: 0,
        };
        for i in 0..n {
            result.changed(i, false);
        }
        result
    }

    fn changed(&mut self, id: usize, urgent: bool) {
        self.revision += 1;
        self.versions.resize(self.tuples.len(), 0);
        self.used.resize(self.tuples.len(), 0);
        self.versions[id] = self.revision;
        let block = id / (BATCH_SIZE / 2);
        self.blocks
            .resize(self.tuples.len().div_ceil(BATCH_SIZE / 2), 0);
        self.blocks[block] = self.revision;
        if urgent && self.focused.insert(block) {
            self.focus.push_back(block);
        }
    }

    fn retain(&mut self, terms: Vec<Term>, urgent: bool) {
        let id = if self.tuples.len() - self.inputs < self.fragment_limit {
            let id = self.tuples.len();
            self.tuples.push(terms.clone());
            id
        } else {
            // Original inputs and the complete bootstrap never rotate out.
            let id = self.replace_next;
            assert!(id >= self.pinned && id < self.tuples.len());
            self.known.remove(&self.tuples[id]);
            self.tuples[id] = terms.clone();
            self.replace_next = if id + 1 == self.tuples.len() {
                self.pinned
            } else {
                id + 1
            };
            self.rotations += 1;
            id
        };
        self.known.insert(terms);
        self.changed(id, urgent);
    }

    fn breadth_pair(&mut self) -> Option<(usize, usize)> {
        loop {
            if let Some(pair) = self.sweep.next() {
                return Some(pair);
            }
            if self.sweep.revision == self.revision {
                return None;
            }
            self.sweep = Sweep {
                blocks: self.blocks.len(),
                round: 0,
                pair: 0,
                revision: self.revision,
            };
        }
    }

    fn focus_pair(&mut self) -> Option<(usize, usize)> {
        let block = self.focus.pop_front()?;
        self.focused.remove(&block);
        let partner = self.focus_turn % self.blocks.len();
        self.focus_turn += 1;
        Some((block, partner))
    }

    pub(super) fn next_batch(
        &mut self,
        control: &SearchControl,
    ) -> Result<Option<Vec<usize>>, String> {
        loop {
            control.check()?;
            let pair = if self.prefer_focus {
                self.focus_pair().or_else(|| self.breadth_pair())
            } else {
                self.breadth_pair().or_else(|| self.focus_pair())
            };
            self.prefer_focus = !self.prefer_focus;
            let Some((a, b)) = pair else {
                return Ok(None);
            };
            let key = (a.min(b), a.max(b));
            let versions = (self.blocks[key.0], self.blocks[key.1]);
            if self.tried.get(&key) == Some(&versions) {
                continue;
            }
            self.tried.insert(key, versions);
            let half = BATCH_SIZE / 2;
            // Process the focus block first when the fixed-term cap forces a
            // smaller batch. Do not let its old partner displace every new term.
            let ids = (a * half..((a + 1) * half).min(self.tuples.len()))
                .chain(b * half..((b + 1) * half).min(self.tuples.len()));
            let mut batch = Vec::new();
            let mut fixed = HashSet::new();
            for id in ids {
                if batch.contains(&id) {
                    continue;
                }
                let mut added = HashSet::new();
                let mut pending: Vec<_> = self.tuples[id].iter().collect();
                while let Some(t) = pending.pop() {
                    control.check()?;
                    if !fixed.contains(t) && added.insert(t.clone()) {
                        if let Term::Expr(a, b, _) = t {
                            pending.extend([a.as_ref(), b.as_ref()]);
                        }
                    }
                }
                if fixed.len() + added.len() <= MAX_FIXED_NODES {
                    fixed.extend(added);
                    batch.push(id);
                }
            }
            if batch.is_empty() {
                continue;
            }
            batch.sort_unstable();
            for &id in &batch {
                if self.used[id] != self.versions[id] {
                    self.new_selected += usize::from(id >= self.pinned);
                    self.used[id] = self.versions[id];
                }
            }
            return Ok(Some(batch));
        }
    }

    pub(super) fn pinned_selected(&self) -> usize {
        (self.inputs..self.pinned)
            .filter(|&i| self.used[i] != 0)
            .count()
    }

    pub(super) fn pinned_count(&self) -> usize {
        self.pinned - self.inputs
    }
    pub(super) fn pending_blocks(&self) -> usize {
        self.focus.len()
    }

    pub(super) fn import(
        &mut self,
        derivation: Derivation,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Option<CertificateSearchOutcome>, String> {
        self.import_inner(derivation, oracle, control, eh, true)
    }

    fn import_inner(
        &mut self,
        derivation: Derivation,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
        urgent: bool,
    ) -> Result<Option<CertificateSearchOutcome>, String> {
        let degree = self.tuples.first().map_or(0, Vec::len);
        let values = derivation.replay(&self.tuples[..self.inputs], degree, control)?;
        if values.len() < derivation.steps.len() {
            eh.notify(
                "Proof: guided oversized fragments skipped",
                derivation.steps.len() - values.len(),
                MAX_TREE_NODES,
            );
        }
        let before = self.revision;
        for mut terms in values {
            control.check()?;
            terms.sort();
            if self.known.contains(&terms) {
                continue;
            }
            if let Some(certificate) = oracle.check(&terms) {
                return Ok(Some(CertificateSearchOutcome::Found {
                    certificate,
                    steps: 0,
                    shared_lines: 1,
                }));
            }
            self.retain(terms, urgent);
        }
        if before != self.revision {
            eh.notify(
                "Proof: guided retained derivation fragments",
                self.tuples.len() - self.inputs,
                self.fragment_limit,
            );
        }
        Ok(None)
    }

    pub(super) fn import_default(
        &mut self,
        derivation: Derivation,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Option<CertificateSearchOutcome>, String> {
        let before = self.tuples.len();
        self.fragment_limit += derivation.steps.len();
        let found = self.import_inner(derivation, oracle, control, eh, false)?;
        self.fragment_limit = MAX_FRAGMENTS + self.tuples.len() - before;
        self.pinned = self.tuples.len();
        self.replace_next = self.pinned;
        eh.notify(
            "Proof: default diagram fragments retained",
            self.tuples.len() - before,
            0,
        );
        Ok(found)
    }
}

#[cfg(test)]
mod tests;
