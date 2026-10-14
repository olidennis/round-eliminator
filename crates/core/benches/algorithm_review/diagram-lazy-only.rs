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
    for (index, profile) in occurrences.iter().enumerate() {
        cancellation.check()?;
        let next = representatives.len();
        let group = *profiles.entry(profile).or_insert_with(|| {
            representatives.push(index);
            next
        });
        groups.push(group);
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
                } else {
                    let source = context(source)?;
                    let target = context(target)?;
                    if let (Some(a), Some(b)) = (&source.unary, &target.unary) {
                        (!a.0 || b.0) && a.1.is_subset_of(&b.1)
                    } else {
                        let mut covered = true;
                        for family in source.residual.configurations() {
                            if !covers_with(&target.residual, family, &cancellation)? {
                                covered = false;
                                break;
                            }
                        }
                        covered
                    }
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
