//! LLM: One round-elimination step: universal passive validity, existential active
//! LLM: validity, then exchange active/passive roles. The original problem and input
//! LLM: promise remain unchanged; the returned problem has its own label table.
//! LLM: Correctness relative to unpruned input-aware RE:
//! LLM: - Interpret every new label as its nonempty old-output set S. A fiber of a
//! LLM: joint part R is contained in R's output projection. Every output selection
//! LLM: from those projections is valid, since selecting corresponding pairs from
//! LLM: the joint rectangle is valid. The new subset mapping separately guarantees
//! LLM: permissions at the actual inputs. The existential image admits exactly
//! LLM: those tuples from which a valid old active output can be selected.
//! LLM: - Conversely, fix a valid unpruned universal-side tuple of inputs i_k and
//! LLM: sets S_k. Parts {(i_k,o) : o in S_k} form a valid joint rectangle, covered
//! LLM: by some joint maximum after aligning occurrences. Its fibers T_k at i_k
//! LLM: contain S_k and respect the same permissions. Emit those fiber labels.
//! LLM: This enlargement is local to the universal node and preserves every old
//! LLM: existential witness at the other side. For unrestricted mappings, use the
//! LLM: same argument with the ordinary output maximum.
//! LLM: These are zero-round mappings in both directions under the retained input
//! LLM: promise. This is not global output maximization followed by input filtering:
//! LLM: that operation can remove sets necessary at restricted inputs.

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

/// LLM: Compute a passive maximum and perform one RE step without external progress handling.
/// LLM: A convenience entry point for direct Rust callers.
pub fn apply(problem: &Problem) -> anyhow::Result<Problem> {
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |_| {};
    let mut computation = Computation::new(Cancellation::default(), &mut report);
    let maximum = maximize::compute_with(problem, &mut computation)?;
    apply_with(problem, &maximum, &mut computation)
}

/// LLM: Perform RE using a completed maximum of this exact problem snapshot. Used by the API
/// LLM: to reuse source artifacts; it forms universal set labels, existential images, and
/// LLM: swaps side roles while preserving the input promise.
pub fn apply_with(
    problem: &Problem,
    maximum: &MaximizationResult,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Problem> {
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Transforming, 0)?;
    let cancellation = computation.cancellation.clone();
    // LLM: Count universal-side parts to combine their progress with the existential
    // LLM: transformation work.
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
    // LLM: Count only active rows at the declared degrees when building the existential side.
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
    // LLM: Count the current pass's work items so reported completion can advance
    // LLM: monotonically.
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
    // LLM: Intern the distinct nonempty output-set labels needed by the universal RE side.
    let sets = set_alphabet_with(maximum, &cancellation)?;
    let ids: BTreeMap<_, _> = sets
        .iter()
        .enumerate()
        .map(|(index, set)| Ok((set, LabelId(u32::try_from(index)?))))
        .collect::<Result<_, std::num::TryFromIntError>>()?;
    let mut completed = 0u64;
    // LLM: Prepare `report_part` for the following computation; it stores the report part for
    // LLM: this stage.
    let mut report_part = || {
        // LLM: Update the current working state before continuing this operation.
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
    // LLM: The universal side is represented by set-labels. For a joint part R,
    // LLM: R(i) = {o : (i,o) in R} supplies its choices. The incidence map below
    // LLM: admits only choices contained in the actual input's old permissions.

    let active = match maximum {
        MaximizationResult::OutputOnly(constraint) => {
            // LLM: Update the current working state before continuing this operation.
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
            // LLM: Update the current working state before continuing this operation.
            anyhow::ensure!(
                matches!(problem.mapping(), OutputMapping::Restricted(_)),
                "Input-independent round elimination requires an output-only maximum."
            );
            // LLM: Substitute whole choice sets while preserving counts and dropping
            // LLM: impossible rows.
            map_parts_with(constraint, &cancellation, |pairs| {
                let choices = fibers_with(pairs, &cancellation)?;
                report_part()?;
                Ok(LabelSet::new(choices.iter().map(|set| ids[set]).collect()))
            })?
        }
    };

    // LLM: Index set-labels by each old output once. The existential image of an
    // LLM: old part O is precisely the new sets S with S intersecting O.

    let mut containing = vec![Vec::new(); problem.output().labels().len()];
    for (set, &id) in &ids {
        for label in set.iter() {
            cancellation.check()?;
            containing[label.0 as usize].push(id);
        }
    }
    let containing: Vec<_> = containing.into_iter().map(LabelSet::new).collect();
    // LLM: Restrict existential conversion to required active degrees without changing the
    // LLM: source graph class.
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
    // LLM: Substitute whole choice sets while preserving counts and dropping impossible rows.
    let passive = map_parts_with(&old_active, &cancellation, |old| {
        // LLM: Union the new set labels that contain any allowed old output from this part.
        let mut choices = LabelSet::default();
        for label in old.iter() {
            cancellation.check()?;
            choices = choices.union(&containing[label.0 as usize]);
        }
        report_part()?;
        Ok(choices)
    })?;
    // LLM: Preserve the incidence permission relation while constructing the transformed
    // LLM: problem.
    let mapping = match problem.mapping() {
        OutputMapping::Unrestricted => OutputMapping::Unrestricted,
        OutputMapping::Restricted(entries) => {
            // LLM: For each original input, permit exactly the new output sets wholly
            // LLM: contained in its old permissions.
            let mut images = Vec::with_capacity(entries.len());
            for allowed in entries {
                // LLM: Union the new set labels that contain any allowed old output from this
                // LLM: part.
                let mut choices = Vec::new();
                for (set, &id) in &ids {
                    cancellation.check()?;
                    if set.is_subset_of(allowed) {
                        choices.push(id);
                    }
                }
                images.push(LabelSet::new(choices));
            }
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            OutputMapping::Restricted(images)
        }
    };
    // LLM: Never infer a weaker input promise from the feasible joint relation.
    // LLM: The physical promise is preserved, with its side roles exchanged.

    let input = match problem.input() {
        Input::Trivial => Input::Trivial,
        Input::Explicit(input) => Input::Explicit(ConstraintPair::new(
            input.labels().to_vec(),
            input.passive().clone(),
            input.active().clone(),
        )?),
    };
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let names = (0..sets.len()).map(label_name).collect();
    let output = ConstraintPair::new(names, active, passive)?;
    // LLM: Build the new problem with exchanged active/passive degrees and the original input
    // LLM: promise with its side roles swapped.
    let transformed = Problem::new(
        GraphClass::new(
            problem.graph_class().passive().to_vec(),
            problem.graph_class().active().to_vec(),
        ),
        input,
        output,
        mapping,
    )?;
    // LLM: An impossible part discards its entire configuration, resolving the
    // LLM: remaining parts without visiting them. All parts are decided at this point.

    computation.report_work(
        Phase::Transforming,
        total.min(u64::from(u32::MAX)) as u32,
        WorkProgress {
            completed: total,
            total,
            pass: 1,
        },
    )?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(transformed)
}

/// LLM: Collect and deterministically intern the nonempty output sets actually needed by the
/// LLM: universal side. Used by RE; joint parts contribute their per-input fibers.
fn set_alphabet_with(
    maximum: &MaximizationResult,
    cancellation: &Cancellation,
) -> Result<Vec<LabelSet>, Cancelled> {
    // LLM: Collect canonical working sets before assigning deterministic local IDs.
    let mut sets = BTreeSet::new();
    match maximum {
        MaximizationResult::OutputOnly(constraint) => {
            // LLM: Process each condensed part without expanding its repeated occurrences.
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
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(sets.into_iter().collect())
}

/// LLM: Group a joint part by input and return its distinct sets of permitted outputs without
/// LLM: enumerating input configurations. Used by input-aware RE to construct set labels.
fn fibers_with(
    pairs: &LabelSet<InputOutputLabel>,
    cancellation: &Cancellation,
) -> Result<Vec<LabelSet>, Cancelled> {
    // LLM: Partition joint labels by input, then use each group's outputs as one ordinary set
    // LLM: label.
    let mut fibers = BTreeMap::<_, Vec<_>>::new();
    for pair in pairs.iter() {
        cancellation.check()?;
        fibers.entry(pair.input).or_default().push(pair.output);
    }
    Ok(fibers.into_values().map(LabelSet::new).collect())
}

// LLM: Compact fresh names avoid nesting source-set names on every RE iteration.
/// LLM: Assign short deterministic fresh output names, using a numeric fallback after the
/// LLM: initial character alphabet. Used by RE to avoid recursively nesting source-set names.
fn label_name(index: usize) -> String {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    const CHARACTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    CHARACTERS
        .get(index)
        .map_or_else(|| format!("L{index}"), |&c| char::from(c).to_string())
}

#[cfg(test)]
mod tests;
