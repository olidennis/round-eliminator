//! Temporary implementation of the old Basic default-lattice construction.
//! Active closure uses the dual set order; passive closure uses inclusion.
//! Empty lattice values are valid labels, not empty model parts.

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

/// Construct a candidate using the sound default output diagram. Output-independent
/// input promises are retained. Dependent inputs need their own FP definition.
pub fn basic_with(problem: &Problem, computation: &mut Computation<'_>) -> anyhow::Result<Problem> {
    let candidate = construct_with(problem, computation)?;
    crate::algorithms::simplification::discard_useless_with(&candidate, computation)
}

fn construct_with(problem: &Problem, computation: &mut Computation<'_>) -> anyhow::Result<Problem> {
    anyhow::ensure!(
        matches!(problem.mapping(), OutputMapping::Unrestricted),
        "Basic FP currently requires output validity independent of inputs."
    );
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
    computation.report(Phase::Preparing, 0)?;
    let lattice = Lattice::new(&diagram.successors, computation)?;
    let active = seeds(
        problem.output().active(),
        problem.graph_class().active(),
        &lattice,
        computation,
    )?;
    let passive = seeds(
        problem.output().passive(),
        problem.graph_class().passive(),
        &lattice,
        computation,
    )?;
    let active = close(&active, &lattice, true, Phase::ClosingActive, computation)?;
    let passive = close(
        &passive,
        &lattice,
        false,
        Phase::ClosingPassive,
        computation,
    )?;

    computation.report(Phase::Transforming, 0)?;
    let convert =
        |rows: Vec<Configuration<LabelId>>, passive: bool| -> anyhow::Result<Constraint> {
            let mut configurations = Vec::new();
            for row in rows {
                computation.cancellation.check()?;
                let mut parts = Vec::new();
                for &(label, count) in row.multiplicities() {
                    let choices = if passive {
                        lattice.subsets(label)
                    } else {
                        LabelSet::new(vec![label])
                    };
                    parts.push(Part::new(choices, count)?);
                }
                configurations.push(CondensedConfiguration::new(parts)?);
            }
            Ok(Constraint::new(configurations))
        };
    let active = convert(active, false)?;
    let passive = convert(passive, true)?;
    // Keep classical single-character labels compact, as in the old UI.
    // Quoting general names keeps different sets distinct despite punctuation.
    let compact_names = problem
        .output()
        .labels()
        .iter()
        .all(|name| name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric());
    let names = lattice
        .sets
        .iter()
        .map(|set| {
            if set.is_empty() {
                "∅".to_owned()
            } else if compact_names {
                let mut members: Vec<_> = set
                    .iter()
                    .map(|id| problem.output().labels()[id.0 as usize].as_str())
                    .collect();
                members.sort_unstable();
                members.concat()
            } else {
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
    let output = ConstraintPair::new(names, active, passive)?;
    Ok(Problem::new(
        problem.graph_class().clone(),
        problem.input().clone(),
        output,
        OutputMapping::Unrestricted,
    )?)
}

fn seeds(
    constraint: &Constraint,
    degrees: &[u32],
    lattice: &Lattice,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelId>>> {
    let rows = concrete_multisets_with(constraint, degrees, &computation.cancellation)?;
    rows.into_iter()
        .map(|row| {
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

fn close(
    initial: &[Configuration<LabelId>],
    lattice: &Lattice,
    dual: bool,
    phase: Phase,
    computation: &mut Computation<'_>,
) -> anyhow::Result<Vec<Configuration<LabelId>>> {
    let cancellation = computation.cancellation.clone();
    let mut report = |mut progress: crate::computation::Progress| {
        if progress.phase == Phase::Maximizing {
            progress.phase = phase;
        }
        // The nested context checks the same cancellation flag again after
        // this callback, so callback-triggered cancellation propagates normally.
        let _ = computation.report_progress(progress);
    };
    let mut context = Computation::new(cancellation, &mut report);
    Ok(if let Some(operations) = lattice.table_operations(dual) {
        maximize_custom_with(initial, &operations, &mut context, None)?
    } else {
        maximize_custom_with(
            initial,
            &LatticeOperations { lattice, dual },
            &mut context,
            None,
        )?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        computation::Cancellation,
        parser::{ProblemText, parse_problem},
    };

    fn parsed(value: serde_json::Value) -> Problem {
        parse_problem(serde_json::from_value::<ProblemText>(value).unwrap()).unwrap()
    }

    #[test]
    fn singleton_fixed_point_retains_empty_lattice_label_and_passive_downward_choices() {
        let problem = parsed(
            serde_json::json!({"kind":"plain","constraints":{"active":"A^2","passive":"A^2"}}),
        );
        let mut events = Vec::new();
        let result = construct_with(
            &problem,
            &mut Computation::new(Cancellation::default(), &mut |p| events.push(p)),
        )
        .unwrap();
        assert_eq!(result.output().labels(), &["∅", "A"]);
        let active = &result.output().active().configurations_at_degree(2)[0];
        assert_eq!(active.parts()[0].labels(), &LabelSet::new(vec![LabelId(1)]));
        let passive = &result.output().passive().configurations_at_degree(2)[0];
        assert_eq!(
            passive.parts()[0].labels(),
            &LabelSet::new(vec![LabelId(0), LabelId(1)])
        );
        assert!(
            events
                .iter()
                .any(|p| p.phase == Phase::ClosingActive && p.work.is_some())
        );
        assert!(
            events
                .iter()
                .any(|p| p.phase == Phase::ClosingPassive && p.work.is_some())
        );
    }

    #[test]
    fn independent_inputs_and_graph_class_are_preserved_and_restricted_inputs_are_rejected() {
        let p = parsed(
            serde_json::json!({"kind":"independent","input":{"active":"i","passive":"i^2"},
            "output":{"active":"A","passive":"A^2"},"degrees":{"active":"0 1 3","passive":"0 2 4"}}),
        );
        let result = basic_with(
            &p,
            &mut Computation::new(Cancellation::default(), &mut |_| {}),
        )
        .unwrap();
        assert_eq!(result.graph_class(), p.graph_class());
        assert_eq!(result.input(), p.input());
        let mapped = parsed(
            serde_json::json!({"kind":"mapped","input":{"active":"i","passive":"i^2"},
            "output":{"active":"A","passive":"A^2"},"mapping":"i -> A"}),
        );
        assert!(
            basic_with(
                &mapped,
                &mut Computation::new(Cancellation::default(), &mut |_| {})
            )
            .is_err()
        );
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(basic_with(&p, &mut Computation::new(cancellation, &mut |_| {})).is_err());
    }
}
