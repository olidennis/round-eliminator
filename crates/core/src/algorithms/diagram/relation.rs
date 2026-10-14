//! LLM: Compare occurrence profiles or exact residual context languages.

use crate::{
    algorithms::{
        configurations::{contexts_with, covers_with},
        parallel::map_with_progress,
    },
    computation::{Cancelled, Computation, Phase},
    model::{Constraint, LabelSet},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

/// LLM: The residual allowed language after deleting one occurrence of a diagram label. When
/// LLM: residual degrees are at most one, an optional empty-row flag and label union provide a
/// LLM: cheaper exact inclusion test.
struct Context<L> {
    residual: Constraint<L>,
    unary: Option<(bool, LabelSet<L>)>,
}

/// LLM: Compute candidate label-replacement arrows from occurrence profiles and, when
/// LLM: requested, exact residual-language coverage. Used by output and joint diagram
/// LLM: construction with caller-selected target ranges.
pub(super) fn compute<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    exact: bool,
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, Cancelled> {
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
    // LLM: Prepare `occurrence_work` for the following computation; it stores the occurrence
    // LLM: work for this stage.
    let occurrence_work = occurrences
        .iter()
        .fold(0usize, |sum, set| sum.saturating_add(set.len()))
        .saturating_mul(labels.len());
    let large_table = occurrence_work >= 1_000_000;
    let small = exact && constraint.degrees().iter().all(|group| group.degree() <= 2);
    // LLM: Equal occurrence profiles give identical residual languages: deleting one
    // LLM: occurrence removes the same part position, regardless of the label name.
    // LLM: OnceLock shares preparation between worker rows and caches cancellation.

    let mut profiles = BTreeMap::new();
    let mut groups = Vec::new();
    let mut representatives = Vec::new();
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if exact {
        // LLM: Visit the remaining alternatives for this stage, stopping when the required
        // LLM: search condition is settled.
        for (index, profile) in occurrences.iter().enumerate() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            let next = representatives.len();
            let group = *profiles.entry(profile).or_insert_with(|| {
                representatives.push(index);
                next
            });
            groups.push(group);
        }
    }
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if small && representatives.len() > labels.len() / 2 {
        // LLM: Return the result of this completed search or conversion to its caller.
        return unary_rows(
            constraint,
            labels,
            &occurrences,
            candidates,
            large_table,
            computation,
        );
    }
    // LLM: Store the residual languages used to decide exact replacement arrows.
    let contexts: Vec<OnceLock<Result<Context<L>, Cancelled>>> = (0..representatives.len())
        .map(|_| OnceLock::new())
        .collect();
    // LLM: Prepare the residual language obtained by deleting one occurrence of the compared
    // LLM: label.
    let context = |index: usize| -> Result<&Context<L>, Cancelled> {
        // LLM: Prepare the residual language obtained by deleting one occurrence of the
        // LLM: compared label.
        contexts[groups[index]]
            .get_or_init(|| {
                // LLM: Prepare the residual language obtained by deleting one occurrence of
                // LLM: the compared label.
                let residual = contexts_with(
                    constraint,
                    labels[representatives[groups[index]]],
                    &cancellation,
                )?;
                // LLM: For degree-two constraints, reduce each residual language to its
                // LLM: empty-row flag and unary label union.
                let unary = if small {
                    // LLM: Prepare `union` for the following computation; it stores the union
                    // LLM: for this stage.
                    let mut union = LabelSet::default();
                    for family in residual.configurations_at_degree(1) {
                        cancellation.check()?;
                        union = union.union(family.parts()[0].labels());
                    }
                    Some((!residual.configurations_at_degree(0).is_empty(), union))
                } else {
                    None
                };
                // LLM: Return the result of this completed search or conversion to its caller.
                Ok(Context { residual, unary })
            })
            .as_ref()
            .map_err(|_| Cancelled)
    };
    // LLM: Repeated profile pairs have the same coverage answer. Use a shared table
    // LLM: when grouping removes at least half the labels; otherwise avoid allocating
    // LLM: a quadratic cache for a handful of aliases.

    let comparisons = (exact && representatives.len() <= labels.len() / 2).then(|| {
        // LLM: Update the current working state before continuing this operation.
        (0..representatives.len())
            .map(|_| {
                (0..representatives.len())
                    .map(|_| OnceLock::<Result<bool, Cancelled>>::new())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    // LLM: Check full residual-language coverage when occurrence containment alone is
    // LLM: insufficient.
    let compare = |source: usize, target: usize| -> Result<bool, Cancelled> {
        // LLM: Create the source capacity side of the matching network.
        let source = context(source)?;
        let target = context(target)?;
        if let (Some(a), Some(b)) = (&source.unary, &target.unary) {
            return Ok((!a.0 || b.0) && a.1.is_subset_of(&b.1));
        }
        // LLM: Check full residual-language coverage when occurrence containment alone is
        // LLM: insufficient.
        for family in source.residual.configurations() {
            if !covers_with(&target.residual, family, &cancellation)? {
                return Ok(false);
            }
        }
        Ok(true)
    };
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if exact {
        computation.report(Phase::Preparing, 0)?;
    }
    computation.report(Phase::Comparing, 0)?;
    // LLM: Observe cooperative interruption before continuing this unit of work.
    map_with_progress(
        labels.len(),
        &cancellation,
        large_table || (exact && !small),
        |source| {
            // LLM: Compute one successor row, trying sound occurrence containment before any
            // LLM: exact context work.
            let mut row = Vec::new();
            for target in candidates(source) {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                let holds = if occurrences[source].is_subset(&occurrences[target]) {
                    true
                } else if !exact {
                    false
                } else if let Some(table) = &comparisons {
                    table[groups[source]][groups[target]]
                        .get_or_init(|| compare(source, target))
                        .as_ref()
                        .copied()
                        .map_err(|_| Cancelled)?
                } else {
                    compare(source, target)?
                };
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if holds {
                    row.push(target);
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed.min(u32::MAX as usize) as u32),
    )
}

/// LLM: Compare exact residual languages with degree zero/one using direct label-set profiles.
/// LLM: Used by exact degree-two diagrams when few occurrence profiles can be shared.
fn unary_rows<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    occurrences: &[BTreeSet<usize>],
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    parallel: bool,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, Cancelled> {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = computation.cancellation.clone();
    computation.report(Phase::Preparing, 0)?;
    // LLM: Prepare the residual language obtained by deleting one occurrence of the compared
    // LLM: label.
    let contexts = map_with_progress(
        labels.len(),
        &cancellation,
        parallel,
        |index| contexts_with(constraint, labels[index], &cancellation),
        |_| computation.report(Phase::Preparing, 0),
    )?;
    // LLM: Summarize equivalent occurrence/residual profiles so repeated label behavior can
    // LLM: share work.
    let profiles: Vec<_> = contexts
        .iter()
        .map(|context| {
            let mut labels = LabelSet::default();
            for family in context.configurations_at_degree(1) {
                labels = labels.union(family.parts()[0].labels());
            }
            (!context.configurations_at_degree(0).is_empty(), labels)
        })
        .collect();
    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Comparing, 0)?;
    map_with_progress(
        labels.len(),
        &cancellation,
        parallel,
        |source| {
            // LLM: Prepare `row` for the following computation; it stores the row for this
            // LLM: stage.
            let mut row = Vec::new();
            for target in candidates(source) {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                if occurrences[source].is_subset(&occurrences[target])
                    || ((!profiles[source].0 || profiles[target].0)
                        && profiles[source].1.is_subset_of(&profiles[target].1))
                {
                    row.push(target);
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed as u32),
    )
}
