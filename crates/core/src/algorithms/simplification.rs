//! LLM: Output-independent simplification used by the Basic fixed-point button.
//! LLM: Strict passive replacements preserve solutions when weakening active choices.

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

/// LLM: Repeat the Basic FP cleanup: remove strictly weaker active choices, dominated rows,
/// LLM: and labels absent from either side until stable. Used after fixed-point construction
/// LLM: and preserves the input promise and graph class.
pub(crate) fn discard_useless_with(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Problem> {
    // LLM: Update the current working state before continuing this operation.
    anyhow::ensure!(
        matches!(problem.mapping(), OutputMapping::Unrestricted),
        "Output-only simplification requires input-independent output validity."
    );
    let mut current = problem.clone();
    // LLM: Try a single-row containment proof using counted subset-compatible matching.
    loop {
        // LLM: Prepare required output data without modifying the original problem snapshot.
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
        // LLM: Prepare `diagram` for the following computation; it stores the diagram for this
        // LLM: stage.
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
        // LLM: Include all reachable replacements, including the source itself, in the working
        // LLM: order.
        let successors = reachability_with(
            &diagram
                .successors
                .iter()
                .map(|row| row.iter().map(|&i| i as usize).collect())
                .collect::<Vec<_>>(),
            &computation.cancellation,
        )?;
        // LLM: Treat A as strictly weaker only when A reaches B and B cannot reach A;
        // LLM: equivalent labels are not strictly weaker.
        let strict = |a: LabelId, b: LabelId| {
            // LLM: Update the current working state before continuing this operation.
            a != b
                && successors[a.0 as usize]
                    .binary_search(&(b.0 as usize))
                    .is_ok()
                && successors[b.0 as usize]
                    .binary_search(&(a.0 as usize))
                    .is_err()
        };
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Cleaning, 0)?;
        let mut rows = Vec::new();
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for row in current.output().active().configurations() {
            // LLM: Prepare `parts` for the following computation; it stores the parts for this
            // LLM: stage.
            let mut parts = Vec::new();
            for part in row.parts() {
                // LLM: Remove a choice only when the same part also offers a strictly stronger
                // LLM: replacement.
                let mut labels = Vec::new();
                for a in part.labels().iter() {
                    computation.cancellation.check()?;
                    if !part.labels().iter().any(|b| strict(a, b)) {
                        labels.push(a);
                    }
                }
                parts.push(Part::new(LabelSet::new(labels), part.multiplicity())?);
            }
            // LLM: Retain the newly constructed choices in the result for this stage.
            rows.push(CondensedConfiguration::new(parts)?);
        }
        // LLM: Use exact counted matching so repeated occurrences do not need a
        // LLM: port-permutation expansion.
        let active = prune_with(
            &Constraint::new(rows),
            &computation.cancellation,
            |target, family| {
                // LLM: Prepare `offered` for the following computation; it stores the offered
                // LLM: for this stage.
                let offered: Vec<_> = family
                    .parts()
                    .iter()
                    .map(|p| (p.labels(), p.multiplicity()))
                    .collect();
                // LLM: Prepare `allowed` for the following computation; it stores the allowed
                // LLM: for this stage.
                let allowed: Vec<_> = target
                    .parts()
                    .iter()
                    .map(|p| (p.labels(), p.multiplicity()))
                    .collect();
                // LLM: Use exact counted matching so repeated occurrences do not need a
                // LLM: port-permutation expansion.
                matches_counts_with(
                    &offered,
                    &allowed,
                    |a, b| a.iter().all(|x| b.iter().any(|y| x == y || strict(x, y))),
                    &computation.cancellation,
                )
            },
        )?;
        // LLM: Try a single-row containment proof using counted subset-compatible matching.
        let passive = prune_with(
            current.output().passive(),
            &computation.cancellation,
            |target, family| contains_condensed_with(target, family, &computation.cancellation),
        )?;
        let (active, passive) = common_labels_with(active, passive, &computation.cancellation)?;
        // LLM: Stop cleanup when neither side changes; otherwise recompute the diagram of the
        // LLM: new constraint pair.
        if &active == current.output().active() && &passive == current.output().passive() {
            break;
        }
        // LLM: Update the current working state before continuing this operation.
        current = Problem::new(
            current.graph_class().clone(),
            current.input().clone(),
            ConstraintPair::new(current.output().labels().to_vec(), active, passive)?,
            OutputMapping::Unrestricted,
        )?;
    }
    // LLM: Compact IDs only after the constraints stabilize; graph degrees and the
    // LLM: original input promise remain independently stored in the result.

    let used = LabelSet::new(
        current
            .output()
            .active()
            .labels()
            .chain(current.output().passive().labels())
            .collect(),
    );
    // LLM: Build the local value-to-ID table so storage order and parallel completion cannot
    // LLM: change identity.
    let mut ids = vec![None; current.output().labels().len()];
    let mut names = Vec::new();
    for label in used.iter() {
        computation.cancellation.check()?;
        ids[label.0 as usize] = Some(LabelId(u32::try_from(names.len())?));
        names.push(current.output().labels()[label.0 as usize].clone());
    }
    // LLM: Prepare `remap` for the following computation; it stores the remap for this stage.
    let remap = |set: &LabelSet| {
        Ok(LabelSet::new(
            set.iter().map(|id| ids[id.0 as usize].unwrap()).collect(),
        ))
    };
    let active = map_parts_with(current.output().active(), &computation.cancellation, remap)?;
    let passive = map_parts_with(current.output().passive(), &computation.cancellation, remap)?;
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Problem::new(
        current.graph_class().clone(),
        current.input().clone(),
        ConstraintPair::new(names, active, passive)?,
        OutputMapping::Unrestricted,
    )?)
}

/// LLM: Restrict both sides to their common labels repeatedly, dropping rows made impossible.
/// LLM: Used by Basic FP cleanup because removing a row can make another label one-sided.
fn common_labels_with(
    mut active: Constraint,
    mut passive: Constraint,
    cancellation: &Cancellation,
) -> Result<(Constraint, Constraint), crate::computation::Cancelled> {
    // LLM: Substitute whole choice sets while preserving counts and dropping impossible rows.
    loop {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let keep = LabelSet::new(active.labels().collect())
            .intersection(&LabelSet::new(passive.labels().collect()));
        let next_active = map_parts_with(&active, cancellation, |set| Ok(set.intersection(&keep)))?;
        let next_passive =
            map_parts_with(&passive, cancellation, |set| Ok(set.intersection(&keep)))?;
        // LLM: Return only when filtering has stabilized on both sides, including labels lost
        // LLM: through impossible rows.
        if next_active == active && next_passive == passive {
            return Ok((active, passive));
        }
        active = next_active;
        passive = next_passive;
    }
}

/// LLM: Remove same-degree rows dominated under a supplied relation, keeping one deterministic
/// LLM: representative of equivalent rows. Used by active and passive simplification with
/// LLM: their respective containment semantics.
fn prune_with(
    constraint: &Constraint,
    cancellation: &Cancellation,
    mut dominates: impl FnMut(
        &CondensedConfiguration,
        &CondensedConfiguration,
    ) -> Result<bool, crate::computation::Cancelled>,
) -> Result<Constraint, crate::computation::Cancelled> {
    // LLM: Prepare `kept` for the following computation; it stores the kept for this stage.
    let mut kept = Vec::new();
    for group in constraint.degrees() {
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for (index, row) in group.configurations().iter().enumerate() {
            // LLM: Look for a distinct same-degree dominator, breaking equivalent-row ties by
            // LLM: their original order.
            let mut redundant = false;
            for (other, target) in group.configurations().iter().enumerate() {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                if other != index
                    && dominates(target, row)?
                    && (other < index || !dominates(row, target)?)
                {
                    redundant = true;
                    break;
                }
            }
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if !redundant {
                kept.push(row.clone());
            }
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(Constraint::new(kept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{algorithms::configurations::concrete_multisets, parser::parse_problem};
    /// LLM: Regression test: verify that equivalent rows keep one and side only labels are
    /// LLM: removed until stable. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn equivalent_rows_keep_one_and_side_only_labels_are_removed_until_stable() {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = parse_problem(
            serde_json::from_value(serde_json::json!({"kind":"plain",
            "constraints":{"active":"A B\nB A\nC^2","passive":"AB^2"}}))
            .unwrap(),
        )
        .unwrap();
        // LLM: Remove output choices and rows that are redundant after fixed-point
        // LLM: construction.
        let result = discard_useless_with(
            &p,
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        assert_eq!(result.graph_class(), p.graph_class());
        assert_eq!(result.output().labels(), &["A", "B"]);
        assert_eq!(concrete_multisets(result.output().active(), &[2]).len(), 1);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            result.output().passive().configurations_at_degree(2).len(),
            1
        );
    }
}
