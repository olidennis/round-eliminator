//! Witness-producing, finite reachability version of the two-player tree game.
//!
//! A position is an unordered active tuple to be dominated. The first player
//! chooses a coordinate z and a split x,y with z <= x join y; the second player
//! chooses which replacement tuple to challenge. Reaching a tuple dominated by
//! an input line wins. Cycles without a route to such a leaf do NOT win.

use super::*;

pub(super) struct Check {
    pub passive: Constraint,
    pub active_terms: Vec<Vec<Term>>,
    pub derivation: super::proof::guided::Derivation,
    pub positions: usize,
    pub moves: usize,
}

enum Strategy {
    Input {
        line: usize,
        permutation: Vec<usize>,
    },
    Split(usize),
}

struct Position {
    target: Vec<usize>,
    strategy: Option<Strategy>,
    // Moves that can become winning when this position becomes winning.
    parents: Vec<usize>,
    next_coordinate: usize,
    next_split: usize,
}

struct Move {
    parent: usize,
    children: [usize; 2],
    coordinate: usize,
    // Parent coordinates -> sorted child coordinates, preserving occurrences.
    permutations: [Vec<usize>; 2],
}

struct Game<'a> {
    candidate: &'a Candidate,
    inputs: Vec<Vec<Label>>,
    splits: Vec<Vec<(usize, usize)>>,
    ids: HashMap<Vec<usize>, usize>,
    positions: Vec<Position>,
    moves: Vec<Move>,
    pending: Vec<usize>,
}

impl<'a> Game<'a> {
    // Export the actual strategy, not a tree inferred from simplified terms.
    // Children precede parents and occurrence permutations remain explicit.
    fn derivation(
        &self,
        roots: &[usize],
        eh: &EventHandler,
    ) -> Result<super::proof::guided::Derivation, String> {
        use super::proof::guided::{Derivation, Step};
        let mut result = Derivation::default();
        let mut ids = HashMap::new();
        for &root in roots {
            let mut pending = vec![root];
            while let Some(&id) = pending.last() {
                search::check_event(eh)?;
                if ids.contains_key(&id) {
                    pending.pop();
                    continue;
                }
                let step = match self.positions[id].strategy.as_ref().unwrap() {
                    Strategy::Input { line, permutation } => {
                        Step::Input(permutation.iter().map(|&i| self.inputs[*line][i]).collect())
                    }
                    Strategy::Split(m) => {
                        let step = &self.moves[*m];
                        if let Some(&child) = step.children.iter().find(|c| !ids.contains_key(*c)) {
                            pending.push(child);
                            continue;
                        }
                        Step::Combine {
                            parents: step.children.map(|c| ids[&c]),
                            permutations: step.permutations.clone(),
                            pivot: step.coordinate,
                        }
                    }
                };
                ids.insert(id, result.steps.len());
                result.steps.push(step);
                pending.pop();
            }
        }
        // Retain alternative, already explored winning splits too. Their
        // children use the finite recorded strategies above, so even a move
        // participating in a game cycle is replayed as a finite extra step.
        for step in &self.moves {
            search::check_event(eh)?;
            if step.children.iter().all(|c| ids.contains_key(c)) {
                result.steps.push(Step::Combine {
                    parents: step.children.map(|c| ids[&c]),
                    permutations: step.permutations.clone(),
                    pivot: step.coordinate,
                });
            }
        }
        // The decision game intentionally prunes equivalent decompositions.
        // For certificate discovery, sample some omitted ones after the check,
        // using only already won positions or immediate original-input leaves.
        // No extra reachability search, and no change to the selected strategy
        // or the blocker. Every exported alternative is a finite actual proof.
        let mut checks = 0;
        let mut added = 0;
        'alternatives: for &root in roots.iter().rev() {
            let target = &self.positions[root].target;
            for coordinate in 0..target.len() {
                for x in 0..self.candidate.order.len() {
                    for y in x + 1..self.candidate.order.len() {
                        search::check_event(eh)?;
                        checks += 1;
                        if checks > 2048 || added >= 64 {
                            break 'alternatives;
                        }
                        let z = target[coordinate];
                        if !self.candidate.order[z][self.candidate.join[x][y]]
                            || self.candidate.order[z][x]
                            || self.candidate.order[z][y]
                            || self.splits[z].contains(&(x, y))
                        {
                            continue;
                        }
                        let replacements = [x, y].map(|v| Self::replacement(target, coordinate, v));
                        let mut children = Vec::new();
                        for (child, _) in &replacements {
                            if let Some(&id) = self.ids.get(child).and_then(|p| ids.get(p)) {
                                children.push(id);
                            } else if let Some(Strategy::Input { line, permutation }) =
                                self.input_strategy(child)
                            {
                                let id = result.steps.len();
                                result.steps.push(Step::Input(
                                    permutation.iter().map(|&i| self.inputs[line][i]).collect(),
                                ));
                                children.push(id);
                            } else {
                                break;
                            }
                        }
                        if children.len() == 2 {
                            result.steps.push(Step::Combine {
                                parents: [children[0], children[1]],
                                permutations: replacements.map(|(_, p)| p),
                                pivot: coordinate,
                            });
                            added += 1;
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    fn new(original: &Constraint, candidate: &'a Candidate) -> Self {
        let nodes = candidate.order.len();
        let mut splits = vec![Vec::new(); nodes];
        for z in 0..nodes {
            let mut pairs: Vec<(usize, usize)> = Vec::new();
            for x in 0..nodes {
                for y in x + 1..nodes {
                    if candidate.order[z][candidate.join[x][y]]
                        && !candidate.order[z][x]
                        && !candidate.order[z][y]
                    {
                        pairs.push((x, y));
                    }
                }
            }
            // A genuine decomposition of z suffices by itself: both child
            // targets are weaker than z. Otherwise retain every minimal split,
            // including splits through incomparable nodes in non-distributive
            // lattices. Such splits can produce cycles in the game.
            if let Some(&pair) = pairs
                .iter()
                .find(|&&(x, y)| candidate.order[x][z] && candidate.order[y][z])
            {
                splits[z].push(pair);
            } else {
                splits[z] = pairs
                    .iter()
                    .copied()
                    .filter(|&(x, y)| {
                        !pairs.iter().any(|&(a, b)| {
                            (a, b) != (x, y)
                                && ((candidate.order[a][x] && candidate.order[b][y])
                                    || (candidate.order[a][y] && candidate.order[b][x]))
                        })
                    })
                    .collect();
            }
        }
        let mut inputs: Vec<_> = original.all_choices(true).iter().map(expanded).collect();
        inputs.sort();
        Self {
            candidate,
            inputs,
            splits,
            ids: HashMap::new(),
            positions: Vec::new(),
            moves: Vec::new(),
            pending: Vec::new(),
        }
    }

    // Bipartite matching, not inversion of the (possibly many-to-one) label
    // map. Each leaf retains a permutation of one whole original input line.
    fn input_strategy(&self, target: &[usize]) -> Option<Strategy> {
        fn augment(
            i: usize,
            target: &[usize],
            source: &[usize],
            order: &[Vec<bool>],
            seen: &mut [bool],
            matched: &mut [Option<usize>],
        ) -> bool {
            for j in 0..source.len() {
                if !seen[j] && order[target[i]][source[j]] {
                    seen[j] = true;
                    if matched[j].is_none()
                        || augment(matched[j].unwrap(), target, source, order, seen, matched)
                    {
                        matched[j] = Some(i);
                        return true;
                    }
                }
            }
            false
        }
        for (line, labels) in self.inputs.iter().enumerate() {
            let source: Vec<_> = labels.iter().map(|l| self.candidate.mapping[l]).collect();
            let mut matched = vec![None; target.len()];
            if (0..target.len()).all(|i| {
                augment(
                    i,
                    target,
                    &source,
                    &self.candidate.order,
                    &mut vec![false; source.len()],
                    &mut matched,
                )
            }) {
                let mut permutation = vec![0; target.len()];
                for (j, i) in matched.into_iter().enumerate() {
                    permutation[i.unwrap()] = j;
                }
                return Some(Strategy::Input { line, permutation });
            }
        }
        None
    }

    fn intern(&mut self, target: Vec<usize>) -> usize {
        if let Some(&id) = self.ids.get(&target) {
            return id;
        }
        let strategy = self.input_strategy(&target);
        let id = self.positions.len();
        if strategy.is_none() {
            self.pending.push(id);
        }
        self.ids.insert(target.clone(), id);
        self.positions.push(Position {
            target,
            strategy,
            parents: Vec::new(),
            next_coordinate: 0,
            next_split: 0,
        });
        id
    }

    fn replacement(target: &[usize], coordinate: usize, value: usize) -> (Vec<usize>, Vec<usize>) {
        let mut indexed: Vec<_> = target.iter().copied().enumerate().collect();
        indexed[coordinate].1 = value;
        indexed.sort_by_key(|&(i, x)| (x, i));
        let mut permutation = vec![0; target.len()];
        for (j, &(i, _)) in indexed.iter().enumerate() {
            permutation[i] = j;
        }
        (indexed.into_iter().map(|(_, x)| x).collect(), permutation)
    }

    fn next_move(&mut self, id: usize) -> Option<(usize, usize, usize)> {
        let position = &mut self.positions[id];
        while position.next_coordinate < position.target.len() {
            let i = position.next_coordinate;
            // Equal occurrences have isomorphic successors.
            if i > 0 && position.target[i] == position.target[i - 1] {
                position.next_coordinate += 1;
                position.next_split = 0;
                continue;
            }
            if let Some(&(x, y)) = self.splits[position.target[i]].get(position.next_split) {
                position.next_split += 1;
                return Some((i, x, y));
            }
            position.next_coordinate += 1;
            position.next_split = 0;
        }
        None
    }

    // Least winning fixed point of the AND/OR graph. Registering an edge to
    // an ancestor does not declare that ancestor losing (or winning). When
    // both children win, propagate the finite strategy through reverse edges.
    fn propagate(&mut self, first: usize) {
        let mut ready = vec![first];
        while let Some(m) = ready.pop() {
            let step = &self.moves[m];
            if self.positions[step.parent].strategy.is_none()
                && step
                    .children
                    .iter()
                    .all(|&c| self.positions[c].strategy.is_some())
            {
                let parent = step.parent;
                self.positions[parent].strategy = Some(Strategy::Split(m));
                ready.extend(self.positions[parent].parents.iter().copied());
            }
        }
    }

    fn solve(&mut self, target: Vec<usize>, eh: &mut EventHandler) -> Option<usize> {
        let root = self.intern(target);
        while self.positions[root].strategy.is_none() {
            if eh.is_cancelled() {
                return None;
            }
            let Some(id) = self.pending.pop() else { break };
            if self.positions[id].strategy.is_some() {
                continue;
            }
            let Some((coordinate, x, y)) = self.next_move(id) else {
                continue;
            };
            // Depth-first exploration of one move at a time, with a shared
            // worklist for cycles. Do not build the entire game in advance.
            self.pending.push(id);
            let (left, left_perm) = Self::replacement(&self.positions[id].target, coordinate, x);
            let (right, right_perm) = Self::replacement(&self.positions[id].target, coordinate, y);
            let right = self.intern(right);
            let left = self.intern(left);
            let m = self.moves.len();
            self.moves.push(Move {
                parent: id,
                children: [left, right],
                coordinate,
                permutations: [left_perm, right_perm],
            });
            self.positions[left].parents.push(m);
            self.positions[right].parents.push(m);
            self.propagate(m);
            if self.moves.len() % 1024 == 0 {
                eh.notify("SAT: exploring tree game", self.positions.len(), 0);
            }
        }
        self.positions[root].strategy.as_ref().map(|_| root)
    }

    fn terms(
        &self,
        root: usize,
        memo: &mut HashMap<usize, Vec<Term>>,
    ) -> Result<Vec<Term>, String> {
        // Strategies are acyclic even when the game isn't. Use an explicit
        // stack so long winning paths do not consume the process call stack.
        let mut pending = vec![root];
        while let Some(&id) = pending.last() {
            if memo.contains_key(&id) {
                pending.pop();
                continue;
            }
            let terms = match self.positions[id].strategy.as_ref().unwrap() {
                Strategy::Input { line, permutation } => permutation
                    .iter()
                    .map(|&i| Term::Terminal(self.inputs[*line][i]))
                    .collect::<Vec<_>>(),
                Strategy::Split(m) => {
                    let step = &self.moves[*m];
                    if let Some(&child) = step.children.iter().find(|c| !memo.contains_key(c)) {
                        pending.push(child);
                        continue;
                    }
                    (0..self.positions[id].target.len())
                        .map(|i| {
                            let left = memo[&step.children[0]][step.permutations[0][i]].clone();
                            let right = memo[&step.children[1]][step.permutations[1][i]].clone();
                            if left == right {
                                left
                            } else {
                                Term::Expr(
                                    Box::new(left),
                                    Box::new(right),
                                    if i == step.coordinate {
                                        Operation::Union
                                    } else {
                                        Operation::Intersection
                                    },
                                )
                            }
                        })
                        .collect()
                }
            };
            if !terms
                .iter()
                .zip(&self.positions[id].target)
                .all(|(term, &goal)| self.candidate.order[goal][self.candidate.eval(term)])
            {
                return Err("Game strategy does not dominate its target".to_string());
            }
            memo.insert(id, terms);
            pending.pop();
        }
        Ok(memo[&root].clone())
    }
}

fn minimal_targets<'a>(
    zeros: &'a [usize],
    compatible: &'a [Vec<bool>],
    order: &'a [Vec<bool>],
    degree: usize,
) -> impl Iterator<Item = Vec<usize>> + 'a {
    let mut pending = vec![(Vec::new(), zeros.to_vec())];
    std::iter::from_fn(move || {
        while let Some((prefix, choices)) = pending.pop() {
            if prefix.len() == degree {
                return Some(prefix);
            }
            for &a in choices.iter().rev() {
                let mut target = prefix.clone();
                target.push(a);
                let remaining: Vec<_> = choices
                    .iter()
                    .copied()
                    .filter(|&b| b >= a && compatible[a][b])
                    .collect();
                if target.len() < degree && remaining.is_empty() {
                    continue;
                }
                // Skip a whole subtree if one chosen coordinate can be lowered
                // regardless of how the rest is filled. At a leaf this is the
                // exact minimality test. In particular, a universal bottom
                // target does not cause enumeration of C(N+d-1,d) tuples.
                if target.iter().enumerate().any(|(i, &x)| {
                    zeros.iter().any(|&b| {
                        b != x
                            && order[b][x]
                            && target
                                .iter()
                                .enumerate()
                                .all(|(j, &y)| i == j || compatible[b][y])
                            && (target.len() == degree
                                || remaining.iter().all(|&y| compatible[b][y]))
                    })
                }) {
                    continue;
                }
                pending.push((target, remaining));
            }
        }
        None
    })
}

pub(super) fn check(
    original: &Problem,
    candidate: &Candidate,
    tracking: Option<&DashMap<Line, Tracking>>,
    all_witnesses: bool,
    eh: &mut EventHandler,
) -> Result<Check, String> {
    eh.notify("SAT: checking candidate with tree game", 0, 0);
    // The same passive saturation as procedure(), using the SAT candidate's
    // already validated tables. Reverse the order and interchange join/meet.
    let mut passive = Constraint {
        lines: original.passive.all_choices(true),
        is_maximized: false,
        degree: original.passive.degree,
    }
    .edited(|g| Group::from(vec![candidate.mapping[&g.first()] as Label]));
    passive.maximize_custom(
        eh,
        true,
        false,
        tracking,
        |a, b| candidate.order[a.first() as usize][b.first() as usize],
        |a, b| {
            Group::from(vec![
                candidate.meet[a.first() as usize][b.first() as usize] as Label,
            ])
        },
        |a, b| {
            Group::from(vec![
                candidate.join[a.first() as usize][b.first() as usize] as Label,
            ])
        },
    );
    search::check_event(eh)?;
    let nodes = candidate.order.len();
    let mut compatible = vec![vec![false; nodes]; nodes];
    for line in &passive.lines {
        search::check_event(eh)?;
        let pair = expanded(line);
        for a in 0..nodes {
            for b in 0..nodes {
                if candidate.order[pair[0] as usize][a] && candidate.order[pair[1] as usize][b] {
                    compatible[a][b] = true;
                    compatible[b][a] = true;
                }
            }
        }
    }
    let zeros: Vec<_> = (0..nodes).filter(|&a| compatible[a][a]).collect();
    let mut game = Game::new(&original.active, candidate);
    let mut wins = Vec::new();
    // Enumerate multisets, not all d! permutations. Only minimal compatible
    // tuples need solving: obtainability is downward closed. Minimize whole
    // tuples, not individual self-compatible labels, to preserve cross-pairs.
    for (count, target) in minimal_targets(
        &zeros,
        &compatible,
        &candidate.order,
        original.active.finite_degree(),
    )
    .enumerate()
    {
        search::check_event(eh)?;
        if count % 1024 == 0 {
            eh.notify("SAT: checking game targets", count, 0);
        }
        if let Some(id) = game.solve(target, eh) {
            wins.push(id);
            if !all_witnesses {
                break;
            }
        }
    }
    let mut memo = HashMap::new();
    search::check_event(eh)?;
    // Include winning intermediate positions explored off the final strategy.
    // Their configurations are also valid; only roots go into the blocker.
    let fragment_roots: Vec<_> = game
        .positions
        .iter()
        .enumerate()
        .filter_map(|(id, p)| p.strategy.as_ref().map(|_| id))
        .collect();
    let derivation = game.derivation(&fragment_roots, eh)?;
    let active_terms = wins
        .into_iter()
        .map(|id| game.terms(id, &mut memo))
        .collect::<Result<_, _>>()?;
    Ok(Check {
        passive,
        active_terms,
        derivation,
        positions: game.positions.len(),
        moves: game.moves.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_export_visits_omitted_decompositions_without_changing_the_game() {
        let original = Problem::from_string("A B\n\nA B").unwrap();
        let candidate = Candidate {
            order: (0..8)
                .map(|a| (0..8).map(|b| a & b == a).collect())
                .collect(),
            join: (0..8).map(|a| (0..8).map(|b| a | b).collect()).collect(),
            meet: (0..8).map(|a| (0..8).map(|b| a & b).collect()).collect(),
            mapping: original
                .mapping_label_text
                .iter()
                .map(|(l, s)| (*l, if s == "A" { 7 } else { 0 }))
                .collect(),
        };
        let mut game = Game::new(&original.active, &candidate);
        assert_eq!(game.splits[7].len(), 1);
        let root = game.solve(vec![0, 7], &mut EventHandler::null()).unwrap();
        assert!(game.moves.is_empty());
        let dag = game.derivation(&[root], &EventHandler::null()).unwrap();
        assert!(dag
            .steps
            .iter()
            .any(|s| matches!(s, super::super::proof::guided::Step::Combine { .. })));
        assert!(game.moves.is_empty());
        let inputs: Vec<_> = original
            .active
            .all_choices(true)
            .iter()
            .map(|line| {
                let mut terms: Vec<_> = expanded(line).into_iter().map(Term::Terminal).collect();
                terms.sort();
                terms
            })
            .collect();
        assert!(dag.replay(&inputs, 2, &SearchControl::default()).is_ok());
    }

    #[test]
    fn exported_strategy_replays_whole_intermediate_configurations() {
        let original = Problem::from_string("A A B\nA B B\n\nA B").unwrap();
        let candidate = Candidate {
            order: (0..4)
                .map(|a| (0..4).map(|b| a & b == a).collect())
                .collect(),
            join: (0..4).map(|a| (0..4).map(|b| a | b).collect()).collect(),
            meet: (0..4).map(|a| (0..4).map(|b| a & b).collect()).collect(),
            mapping: original
                .mapping_label_text
                .iter()
                .map(|(id, name)| (*id, if name == "A" { 1 } else { 2 }))
                .collect(),
        };
        let mut game = Game::new(&original.active, &candidate);
        let root = game
            .solve(vec![0, 0, 3], &mut EventHandler::null())
            .unwrap();
        let inputs: Vec<_> = original
            .active
            .all_choices(true)
            .iter()
            .map(|line| {
                let mut terms: Vec<_> = expanded(line).into_iter().map(Term::Terminal).collect();
                terms.sort();
                terms
            })
            .collect();
        let dag = game.derivation(&[root], &EventHandler::null()).unwrap();
        let fragments = dag.replay(&inputs, 3, &SearchControl::default()).unwrap();
        assert!(fragments.len() >= 3);
        assert!(fragments
            .iter()
            .any(|t| t.iter().any(|term| matches!(term, Term::Expr(..)))));
        let expected = game.terms(root, &mut HashMap::new()).unwrap();
        let mut expected: Vec<_> = expected.iter().map(|term| candidate.eval(term)).collect();
        expected.sort();
        assert!(fragments.iter().any(|t| {
            let mut values: Vec<_> = t.iter().map(|term| candidate.eval(term)).collect();
            values.sort();
            values == expected
        }));
    }
    use crate::{group::GroupType, part::Part};
    use itertools::Itertools;
    use std::collections::BTreeSet;

    #[test]
    fn every_boolean_lattice_position_matches_full_active_closure() {
        let nodes = 8;
        let mut candidate = Candidate {
            order: (0..nodes)
                .map(|a| (0..nodes).map(|b| a & b == a).collect())
                .collect(),
            mapping: HashMap::new(),
            join: (0..nodes)
                .map(|a| (0..nodes).map(|b| a | b).collect())
                .collect(),
            meet: (0..nodes)
                .map(|a| (0..nodes).map(|b| a & b).collect())
                .collect(),
        };
        let mut seed = 42u64;
        let mut compound = false;
        let mut losing = false;
        for degree in 1..=3 {
            for _ in 0..8 {
                let mut text = String::new();
                for _ in 0..4 {
                    for _ in 0..degree {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        text.push((b'A' + ((seed >> 32) % 8) as u8) as char);
                        text.push(' ');
                    }
                    text.push('\n');
                }
                text.push_str("\nABCDEFGH ABCDEFGH");
                let original = Problem::from_string(text).unwrap();
                candidate.mapping = original
                    .mapping_label_text
                    .iter()
                    .map(|(l, s)| (*l, (s.as_bytes()[0] - b'A') as usize))
                    .collect();
                let names = (0..nodes)
                    .map(|i| (i as Label, format!("(N{i})")))
                    .collect();
                let (full, _) = original
                    .fixpoint_onestep(
                        false,
                        &candidate.label_mapping(),
                        &names,
                        &candidate.diagram(),
                        None,
                        None,
                        &mut EventHandler::null(),
                    )
                    .unwrap();
                let mut game = Game::new(&original.active, &candidate);
                let mut memo = HashMap::new();
                for target in (0..nodes).combinations_with_replacement(degree) {
                    let line = |values: &[usize]| {
                        let mut line = Line {
                            parts: values
                                .iter()
                                .map(|&l| Part {
                                    group: Group::from(vec![l as Label]),
                                    gtype: GroupType::Many(1),
                                })
                                .collect(),
                        };
                        line.normalize();
                        line
                    };
                    let dominates = |a: &Group, b: &Group| {
                        candidate.order[b.first() as usize][a.first() as usize]
                    };
                    let expected = full
                        .active
                        .includes_with_custom_supersets(&line(&target), Some(dominates));
                    let win = game.solve(target.clone(), &mut EventHandler::null());
                    assert_eq!(
                        win.is_some(),
                        expected,
                        "degree {degree}, target {target:?}"
                    );
                    if let Some(id) = win {
                        let terms = game.terms(id, &mut memo).unwrap();
                        compound |= terms.iter().any(|t| matches!(t, Term::Expr(..)));
                        let values: Vec<_> = terms.iter().map(|t| candidate.eval(t)).collect();
                        assert!(full
                            .active
                            .includes_with_custom_supersets(&line(&values), Some(dominates)));
                    } else {
                        losing = true;
                    }
                }
            }
        }
        assert!(compound && losing);
    }

    #[test]
    fn minimal_targets_match_brute_force() {
        // Exercise cross-pair constraints, not just independently minimal
        // self-compatible labels. This test does not use passive saturation.
        let nodes = 4;
        let order: Vec<Vec<_>> = (0..nodes)
            .map(|a| (0..nodes).map(|b| a <= b).collect())
            .collect();
        let pairs: Vec<_> = (0..nodes)
            .flat_map(|a| (a..nodes).map(move |b| (a, b)))
            .collect();
        for mask in 0..1usize << pairs.len() {
            let mut compatible = vec![vec![false; nodes]; nodes];
            for (i, &(a, b)) in pairs.iter().enumerate() {
                compatible[a][b] = mask & (1 << i) != 0;
                compatible[b][a] = compatible[a][b];
            }
            let zeros: Vec<_> = (0..nodes).filter(|&a| compatible[a][a]).collect();
            for degree in 1..=4 {
                let expected: BTreeSet<_> = zeros
                    .iter()
                    .copied()
                    .combinations_with_replacement(degree)
                    .filter(|target| {
                        target
                            .iter()
                            .all(|&a| target.iter().all(|&b| compatible[a][b]))
                    })
                    .filter(|target| {
                        !target.iter().enumerate().any(|(i, &a)| {
                            zeros.iter().any(|&b| {
                                a != b
                                    && order[b][a]
                                    && target
                                        .iter()
                                        .enumerate()
                                        .all(|(j, &c)| i == j || compatible[b][c])
                            })
                        })
                    })
                    .collect();
                let actual: BTreeSet<_> =
                    minimal_targets(&zeros, &compatible, &order, degree).collect();
                assert_eq!(actual, expected, "mask {mask}, degree {degree}");
            }
        }
        // There are over a billion multisets here, but just one minimal target.
        let nodes = 20;
        let order: Vec<Vec<_>> = (0..nodes)
            .map(|a| (0..nodes).map(|b| a <= b).collect())
            .collect();
        let compatible = vec![vec![true; nodes]; nodes];
        let zeros: Vec<_> = (0..nodes).collect();
        assert_eq!(
            minimal_targets(&zeros, &compatible, &order, 20).collect::<Vec<_>>(),
            vec![vec![0; 20]]
        );
    }

    #[test]
    fn cyclic_games_require_finite_winning_strategies() {
        // M3: three incomparable atoms. Splitting an atom uses the other two,
        // so naive recursive negative caching is not a correct game solver.
        let mut candidate = Candidate {
            order: vec![vec![false; 5]; 5],
            mapping: HashMap::new(),
            join: vec![vec![4; 5]; 5],
            meet: vec![vec![0; 5]; 5],
        };
        for a in 0..5 {
            for b in 0..5 {
                candidate.order[a][b] = a == 0 || b == 4 || a == b;
                candidate.join[a][b] = if a == 0 || b == 4 || a == b {
                    b
                } else if b == 0 || a == 4 {
                    a
                } else {
                    4
                };
                candidate.meet[a][b] = if a == 0 || b == 4 || a == b {
                    a
                } else if b == 0 || a == 4 {
                    b
                } else {
                    0
                };
            }
        }
        for text in ["A A\n\nA A", "A A\nB B\n\nA B"] {
            let original = Problem::from_string(text).unwrap();
            candidate.mapping = original
                .mapping_label_text
                .iter()
                .map(|(l, s)| (*l, if s == "A" { 1 } else { 2 }))
                .collect();
            let mut game = Game::new(&original.active, &candidate);
            let mut events = EventHandler::null();
            assert!(game.solve(vec![1, 3], &mut events).is_none());
            assert!(game.moves.len() > 0);
            assert!(game.positions.len() <= 15);
            let win = game.solve(vec![0, 3], &mut events);
            if original.active.lines.len() == 1 {
                assert!(win.is_none(), "A cycle without a winning exit must lose");
            } else {
                let terms = game.terms(win.unwrap(), &mut HashMap::new()).unwrap();
                let values: Vec<_> = terms.iter().map(|t| candidate.eval(t)).collect();
                assert_eq!(
                    values,
                    vec![0, 4],
                    "A strategy may strictly dominate its goal"
                );
            }
            // Reusing solved components, in either outcome, must be stable.
            assert!(game.solve(vec![1, 3], &mut events).is_none());
            assert!(game.solve(vec![1, 1], &mut events).is_some());
        }
    }
}
