//! Compare occurrence profiles or exact residual context languages.

use crate::{
    algorithms::{
        configurations::{contexts_with, covers_with},
        parallel::map_with_progress,
    },
    computation::{Computation, Phase},
    model::{Constraint, LabelSet},
};
use std::collections::BTreeSet;

pub(super) fn compute<L: Copy + Ord + Send + Sync>(
    constraint: &Constraint<L>,
    labels: &[L],
    exact: bool,
    candidates: impl Fn(usize) -> std::ops::Range<usize> + Sync,
    computation: &mut Computation<'_>,
) -> Result<Vec<Vec<usize>>, crate::computation::Cancelled> {
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
    // Scheduling was slower for cheap 64- and 256-label fixtures. Estimate
    // occurrence comparisons, not just label count, so large profiles can still
    // use workers. This is a scheduling heuristic, never a computation cutoff.
    let occurrence_work = occurrences
        .iter()
        .fold(0usize, |sum, set| sum.saturating_add(set.len()))
        .saturating_mul(labels.len());
    let large_table = occurrence_work >= 1_000_000;
    let contexts = if exact {
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
    // Degree-two passive relations reduce to degree-one contexts. Cache their
    // unions once rather than repeatedly invoking general coverage or flow.
    let small = exact && constraint.degrees().iter().all(|group| group.degree() <= 2);
    let profiles: Vec<_> = if small {
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
                } else if small {
                    (!profiles[source].0 || profiles[target].0)
                        && profiles[source].1.is_subset_of(&profiles[target].1)
                } else {
                    let mut covered = true;
                    for family in contexts[source].configurations() {
                        if !covers_with(&contexts[target], family, &cancellation)? {
                            covered = false;
                            break;
                        }
                    }
                    covered
                };
                if holds {
                    row.push(target)
                }
            }
            Ok(row)
        },
        |completed| computation.report(Phase::Comparing, completed.min(u32::MAX as usize) as u32),
    )
}
