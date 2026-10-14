//! Compare occurrence profiles or exact residual context languages.

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

struct Context<L> {
    residual: Constraint<L>,
    unary: Option<(bool, LabelSet<L>)>,
}

pub(super) fn compute<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    exact: bool,
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, Cancelled> {
    let cancellation = computation.cancellation.clone();
    let mut occurrences = vec![BTreeSet::new(); labels.len()];
    for (part, group) in constraint
        .configurations()
        .flat_map(|family| family.parts())
        .enumerate()
    {
        cancellation.check()?;
        for label in group.labels().iter() {
            if let Ok(index) = labels.binary_search(&label) {
                occurrences[index].insert(part);
            }
        }
    }
    let occurrence_work = occurrences
        .iter()
        .fold(0usize, |sum, set| sum.saturating_add(set.len()))
        .saturating_mul(labels.len());
    let large_table = occurrence_work >= 1_000_000;
    let small = exact && constraint.degrees().iter().all(|group| group.degree() <= 2);
    // Equal occurrence profiles give identical residual languages: deleting one
    // occurrence removes the same part position, regardless of the label name.
    // OnceLock shares preparation between worker rows and caches cancellation.
    let mut profiles = BTreeMap::new();
    let mut groups = Vec::new();
    let mut representatives = Vec::new();
    if exact {
        for (index, profile) in occurrences.iter().enumerate() {
            cancellation.check()?;
            let next = representatives.len();
            let group = *profiles.entry(profile).or_insert_with(|| {
                representatives.push(index);
                next
            });
            groups.push(group);
        }
    }
    if small && representatives.len() > labels.len() / 2 {
        return unary_rows(
            constraint,
            labels,
            &occurrences,
            candidates,
            large_table,
            computation,
        );
    }
    let contexts: Vec<OnceLock<Result<Context<L>, Cancelled>>> = (0..representatives.len())
        .map(|_| OnceLock::new())
        .collect();
    let context = |index: usize| -> Result<&Context<L>, Cancelled> {
        contexts[groups[index]]
            .get_or_init(|| {
                let residual = contexts_with(
                    constraint,
                    labels[representatives[groups[index]]],
                    &cancellation,
                )?;
                let unary = if small {
                    let mut union = LabelSet::default();
                    for family in residual.configurations_at_degree(1) {
                        cancellation.check()?;
                        union = union.union(family.parts()[0].labels());
                    }
                    Some((!residual.configurations_at_degree(0).is_empty(), union))
                } else {
                    None
                };
                Ok(Context { residual, unary })
            })
            .as_ref()
            .map_err(|_| Cancelled)
    };
    // Repeated profile pairs have the same coverage answer. Use a shared table
    // when grouping removes at least half the labels; otherwise avoid allocating
    // a quadratic cache for a handful of aliases.
    let comparisons = (exact && representatives.len() <= labels.len() / 2).then(|| {
        (0..representatives.len())
            .map(|_| {
                (0..representatives.len())
                    .map(|_| OnceLock::<Result<bool, Cancelled>>::new())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    let compare = |source: usize, target: usize| -> Result<bool, Cancelled> {
        let source = context(source)?;
        let target = context(target)?;
        if let (Some(a), Some(b)) = (&source.unary, &target.unary) {
            return Ok((!a.0 || b.0) && a.1.is_subset_of(&b.1));
        }
        for family in source.residual.configurations() {
            if !covers_with(&target.residual, family, &cancellation)? {
                return Ok(false);
            }
        }
        Ok(true)
    };
    if exact {
        computation.report(Phase::Preparing, 0)?;
    }
    computation.report(Phase::Comparing, 0)?;
    map_with_progress(
        labels.len(),
        &cancellation,
        large_table || (exact && !small),
        |source| {
            let mut row = Vec::new();
            for target in candidates(source) {
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
                if holds {
                    row.push(target);
                }
            }
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed.min(u32::MAX as usize) as u32),
    )
}

/// Distinct degree-two profiles are already cheap: retain direct vector reads
/// rather than paying shared lazy-cache lookup costs at every comparison.
fn unary_rows<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    occurrences: &[BTreeSet<usize>],
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    parallel: bool,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, Cancelled> {
    let cancellation = computation.cancellation.clone();
    computation.report(Phase::Preparing, 0)?;
    let contexts = map_with_progress(
        labels.len(),
        &cancellation,
        parallel,
        |index| contexts_with(constraint, labels[index], &cancellation),
        |_| computation.report(Phase::Preparing, 0),
    )?;
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
    computation.report(Phase::Comparing, 0)?;
    map_with_progress(
        labels.len(),
        &cancellation,
        parallel,
        |source| {
            let mut row = Vec::new();
            for target in candidates(source) {
                cancellation.check()?;
                if occurrences[source].is_subset(&occurrences[target])
                    || ((!profiles[source].0 || profiles[target].0)
                        && profiles[source].1.is_subset_of(&profiles[target].1))
                {
                    row.push(target);
                }
            }
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed as u32),
    )
}
