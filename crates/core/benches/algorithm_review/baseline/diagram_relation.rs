//! LLM: Compare occurrence profiles or exact residual context languages.

use crate::{
    algorithms::{
        configurations::{contexts_with, covers_with},
        parallel::map_with_progress,
    },
    computation::{Computation, Phase},
    model::{Constraint, LabelSet},
};
use std::collections::BTreeSet;

/// LLM: Compute candidate label-replacement arrows from occurrence profiles and, when
/// LLM: requested, exact residual-language coverage. Used by output and joint diagram
/// LLM: construction with caller-selected target ranges.
pub(super) fn compute<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    exact: bool,
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, crate::computation::Cancelled> {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = computation.cancellation.clone();
    let mut occurrences = vec![BTreeSet::new(); labels.len()];
    // LLM: Process each condensed part without expanding its repeated occurrences.
    for (part, group) in constraint
        .configurations()
        .flat_map(|family| family.parts())
        .enumerate()
    {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        for label in group.labels().iter() {
            if let Ok(index) = labels.binary_search(&label) {
                occurrences[index].insert(part);
            }
        }
    }
    // LLM: Scheduling was slower for cheap 64- and 256-label fixtures. Estimate
    // LLM: occurrence comparisons, not just label count, so large profiles can still
    // LLM: use workers. This is a scheduling heuristic, never a computation cutoff.

    let occurrence_work = occurrences
        .iter()
        .fold(0usize, |sum, set| sum.saturating_add(set.len()))
        .saturating_mul(labels.len());
    let large_table = occurrence_work >= 1_000_000;
    // LLM: Prepare the residual language obtained by deleting one occurrence of the compared
    // LLM: label.
    let contexts = if exact {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Preparing, 0)?;
        map_with_progress(
            labels.len(),
            &cancellation,
            large_table,
            |index| contexts_with(constraint, labels[index], &cancellation),
            |_| computation.report(Phase::Preparing, 0),
        )?
    } else {
        Vec::new()
    };
    // LLM: Degree-two passive relations reduce to degree-one contexts. Cache their
    // LLM: unions once rather than repeatedly invoking general coverage or flow.

    let small = exact && constraint.degrees().iter().all(|group| group.degree() <= 2);
    // LLM: Summarize equivalent occurrence/residual profiles so repeated label behavior can
    // LLM: share work.
    let profiles: Vec<_> = if small {
        // LLM: Update the current working state before continuing this operation.
        contexts
            .iter()
            .map(|context| {
                let mut labels = LabelSet::default();
                for family in context.configurations_at_degree(1) {
                    labels = labels.union(family.parts()[0].labels());
                }
                (!context.configurations_at_degree(0).is_empty(), labels)
            })
            .collect()
    } else {
        Vec::new()
    };
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Comparing, 0)?;
    map_with_progress(
        labels.len(),
        &cancellation,
        large_table || (exact && !small),
        |source| {
            // LLM: Prepare `row` for the following computation; it stores the row for this
            // LLM: stage.
            let mut row = Vec::new();
            for target in candidates(source) {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                let holds = if occurrences[source].is_subset(&occurrences[target]) {
                    true
                } else if !exact {
                    false
                } else if small {
                    (!profiles[source].0 || profiles[target].0)
                        && profiles[source].1.is_subset_of(&profiles[target].1)
                } else {
                    // LLM: Prepare `covered` for the following computation; it stores the
                    // LLM: covered for this stage.
                    let mut covered = true;
                    for family in contexts[source].configurations() {
                        if !covers_with(&contexts[target], family, &cancellation)? {
                            covered = false;
                            break;
                        }
                    }
                    covered
                };
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if holds {
                    row.push(target)
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed.min(u32::MAX as usize) as u32),
    )
}
