//! One round-elimination step: universal passive validity, existential active
//! validity, then exchange active/passive roles. The original problem and input
//! promise remain unchanged; the returned problem has its own label table.
//!
//! Correctness relative to unpruned input-aware RE:
//! - Interpret every new label as its nonempty old-output set S. A fiber of a
//!   joint part R is contained in R's output projection. Every output selection
//!   from those projections is valid, since selecting corresponding pairs from
//!   the joint rectangle is valid. The new subset mapping separately guarantees
//!   permissions at the actual inputs. The existential image admits exactly
//!   those tuples from which a valid old active output can be selected.
//! - Conversely, fix a valid unpruned universal-side tuple of inputs i_k and
//!   sets S_k. Parts {(i_k,o) : o in S_k} form a valid joint rectangle, covered
//!   by some joint maximum after aligning occurrences. Its fibers T_k at i_k
//!   contain S_k and respect the same permissions. Emit those fiber labels.
//!   This enlargement is local to the universal node and preserves every old
//!   existential witness at the other side. For unrestricted mappings, use the
//!   same argument with the ordinary output maximum.
//!
//! These are zero-round mappings in both directions under the retained input
//! promise. This is not global output maximization followed by input filtering:
//! that operation can remove sets necessary at restricted inputs.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    algorithms::{
        configurations::map_parts_with,
        maximize::{self, MaximizationResult},
    },
    computation::{Cancellation, Cancelled, Computation, Phase, WorkProgress},
    model::{
        Constraint, ConstraintPair, GraphClass, Input, InputOutputLabel, LabelId, LabelSet,
        OutputMapping, Problem,
    },
};

/// Convenience entry point that computes the passive maximum first.
pub fn apply(problem: &Problem) -> anyhow::Result<Problem> {
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    let maximum = maximize::compute_with(problem, &mut computation)?;
    apply_with(problem, &maximum, &mut computation)
}

/// Use a complete maximum belonging to this exact immutable problem snapshot.
/// Like the submitted problem, the cached result is trusted rather than verified
/// by repeating maximization. It must have the correct output-only/joint guarantee.
pub fn apply_with(
    problem: &Problem,
    maximum: &MaximizationResult,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Problem> {
    computation.report(Phase::Transforming, 0)?;
    let cancellation = computation.cancellation.clone();
    let universal_parts = match maximum {
        MaximizationResult::OutputOnly(constraint) => constraint
            .configurations()
            .map(|row| row.parts().len() as u64)
            .sum::<u64>(),
        MaximizationResult::InputOutput(constraint) => constraint
            .configurations()
            .map(|row| row.parts().len() as u64)
            .sum::<u64>(),
    };
    let existential_parts = problem
        .output()
        .active()
        .configurations()
        .filter(|row| {
            problem
                .graph_class()
                .active()
                .binary_search(&row.degree())
                .is_ok()
        })
        .map(|row| row.parts().len() as u64)
        .sum::<u64>();
    let total = universal_parts.saturating_add(existential_parts);
    computation.report_work(
        Phase::Transforming,
        0,
        WorkProgress {
            completed: 0,
            total,
            pass: 1,
        },
    )?;
    let sets = set_alphabet_with(maximum, &cancellation)?;
    let ids: BTreeMap<_, _> = sets
        .iter()
        .enumerate()
        .map(|(index, set)| Ok((set, LabelId(u32::try_from(index)?))))
        .collect::<Result<_, std::num::TryFromIntError>>()?;
    let mut completed = 0u64;
    let mut report_part = || {
        completed = completed.saturating_add(1);
        computation.report_work(
            Phase::Transforming,
            completed.min(u64::from(u32::MAX)) as u32,
            WorkProgress {
                completed,
                total,
                pass: 1,
            },
        )
    };
    // The universal side is represented by set-labels. For a joint part R,
    // R(i) = {o : (i,o) in R} supplies its choices. The incidence map below
    // admits only choices contained in the actual input's old permissions.
    let active = match maximum {
        MaximizationResult::OutputOnly(constraint) => {
            anyhow::ensure!(
                matches!(problem.mapping(), OutputMapping::Unrestricted),
                "Input-dependent round elimination requires an input-aware maximum."
            );
            map_parts_with(constraint, &cancellation, |set| {
                report_part()?;
                Ok(LabelSet::new(vec![ids[set]]))
            })?
        }
        MaximizationResult::InputOutput(constraint) => {
            anyhow::ensure!(
                matches!(problem.mapping(), OutputMapping::Restricted(_)),
                "Input-independent round elimination requires an output-only maximum."
            );
            map_parts_with(constraint, &cancellation, |pairs| {
                let choices = fibers_with(pairs, &cancellation)?;
                report_part()?;
                Ok(LabelSet::new(choices.iter().map(|set| ids[set]).collect()))
            })?
        }
    };

    // Index set-labels by each old output once. The existential image of an
    // old part O is precisely the new sets S with S intersecting O.
    let mut containing = vec![Vec::new(); problem.output().labels().len()];
    for (set, &id) in &ids {
        for label in set.iter() {
            cancellation.check()?;
            containing[label.0 as usize].push(id);
        }
    }
    let containing: Vec<_> = containing.into_iter().map(LabelSet::new).collect();
    let old_active = Constraint::new(
        problem
            .output()
            .active()
            .configurations()
            .filter(|configuration| {
                problem
                    .graph_class()
                    .active()
                    .binary_search(&configuration.degree())
                    .is_ok()
            })
            .cloned()
            .collect(),
    );
    let passive = map_parts_with(&old_active, &cancellation, |old| {
        let mut choices = LabelSet::default();
        for label in old.iter() {
            cancellation.check()?;
            choices = choices.union(&containing[label.0 as usize]);
        }
        report_part()?;
        Ok(choices)
    })?;
    let mapping = match problem.mapping() {
        OutputMapping::Unrestricted => OutputMapping::Unrestricted,
        OutputMapping::Restricted(entries) => {
            let mut images = Vec::with_capacity(entries.len());
            for allowed in entries {
                let mut choices = Vec::new();
                for (set, &id) in &ids {
                    cancellation.check()?;
                    if set.is_subset_of(allowed) {
                        choices.push(id);
                    }
                }
                images.push(LabelSet::new(choices));
            }
            OutputMapping::Restricted(images)
        }
    };
    // Never infer a weaker input promise from the feasible joint relation.
    // The physical promise is preserved, with its side roles exchanged.
    let input = match problem.input() {
        Input::Trivial => Input::Trivial,
        Input::Explicit(input) => Input::Explicit(ConstraintPair::new(
            input.labels().to_vec(),
            input.passive().clone(),
            input.active().clone(),
        )?),
    };
    cancellation.check()?;
    let names = (0..sets.len()).map(label_name).collect();
    let output = ConstraintPair::new(names, active, passive)?;
    let transformed = Problem::new(
        GraphClass::new(
            problem.graph_class().passive().to_vec(),
            problem.graph_class().active().to_vec(),
        ),
        input,
        output,
        mapping,
    )?;
    // An impossible part discards its entire configuration, resolving the
    // remaining parts without visiting them. All parts are decided at this point.
    computation.report_work(
        Phase::Transforming,
        total.min(u64::from(u32::MAX)) as u32,
        WorkProgress {
            completed: total,
            total,
            pass: 1,
        },
    )?;
    Ok(transformed)
}

/// Deterministic IDs depend on the sets themselves, not parallel completion
/// order. Only the nonempty sets actually used by the universal side are interned.
fn set_alphabet_with(
    maximum: &MaximizationResult,
    cancellation: &Cancellation,
) -> Result<Vec<LabelSet>, Cancelled> {
    let mut sets = BTreeSet::new();
    match maximum {
        MaximizationResult::OutputOnly(constraint) => {
            for configuration in constraint.configurations() {
                for part in configuration.parts() {
                    cancellation.check()?;
                    sets.insert(part.labels().clone());
                }
            }
        }
        MaximizationResult::InputOutput(constraint) => {
            for configuration in constraint.configurations() {
                for part in configuration.parts() {
                    sets.extend(fibers_with(part.labels(), cancellation)?);
                }
            }
        }
    }
    Ok(sets.into_iter().collect())
}

/// Group a joint part by input without enumerating any input configurations.
/// Identical fibers are one set-label, even if different inputs produced them.
fn fibers_with(
    pairs: &LabelSet<InputOutputLabel>,
    cancellation: &Cancellation,
) -> Result<Vec<LabelSet>, Cancelled> {
    let mut fibers = BTreeMap::<_, Vec<_>>::new();
    for pair in pairs.iter() {
        cancellation.check()?;
        fibers.entry(pair.input).or_default().push(pair.output);
    }
    Ok(fibers.into_values().map(LabelSet::new).collect())
}

// Compact fresh names avoid nesting source-set names on every RE iteration.
fn label_name(index: usize) -> String {
    const CHARACTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    CHARACTERS
        .get(index)
        .map_or_else(|| format!("L{index}"), |&c| char::from(c).to_string())
}

#[cfg(test)]
mod tests;
