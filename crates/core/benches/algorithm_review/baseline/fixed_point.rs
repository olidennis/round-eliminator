//! LLM: Temporary implementation of the old Basic default-lattice construction.
//! LLM: Active closure uses the dual set order; passive closure uses inclusion.
//! LLM: Empty lattice values are valid labels, not empty model parts.

mod lattice;

use crate::{
    algorithms::{
        configurations::concrete_multisets_with,
        diagram::{
            self, DiagramKind, DiagramOptions, InputOutputDiagramMethod, OutputDiagramMethod,
        },
        maximize::maximize_custom_with,
    },
    computation::{Computation, Phase},
    model::{
        CondensedConfiguration, Configuration, Constraint, ConstraintPair, LabelId, LabelSet,
        OutputMapping, Part, Problem,
    },
};
use lattice::{Lattice, LatticeOperations};

/// LLM: Construct the temporary Basic fixed-point problem and run its output-independent
/// LLM: cleanup. Used by the Basic FP operation; restricted input-dependent mappings are
/// LLM: unsupported here.
pub fn basic_with(problem: &Problem, computation: &mut Computation<'_>) -> anyhow::Result<Problem> {
    // LLM: Prepare `candidate` for the following computation; it stores the candidate for this
    // LLM: stage.
    let candidate = construct_with(problem, computation)?;
    crate::algorithms::simplification::discard_useless_with(&candidate, computation)
}

/// LLM: Build the diagram-generated lattice and the standard/dual counted closures of the two
/// LLM: sides. Used internally by Basic FP before discard-useless simplification.
fn construct_with(problem: &Problem, computation: &mut Computation<'_>) -> anyhow::Result<Problem> {
    // LLM: Reject input-dependent output permissions because this temporary Basic construction
    // LLM: uses an output-only lattice.
    anyhow::ensure!(
        matches!(problem.mapping(), OutputMapping::Unrestricted),
        "Basic FP currently requires output validity independent of inputs."
    );
    // LLM: Use the original representation-dependent partial output diagram that defines the
    // LLM: Basic construction.
    let diagram = diagram::compute_with(
        problem,
        DiagramOptions {
            kind: DiagramKind::OutputOnly,
            output: OutputDiagramMethod::Partial,
            input_output: InputOutputDiagramMethod::Cheap,
        },
        computation,
    )?
    .diagram;
    // LLM: Build the finite diagram-generated lattice before replacing original labels by
    // LLM: principal elements.
    computation.report(Phase::Preparing, 0)?;
    let lattice = Lattice::new(&diagram.successors, computation)?;
    let active = seeds(
        problem.output().active(),
        problem.graph_class().active(),
        &lattice,
        computation,
    )?;
    // LLM: Prepare passive seeds at the independently declared passive degrees.
    let passive = seeds(
        problem.output().passive(),
        problem.graph_class().passive(),
        &lattice,
        computation,
    )?;
    let active = close(&active, &lattice, true, Phase::ClosingActive, computation)?;
    // LLM: Close the passive side in ordinary inclusion order; the active side used the dual
    // LLM: order.
    let passive = close(
        &passive,
        &lattice,
        false,
        Phase::ClosingPassive,
        computation,
    )?;

    // LLM: Update the current working state before continuing this operation.
    computation.report(Phase::Transforming, 0)?;
    let convert =
        |rows: Vec<Configuration<LabelId>>, passive: bool| -> anyhow::Result<Constraint> {
            // LLM: Prepare `configurations` for the following computation; it stores the
            // LLM: configurations for this stage.
            let mut configurations = Vec::new();
            for row in rows {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                computation.cancellation.check()?;
                let mut parts = Vec::new();
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for &(label, count) in row.multiplicities() {
                    // LLM: Expose all lattice subsets as passive choices, while active values
                    // LLM: become singleton output labels.
                    let choices = if passive {
                        lattice.subsets(label)
                    } else {
                        LabelSet::new(vec![label])
                    };
                    parts.push(Part::new(choices, count)?);
                }
                // LLM: Retain the newly constructed choices in the result for this stage.
                configurations.push(CondensedConfiguration::new(parts)?);
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(Constraint::new(configurations))
        };
    // LLM: Translate both completed algebraic closures back to model constraints with their
    // LLM: counts unchanged.
    let active = convert(active, false)?;
    let passive = convert(passive, true)?;
    // LLM: Keep classical single-character labels compact, as in the old UI.
    // LLM: Quoting general names keeps different sets distinct despite punctuation.

    let compact_names = problem
        .output()
        .labels()
        .iter()
        .all(|name| name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric());
    // LLM: Build readable names in the same order as the corresponding stored IDs.
    let names = lattice
        .sets
        .iter()
        .map(|set| {
            // LLM: Handle the empty case explicitly before the general traversal or allocation
            // LLM: path.
            if set.is_empty() {
                "∅".to_owned()
            } else if compact_names {
                // LLM: Sort single-character member names before concatenation to keep lattice
                // LLM: names readable and deterministic.
                let mut members: Vec<_> = set
                    .iter()
                    .map(|id| problem.output().labels()[id.0 as usize].as_str())
                    .collect();
                members.sort_unstable();
                members.concat()
            } else {
                // LLM: Quote general source names so punctuation cannot make distinct lattice
                // LLM: sets share the same displayed name.
                format!(
                    "{{{}}}",
                    set.iter()
                        .map(
                            |id| serde_json::to_string(&problem.output().labels()[id.0 as usize])
                                .unwrap()
                        )
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        })
        .collect();
    // LLM: Prepare required output data without modifying the original problem snapshot.
    let output = ConstraintPair::new(names, active, passive)?;
    Ok(Problem::new(
        problem.graph_class().clone(),
        problem.input().clone(),
        output,
        OutputMapping::Unrestricted,
    )?)
}

/// LLM: Enumerate concrete seeds at the selected degrees and replace each original label by
/// LLM: its principal lattice element. Used by Basic FP; repeated occurrences retain their
/// LLM: counts.
fn seeds(
    constraint: &Constraint,
    degrees: &[u32],
    lattice: &Lattice,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelId>>> {
    // LLM: Prepare counted rows or remaining row capacities for the next exact comparison.
    let rows = concrete_multisets_with(constraint, degrees, &computation.cancellation)?;
    rows.into_iter()
        .map(|row| {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            computation.cancellation.check()?;
            Ok(Configuration::new(
                row.multiplicities()
                    .iter()
                    .map(|&(label, count)| (lattice.principal[label.0 as usize], count))
                    .collect(),
            )?)
        })
        .collect()
}

/// LLM: Run one fixed-point closure using the standard or dual lattice algebra and translate
/// LLM: its result to model choices. Used once for each side of Basic FP.
fn close(
    initial: &[Configuration<LabelId>],
    lattice: &Lattice,
    dual: bool,
    phase: Phase,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelId>>> {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = computation.cancellation.clone();
    let mut report = |mut progress: crate::computation::Progress| {
        // LLM: Translate generic maximization progress into the active/passive closure phase
        // LLM: used by the caller.
        if progress.phase == Phase::Maximizing {
            progress.phase = phase;
        }
        // LLM: The nested context checks the same cancellation flag again after
        // LLM: this callback, so callback-triggered cancellation propagates normally.

        let _ = computation.report_progress(progress);
    };
    // LLM: Compute exact closure under the selected standard or dual algebra.
    Ok(maximize_custom_with(
        initial,
        &LatticeOperations { lattice, dual },
        &mut Computation::new(cancellation, &mut report),
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        computation::Cancellation,
        parser::{ProblemText, parse_problem},
    };

    /// LLM: Parse a JSON fixture into a checked canonical problem. Used by Basic FP tests
    /// LLM: before invoking the construction.
    fn parsed(value: serde_json::Value) -> Problem {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        parse_problem(serde_json::from_value::<ProblemText>(value).unwrap()).unwrap()
    }

    /// LLM: Regression test: verify that singleton fixed point retains empty lattice label and
    /// LLM: passive downward choices. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn singleton_fixed_point_retains_empty_lattice_label_and_passive_downward_choices() {
        // LLM: Prepare `problem` for the following computation; it stores the problem for this
        // LLM: stage.
        let problem = parsed(
            serde_json::json!({"kind":"plain","constraints":{"active":"A^2","passive":"A^2"}}),
        );
        let mut events = Vec::new();
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = construct_with(
            &problem,
            &mut Computation::new(Cancellation::default(), &mut |p| events.push(p)),
        )
        .unwrap();
        assert_eq!(result.output().labels(), &["∅", "A"]);
        let active = &result.output().active().configurations_at_degree(2)[0];
        assert_eq!(active.parts()[0].labels(), &LabelSet::new(vec![LabelId(1)]));
        // LLM: Prepare `passive` for the following computation; it stores the passive for this
        // LLM: stage.
        let passive = &result.output().passive().configurations_at_degree(2)[0];
        assert_eq!(
            passive.parts()[0].labels(),
            &LabelSet::new(vec![LabelId(0), LabelId(1)])
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            events
                .iter()
                .any(|p| p.phase == Phase::ClosingActive && p.work.is_some())
        );
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(
            events
                .iter()
                .any(|p| p.phase == Phase::ClosingPassive && p.work.is_some())
        );
    }

    /// LLM: Regression test: verify that independent inputs and graph class are preserved and
    /// LLM: restricted inputs are rejected. Used by the test suite to protect this behavior
    /// LLM: while implementations change.
    #[test]
    fn independent_inputs_and_graph_class_are_preserved_and_restricted_inputs_are_rejected() {
        // LLM: Prepare `p` for the following computation; it stores the p for this stage.
        let p = parsed(
            serde_json::json!({"kind":"independent","input":{"active":"i","passive":"i^2"},
            "output":{"active":"A","passive":"A^2"},"degrees":{"active":"0 1 3","passive":"0 2 4"}}),
        );
        // LLM: Collect the computed values, preserving their association with the requested
        // LLM: work items.
        let result = basic_with(
            &p,
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        assert_eq!(result.graph_class(), p.graph_class());
        assert_eq!(result.input(), p.input());
        // LLM: Prepare `mapped` for the following computation; it stores the mapped for this
        // LLM: stage.
        let mapped = parsed(
            serde_json::json!({"kind":"mapped","input":{"active":"i","passive":"i^2"},
            "output":{"active":"A","passive":"A^2"},"mapping":"i -> A"}),
        );
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(
            basic_with(
                &mapped,
                &mut Computation::new(Cancellation::default(), &mut |_| {})
            )
            .is_err()
        );
        let cancellation = Cancellation::default();
        // LLM: Update the current working state before continuing this operation.
        cancellation.cancel();
        assert!(basic_with(&p, &mut Computation::new(cancellation, &mut |_| {})).is_err());
    }
}
