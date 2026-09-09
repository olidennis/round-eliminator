//! One native Loop bootstrap: saturate the node constraint on the ordinary
//! default lattice and retain provenance even for subsequently dominated lines.
//! This only supplies original-input proofs; diagram-specific triviality is
//! never published as a universal certificate.

use super::*;
use proof::guided::{Derivation, Step};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_NODES: usize = 128;
const MAX_STEPS: usize = 4096;
const TIME_BUDGET: Duration = Duration::from_secs(5);

// Same right-closed-set lattice as FixpointDiagram::new_smaller, but enumerate
// distinct sets incrementally so an exponential completion can be stopped.
pub(super) fn default_candidate(
    original: &Problem,
    limit: usize,
    control: &SearchControl,
) -> Result<Option<Candidate>, String> {
    control.check()?;
    let mut labels = original.labels();
    labels.sort_unstable();
    if labels.len() > MAX_NODES {
        return Ok(None);
    }
    let mut closures: HashMap<_, BTreeSet<_>> =
        labels.iter().map(|&l| (l, BTreeSet::from([l]))).collect();
    for &(a, b) in original
        .diagram_indirect
        .as_ref()
        .ok_or("Missing input diagram")?
    {
        closures
            .get_mut(&a)
            .ok_or("Unknown input diagram label")?
            .insert(b);
    }
    for k in &labels {
        control.check()?;
        let through = closures[k].clone();
        for a in &labels {
            if closures[a].contains(k) {
                closures.get_mut(a).unwrap().extend(through.iter().copied());
            }
        }
    }
    let mut sets = BTreeSet::from([BTreeSet::new()]);
    for label in &labels {
        control.check()?;
        let previous: Vec<_> = sets.iter().cloned().collect();
        for set in previous {
            control.check()?;
            sets.insert(set.union(&closures[label]).copied().collect());
            if sets.len() > limit {
                return Ok(None);
            }
        }
    }
    let sets: Vec<_> = sets.into_iter().collect();
    let ids: HashMap<_, _> = sets
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, s)| (s, i))
        .collect();
    let n = sets.len();
    let mut order = vec![vec![false; n]; n];
    let mut join = vec![vec![0; n]; n];
    let mut meet = join.clone();
    for a in 0..n {
        control.check()?;
        for b in 0..n {
            // Diagram arrows use reverse inclusion; join is intersection.
            order[a][b] = sets[a].is_superset(&sets[b]);
            join[a][b] = ids[&sets[a]
                .intersection(&sets[b])
                .copied()
                .collect::<BTreeSet<_>>()];
            meet[a][b] = ids[&sets[a].union(&sets[b]).copied().collect::<BTreeSet<_>>()];
        }
    }
    Ok(Some(Candidate {
        order,
        join,
        meet,
        mapping: labels.iter().map(|&l| (l, ids[&closures[&l]])).collect(),
    }))
}

// Seed from whole original lines, never by inverting a possibly many-to-one
// label mapping. Match the normalization and occurrence order used by tracking.
fn inputs(original: &Constraint, candidate: &Candidate) -> (Derivation, HashMap<Line, usize>) {
    let mut dag = Derivation::default();
    let mut ids = HashMap::new();
    let mut lines = original.all_choices(true);
    lines.sort();
    for line in lines {
        let mut occurrences: HashMap<Label, Vec<Label>> = HashMap::new();
        for label in expanded(&line) {
            occurrences
                .entry(candidate.mapping[&label] as Label)
                .or_default()
                .push(label);
        }
        let mut mapped = line;
        for part in &mut mapped.parts {
            part.group = Group::from(vec![candidate.mapping[&part.group.first()] as Label]);
        }
        mapped.normalize();
        if !ids.contains_key(&mapped) {
            let row = mapped
                .parts
                .iter()
                .flat_map(|p| occurrences[&p.group.first()].iter().copied())
                .collect();
            ids.insert(mapped, dag.steps.len());
            dag.steps.push(Step::Input(row));
        }
    }
    (dag, ids)
}

fn append_tracking(
    root: &Line,
    tracking: &DashMap<Line, Tracking>,
    ids: &mut HashMap<Line, usize>,
    dag: &mut Derivation,
    degree: usize,
    control: &SearchControl,
) -> Result<(), String> {
    let mut pending = vec![root.clone()];
    let mut visiting = HashSet::new();
    while let Some(line) = pending.last().cloned() {
        control.check()?;
        if ids.contains_key(&line) {
            visiting.remove(&line);
            pending.pop();
            continue;
        }
        visiting.insert(line.clone());
        let (left, right, before, normalization, operations) = tracking
            .get(&line)
            .ok_or("Missing default saturation provenance")?
            .clone();
        if let Some(child) = [&left, &right].into_iter().find(|l| !ids.contains_key(*l)) {
            if visiting.contains(child) {
                return Err("Cyclic default saturation provenance".into());
            }
            pending.push(child.clone());
            continue;
        }
        let offsets = |l: &Line| {
            let mut total = 0;
            l.parts
                .iter()
                .map(|p| {
                    let start = total;
                    total += p.gtype.value();
                    start
                })
                .collect::<Vec<_>>()
        };
        let (lo, ro) = (offsets(&left), offsets(&right));
        let (mut lu, mut ru) = (vec![0; left.parts.len()], vec![0; right.parts.len()]);
        let mut permutations = [Vec::new(), Vec::new()];
        let mut pivot = None;
        for index in normalization.iter().flatten().copied() {
            let &(a, b, op) = operations
                .get(index)
                .ok_or("Invalid default provenance operation")?;
            let part = before
                .parts
                .get(index)
                .ok_or("Invalid default provenance part")?;
            if a >= lo.len() || b >= ro.len() {
                return Err("Invalid default provenance parent part".into());
            }
            for _ in 0..part.gtype.value() {
                if op == Operation::Union && pivot.replace(permutations[0].len()).is_some() {
                    return Err("Default provenance has multiple join coordinates".into());
                }
                permutations[0].push(lo[a] + lu[a]);
                permutations[1].push(ro[b] + ru[b]);
                lu[a] += 1;
                ru[b] += 1;
            }
        }
        for p in &permutations {
            let mut sorted = p.clone();
            sorted.sort_unstable();
            if sorted != (0..degree).collect::<Vec<_>>() {
                return Err("Default provenance does not preserve occurrences".into());
            }
        }
        let step = Step::Combine {
            parents: [ids[&left], ids[&right]],
            permutations,
            pivot: pivot.ok_or("Default provenance has no join coordinate")?,
        };
        ids.insert(line.clone(), dag.steps.len());
        dag.steps.push(step);
        visiting.remove(&line);
        pending.pop();
    }
    Ok(())
}

pub(super) fn collect(
    original: &Problem,
    eh: &mut EventHandler,
    control: &SearchControl,
) -> Result<Option<Derivation>, String> {
    collect_bounded(original, eh, control, MAX_NODES, MAX_STEPS, TIME_BUDGET)
}

fn collect_bounded(
    original: &Problem,
    eh: &mut EventHandler,
    control: &SearchControl,
    node_limit: usize,
    step_limit: usize,
    time_budget: Duration,
) -> Result<Option<Derivation>, String> {
    control.check()?;
    eh.notify("Proof: default diagram seed starting", 0, 0);
    // Only this optional bootstrap is bounded. The independent diagram and
    // universal proof workers retain their existing supported degrees/grammar.
    if original.active.finite_degree() > 6 {
        eh.notify(
            "Proof: default diagram seed skipped (degree limit)",
            original.active.finite_degree(),
            6,
        );
        return Ok(None);
    }
    let Some(candidate) = default_candidate(original, node_limit, control)? else {
        eh.notify(
            "Proof: default diagram seed skipped (diagram limit)",
            node_limit,
            0,
        );
        return Ok(None);
    };
    eh.notify(
        "Proof: saturating default diagram",
        candidate.order.len(),
        0,
    );
    control.check()?;
    let tracking = DashMap::new();
    let start = Instant::now();
    let stopped = Arc::new(AtomicBool::new(false));
    let calls = AtomicUsize::new(0);
    let checkpoint = || {
        if control.cancelled.load(Ordering::Relaxed)
            || start.elapsed() >= time_budget
            || tracking.len() >= step_limit
        {
            stopped.store(true, Ordering::Relaxed);
        }
    };
    let mut events = EventHandler::with(|(message, a, b)| {
        checkpoint();
        eh.notify(format!("Proof: default diagram {message}"), a, b);
    })
    .with_cancellation(stopped.clone());
    let mut active = Constraint {
        lines: original.active.all_choices(true),
        degree: original.active.degree,
        is_maximized: false,
    }
    .edited(|g| Group::from(vec![candidate.mapping[&g.first()] as Label]));
    // Exactly the active half of fixpoint_onestep's full saturation. Passive
    // saturation is unnecessary: we export all active derivations, not just
    // the diagram's triviality witnesses, and verify compatibility universally.
    active.maximize_custom(
        &mut events,
        true,
        false,
        Some(&tracking),
        |a, b| {
            if calls.fetch_add(1, Ordering::Relaxed) % 1024 == 0 {
                checkpoint();
            }
            candidate.order[b.first() as usize][a.first() as usize]
        },
        |a, b| {
            Group::from(vec![
                candidate.join[a.first() as usize][b.first() as usize] as Label,
            ])
        },
        |a, b| {
            Group::from(vec![
                candidate.meet[a.first() as usize][b.first() as usize] as Label,
            ])
        },
    );
    drop(events);
    control.check()?;
    if stopped.load(Ordering::Relaxed) {
        eh.notify(
            "Proof: default diagram seed budget reached (retaining partial derivations)",
            tracking.len(),
            step_limit,
        );
    }
    let (mut dag, mut ids) = inputs(&original.active, &candidate);
    let mut lines: Vec<_> = tracking.iter().map(|e| e.key().clone()).collect();
    lines.sort();
    for line in &lines {
        control.check()?;
        if dag.steps.len() >= step_limit {
            eh.notify(
                "Proof: default diagram seed export limit",
                dag.steps.len(),
                step_limit,
            );
            break;
        }
        append_tracking(
            line,
            &tracking,
            &mut ids,
            &mut dag,
            original.active.finite_degree(),
            control,
        )?;
    }
    eh.notify(
        "Proof: default diagram derivations extracted",
        dag.steps
            .iter()
            .filter(|s| matches!(s, Step::Combine { .. }))
            .count(),
        tracking.len(),
    );
    Ok(Some(dag))
}

#[cfg(test)]
mod tests;
