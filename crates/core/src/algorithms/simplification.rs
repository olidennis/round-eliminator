//! Output-independent simplification used by the Basic fixed-point button.
//! Strict passive replacements preserve solutions when weakening active choices.

use crate::{
    algorithms::{
        configurations::{contains_condensed_with, map_parts_with, matches_counts_with},
        diagram::{
            self, DiagramKind, DiagramOptions, InputOutputDiagramMethod, OutputDiagramMethod,
        },
        graphs::reachability_with,
    },
    computation::{Cancellation, Computation, Phase},
    model::{
        CondensedConfiguration, Constraint, ConstraintPair, LabelId, LabelSet, OutputMapping, Part,
        Problem,
    },
};

/// Match the old button's cleanup: remove strict weak active choices, dominated
/// rows, and labels used on only one side, repeating as the diagram changes.
pub(crate) fn discard_useless_with(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Problem> {
    anyhow::ensure!(
        matches!(problem.mapping(), OutputMapping::Unrestricted),
        "Output-only simplification requires input-independent output validity."
    );
    let mut current = problem.clone();
    loop {
        let output = if current
            .graph_class()
            .passive()
            .iter()
            .all(|&degree| degree == 2)
        {
            OutputDiagramMethod::Exact
        } else {
            OutputDiagramMethod::Partial
        };
        let diagram = diagram::compute_with(
            &current,
            DiagramOptions {
                kind: DiagramKind::OutputOnly,
                output,
                input_output: InputOutputDiagramMethod::Cheap,
            },
            computation,
        )?
        .diagram;
        let successors = reachability_with(
            &diagram
                .successors
                .iter()
                .map(|row| row.iter().map(|&i| i as usize).collect())
                .collect::<Vec<_>>(),
            &computation.cancellation,
        )?;
        let strict = |a: LabelId, b: LabelId| {
            a != b
                && successors[a.0 as usize]
                    .binary_search(&(b.0 as usize))
                    .is_ok()
                && successors[b.0 as usize]
                    .binary_search(&(a.0 as usize))
                    .is_err()
        };
        computation.report(Phase::Cleaning, 0)?;
        let mut rows = Vec::new();
        for row in current.output().active().configurations() {
            let mut parts = Vec::new();
            for part in row.parts() {
                let mut labels = Vec::new();
                for a in part.labels().iter() {
                    computation.cancellation.check()?;
                    if !part.labels().iter().any(|b| strict(a, b)) {
                        labels.push(a);
                    }
                }
                parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
            }
            rows.push(CondensedConfiguration::new(parts)?);
        }
        let active = prune_with(
            &Constraint::new(rows),
            &computation.cancellation,
            |target, family| {
                let offered: Vec<_> = family
                    .parts()
                    .iter()
                    .map(|p| (p.labels(), p.multiplicity()))
                    .collect();
                let allowed: Vec<_> = target
                    .parts()
                    .iter()
                    .map(|p| (p.labels(), p.multiplicity()))
                    .collect();
                matches_counts_with(
                    &offered,
                    &allowed,
                    |a, b| a.iter().all(|x| b.iter().any(|y| x == y || strict(x, y))),
                    &computation.cancellation,
                )
            },
        )?;
        let passive = prune_with(
            current.output().passive(),
            &computation.cancellation,
            |target, family| contains_condensed_with(target, family, &computation.cancellation),
        )?;
        let (active, passive) = common_labels_with(active, passive, &computation.cancellation)?;
        if &active == current.output().active() && &passive == current.output().passive() {
            break;
        }
        current = Problem::new(
            current.graph_class().clone(),
            current.input().clone(),
            ConstraintPair::new(current.output().labels().to_vec(), active, passive)?,
            OutputMapping::Unrestricted,
        )?;
    }
    // Compact IDs only after the constraints stabilize; graph degrees and the
    // original input promise remain independently stored in the result.
    let used = LabelSet::new(
        current
            .output()
            .active()
            .labels()
            .chain(current.output().passive().labels())
            .collect(),
    );
    let mut ids = vec![None; current.output().labels().len()];
    let mut names = Vec::new();
    for label in used.iter() {
        computation.cancellation.check()?;
        ids[label.0 as usize] = Some(LabelId(u32::try_from(names.len())?));
        names.push(current.output().labels()[label.0 as usize].clone());
    }
    let remap = |set: &LabelSet| {
        Ok(LabelSet::new(
            set.iter().map(|id| ids[id.0 as usize].unwrap()).collect(),
        ))
    };
    let active = map_parts_with(current.output().active(), &computation.cancellation, remap)?;
    let passive = map_parts_with(current.output().passive(), &computation.cancellation, remap)?;
    Ok(Problem::new(
        current.graph_class().clone(),
        current.input().clone(),
        ConstraintPair::new(names, active, passive)?,
        OutputMapping::Unrestricted,
    )?)
}

fn common_labels_with(
    mut active: Constraint,
    mut passive: Constraint,
    cancellation: &Cancellation,
) -> Result<(Constraint, Constraint), crate::computation::Cancelled> {
    loop {
        cancellation.check()?;
        let keep = LabelSet::new(active.labels().collect())
            .intersection(&LabelSet::new(passive.labels().collect()));
        let next_active = map_parts_with(&active, cancellation, |set| Ok(set.intersection(&keep)))?;
        let next_passive =
            map_parts_with(&passive, cancellation, |set| Ok(set.intersection(&keep)))?;
        if next_active == active && next_passive == passive {
            return Ok((active, passive));
        }
        active = next_active;
        passive = next_passive;
    }
}

fn prune_with(
    constraint: &Constraint,
    cancellation: &Cancellation,
    mut dominates: impl FnMut(
        &CondensedConfiguration,
        &CondensedConfiguration,
    ) -> Result<bool, crate::computation::Cancelled>,
) -> Result<Constraint, crate::computation::Cancelled> {
    let mut kept = Vec::new();
    for group in constraint.degrees() {
        for (index, row) in group.configurations().iter().enumerate() {
            let mut redundant = false;
            for (other, target) in group.configurations().iter().enumerate() {
                cancellation.check()?;
                if other != index
                    && dominates(target, row)?
                    && (other < index || !dominates(row, target)?)
                {
                    redundant = true;
                    break;
                }
            }
            if !redundant {
                kept.push(row.clone());
            }
        }
    }
    Ok(Constraint::new(kept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{algorithms::configurations::concrete_multisets, parser::parse_problem};
    #[test]
    fn equivalent_rows_keep_one_and_side_only_labels_are_removed_until_stable() {
        let p = parse_problem(
            serde_json::from_value(serde_json::json!({"kind":"plain",
            "constraints":{"active":"A B\nB A\nC^2","passive":"AB^2"}}))
            .unwrap(),
        )
        .unwrap();
        let result = discard_useless_with(
            &p,
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        assert_eq!(result.graph_class(), p.graph_class());
        assert_eq!(result.output().labels(), &["A", "B"]);
        assert_eq!(concrete_multisets(result.output().active(), &[2]).len(), 1);
        assert_eq!(
            result.output().passive().configurations_at_degree(2).len(),
            1
        );
    }
}
