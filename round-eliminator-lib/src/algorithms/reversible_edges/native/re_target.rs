//! Bounded exact RE² construction and independent verification of its decoder.
//! Maximality is needed by construction, but NOT trusted by the proof checker:
//! universal rows and the complete existential relation are checked explicitly.
use super::*;
use crate::{
    algorithms::maximize::{combine_lines_custom, without_one},
    constraint::Constraint,
};
use dashmap::DashSet;

const MAX_SEEN_LINES: usize = 100_000;
const MAX_UNIVERSAL_LINES: usize = 1024;
const MAX_DECODER_TUPLES: usize = 200_000;

fn canonical(c: &Constraint) -> BTreeSet<Line> {
    c.lines
        .iter()
        .cloned()
        .map(|mut l| {
            l.normalize();
            l
        })
        .collect()
}

fn shape(p: &Problem, budget: &Budget, eh: &EventHandler) -> Result<(), String> {
    budget.check(eh)?;
    let labels: BTreeSet<_> = p.labels().into_iter().collect();
    if labels.is_empty() || labels.len() > MAX_LABELS {
        return Err(LIMIT.into());
    }
    let names: BTreeMap<_, _> = p.mapping_label_text.iter().cloned().collect();
    if names.len() != p.mapping_label_text.len()
        || labels.iter().any(|l| !names.contains_key(l))
        || names.values().any(|name| name.len() > 4096)
    {
        return Err("Invalid RE² label names".into());
    }
    for c in [&p.active, &p.passive] {
        let Degree::Finite(degree @ 1..=6) = c.degree else {
            return Err("Invalid RE² constraint degree".into());
        };
        if c.lines.is_empty() || c.lines.len() > budget.options.max_configurations {
            return Err(LIMIT.into());
        }
        for row in &c.lines {
            budget.check(eh)?;
            let mut count = 0;
            for part in &row.parts {
                let GroupType::Many(n @ 1..=6) = part.gtype else {
                    return Err("Invalid RE² constraint multiplicity".into());
                };
                let values = part.group.as_vec();
                if values.is_empty() || values.windows(2).any(|w| w[0] >= w[1]) {
                    return Err("Invalid RE² constraint group".into());
                }
                count += n as usize;
            }
            if count != degree {
                return Err("Invalid RE² row degree".into());
            }
        }
    }
    Ok(())
}

/// The same union/intersection closure as ordinary maximize, with one worker,
/// finite-degree inputs, explicit size limits, and cancellation between pairs.
/// A partial closure is discarded, never used as an exact speedup.
fn maximize(
    source: &Constraint,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Constraint, String> {
    let cancellation = eh.cancellation_token();
    let interrupted = || {
        Instant::now() >= budget.deadline
            || cancellation
                .as_ref()
                .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
    };
    let mut current = source.clone();
    current.is_maximized = false;
    current.lines.clear();
    let seen = DashSet::new();
    for line in canonical(source) {
        budget.check(eh)?;
        seen.insert(line.compressed());
        current.add_line_and_discard_non_maximal(line);
    }
    let mut previous = BTreeSet::new();
    loop {
        budget.check(eh)?;
        if current.lines.len() > MAX_UNIVERSAL_LINES.min(budget.options.max_configurations) {
            return Err(LIMIT.into());
        }
        eh.notify(
            "Reversible edges: RE² maximizing target",
            current.lines.len(),
            0,
        );
        let snapshot = canonical(&current);
        let rest = without_one(&current.lines);
        let mut next = current.clone();
        for (i, left) in current.lines.iter().enumerate() {
            for (j, right) in current.lines[..=i].iter().enumerate() {
                budget.check(eh)?;
                if previous.contains(left) && previous.contains(right) {
                    continue;
                }
                let (rows, _, _) = combine_lines_custom(
                    left,
                    right,
                    &rest[i],
                    &rest[j],
                    &seen,
                    usize::MAX,
                    false,
                    false,
                    false,
                    // Once interrupted, stop producing candidates inside the
                    // existing combinator too. The entire result is discarded
                    // by the mandatory budget check immediately afterwards.
                    |a, b| interrupted() || a.is_superset(b),
                    |a, b| a.union(b),
                    |a, b| {
                        if interrupted() {
                            Group::from(vec![])
                        } else {
                            a.intersection(b)
                        }
                    },
                );
                budget.check(eh)?;
                if seen.len() > MAX_SEEN_LINES {
                    return Err(LIMIT.into());
                }
                for row in rows {
                    budget.check(eh)?;
                    next.add_line_and_discard_non_maximal(row);
                    if next.lines.len() > MAX_UNIVERSAL_LINES.min(budget.options.max_configurations)
                    {
                        return Err(LIMIT.into());
                    }
                }
            }
        }
        let next_rows = canonical(&next);
        if next_rows == snapshot {
            next.lines = next_rows.into_iter().collect();
            next.is_maximized = true;
            return Ok(next);
        }
        previous = snapshot;
        next.lines = next_rows.into_iter().collect();
        current = next;
    }
}

pub(super) fn prepare(
    p: &Problem,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Re2Target, String> {
    shape(p, budget, eh)?;
    let mut source = p.clone();
    // Diagram metadata is not part of this construction or its decoder.
    source.diagram_indirect = None;
    let mut first =
        source.speedup_from_universal_constraint(maximize(&source.passive, budget, eh)?);
    first.orientation_given = None;
    shape(&first, budget, eh)?;
    let second = first.speedup_from_universal_constraint(maximize(&first.passive, budget, eh)?);
    let target = Re2Target { first, second };
    if serde_json::to_vec(&target)
        .map_err(|e| e.to_string())?
        .len()
        > CERTIFICATE_BYTES / 2
    {
        return Err(LIMIT.into());
    }
    verify(p, &target, budget, eh)?;
    // The mapper uses this explicit expansion; reject oversized targets once.
    Input::new(&target.second, budget, eh)?;
    eh.notify(
        "Reversible edges: RE² target ready",
        target.second.labels().len(),
        0,
    );
    Ok(target)
}

fn verify_step(
    source: &Problem,
    next: &Problem,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<(), String> {
    shape(source, budget, eh)?;
    shape(next, budget, eh)?;
    if next.active.degree != source.passive.degree || next.passive.degree != source.active.degree {
        return Err("RE² decoder does not swap constraint roles".into());
    }
    let entries = next
        .mapping_label_oldlabels
        .as_ref()
        .ok_or("Missing RE² decoder sets")?;
    let sets: BTreeMap<_, _> = entries.iter().cloned().collect();
    let old: BTreeSet<_> = source.labels().into_iter().collect();
    let labels: BTreeSet<_> = next.labels().into_iter().collect();
    if entries.len() != sets.len()
        || sets.keys().copied().collect::<BTreeSet<_>>() != labels
        || sets.values().any(|s| {
            s.is_empty() || s.windows(2).any(|w| w[0] >= w[1]) || s.iter().any(|l| !old.contains(l))
        })
        || next
            .mapping_oldlabel_text
            .as_ref()
            .map(|v| v.iter().cloned().collect::<BTreeMap<_, _>>())
            != Some(source.mapping_label_text.iter().cloned().collect())
    {
        return Err("Invalid RE² decoder set dictionary".into());
    }
    // Any existential-side tuple must permit a joint choice satisfying the
    // source active constraint. Reconstruct the complete existential relation.
    let expected = source.active.edited(|g| {
        Group::from(
            sets.iter()
                .filter(|(_, s)| s.iter().any(|l| g.contains(l)))
                .map(|(&l, _)| l)
                .collect::<Vec<_>>(),
        )
    });
    if canonical(&expected) != canonical(&next.passive) {
        return Err("RE² decoder has an incorrect existential constraint".into());
    }
    // All choices across each universal row must satisfy the source passive
    // constraint. Do not trust any supplied is_maximized flag or cached diagram.
    let mut tuples = 0;
    for row in &next.active.lines {
        budget.check(eh)?;
        let mut groups = vec![];
        for part in &row.parts {
            if part.group.len() != 1 {
                return Err("Invalid RE² universal row".into());
            }
            let label = part.group.as_vec()[0];
            let GroupType::Many(n) = part.gtype else {
                unreachable!()
            };
            for _ in 0..n {
                groups.push(&sets[&label]);
            }
        }
        let mut digits = vec![0; groups.len()];
        loop {
            budget.check(eh)?;
            tuples += 1;
            if tuples > MAX_DECODER_TUPLES {
                return Err(LIMIT.into());
            }
            let mut choice = Line {
                parts: groups
                    .iter()
                    .zip(&digits)
                    .map(|(g, &i)| Part {
                        group: Group::from(vec![g[i]]),
                        gtype: GroupType::ONE,
                    })
                    .collect(),
            };
            choice.normalize();
            if !source.passive.includes_single_line(&choice) {
                return Err("RE² decoder violates a universal constraint".into());
            }
            let mut i = 0;
            while i < digits.len() {
                digits[i] += 1;
                if digits[i] < groups[i].len() {
                    break;
                }
                digits[i] = 0;
                i += 1;
            }
            if i == digits.len() {
                break;
            }
        }
    }
    Ok(())
}

pub(super) fn verify(
    p: &Problem,
    target: &Re2Target,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<(), String> {
    eh.notify("Reversible edges: verifying RE² decoding", 1, 2);
    verify_step(p, &target.first, budget, eh)?;
    eh.notify("Reversible edges: verifying RE² decoding", 2, 2);
    verify_step(&target.first, &target.second, budget, eh)
}

#[cfg(test)]
mod tests;
