//! Passive label replacement, independent of active output requirements.
//!
//! Output-only diagrams ignore inputs. Input-output arrows keep the input fixed;
//! input-aware arrows must hold at every input admitting the source output.
//! Each request computes one relation and its reduced graph of equivalence components.

mod relation;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    algorithms::{
        configurations::product_with, graphs::reduce_relation, inputs::prepare_passive_inputs,
    },
    computation::{Cancellation, Computation, Phase},
    model::{Constraint, Input, InputOutputLabel, LabelId, OutputMapping, Problem},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputDiagramMethod {
    /// The occurrence test proves arrows but can omit valid ones.
    Partial,
    /// Exactly the same test; caller guarantees a complete maximized constraint.
    Maximized,
    #[default]
    Exact,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum InputOutputDiagramMethod {
    /// Lift established output arrows through the input permissions.
    #[default]
    Cheap,
    /// Check every admissible passive joint input/output context.
    Exact,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagramKind {
    /// Replace outputs without consulting the input promise or mapping.
    #[default]
    OutputOnly,
    /// Vertices are input-output pairs; replacements keep the input fixed.
    InputOutput,
    /// Vertices are outputs; replacements must work at every admitting input.
    InputAware,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DiagramOptions {
    #[serde(default)]
    pub kind: DiagramKind,
    /// Output-only method, also used as the dependency of cheap input-output checks.
    pub output: OutputDiagramMethod,
    /// Used only for input-output pairs and input-aware outputs.
    pub input_output: InputOutputDiagramMethod,
}

impl DiagramOptions {
    /// Ignore options that do not affect this requested relation.
    pub fn same_computation(self, other: Self) -> bool {
        self.kind == other.kind
            && if self.kind == DiagramKind::OutputOnly {
                self.output == other.output
            } else {
                self.input_output == other.input_output
                    && (self.input_output == InputOutputDiagramMethod::Exact
                        || self.output == other.output)
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Diagram {
    pub nodes: Vec<InputOutputLabel>,
    /// Full relation including reflexive arrows, indexed by `nodes`.
    pub successors: Vec<Vec<u32>>,
    /// SCC members are node indices. The UI displays one vertex per component.
    pub components: Vec<Vec<u32>>,
    /// Transitive reduction, indexed by `components`.
    pub edges: Vec<(u32, u32)>,
    /// False means missing arrows are unknown, not disproved.
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DiagramResult {
    pub options: DiagramOptions,
    pub diagram: Diagram,
}

fn diagram(
    nodes: Vec<InputOutputLabel>,
    successors: Vec<Vec<usize>>,
    complete: bool,
    cancellation: &Cancellation,
) -> Result<Diagram, crate::computation::Cancelled> {
    let reduced = reduce_relation(&successors, cancellation)?;
    Ok(Diagram {
        nodes,
        successors: successors
            .into_iter()
            .map(|row| row.into_iter().map(|i| i as u32).collect())
            .collect(),
        components: reduced.components,
        edges: reduced.edges,
        complete,
    })
}

pub fn compute(problem: &Problem, options: DiagramOptions) -> anyhow::Result<DiagramResult> {
    compute_with(
        problem,
        options,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

pub fn compute_with(
    problem: &Problem,
    options: DiagramOptions,
    computation: &mut Computation<'_>,
) -> anyhow::Result<DiagramResult> {
    compute_with_maximum(problem, options, None, computation)
}

/// Reuse a complete maximum of this snapshot without computing a new one.
/// A joint maximum certifies only the joint relation, never output-only validity.
pub fn compute_with_maximum(
    problem: &Problem,
    options: DiagramOptions,
    maximum: Option<&crate::algorithms::maximize::MaximizationResult>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<DiagramResult> {
    use crate::algorithms::maximize::MaximizationResult;
    computation.report(Phase::Preparing, 0)?;
    let cancellation = computation.cancellation.clone();
    // Ignore output degrees outside the declared graph class, but retain every
    // output label as a vertex, including labels absent from passive constraints.
    let original_output = Constraint::new(
        problem
            .graph_class()
            .passive()
            .iter()
            .flat_map(|&degree| {
                problem
                    .output()
                    .passive()
                    .configurations_at_degree(degree)
                    .iter()
                    .cloned()
            })
            .collect(),
    );
    let output_maximum = match maximum {
        Some(MaximizationResult::OutputOnly(constraint)) => Some(constraint),
        _ => None,
    };
    let output = output_maximum.unwrap_or(&original_output);
    let labels: Vec<_> = (0..problem.output().labels().len())
        .map(|i| LabelId(i as u32))
        .collect();
    let output_nodes = || {
        labels
            .iter()
            .map(|&output| InputOutputLabel {
                input: None,
                output,
            })
            .collect()
    };
    let output_only_rows = |computation: &mut Computation<'_>, exact| {
        relation::compute(
            output,
            &labels,
            exact && output_maximum.is_none(),
            |_| 0..labels.len(),
            computation,
        )
    };
    if options.kind == DiagramKind::OutputOnly {
        let rows = output_only_rows(computation, options.output == OutputDiagramMethod::Exact)?;
        computation.report(Phase::Reducing, 0)?;
        return Ok(DiagramResult {
            options,
            diagram: diagram(
                output_nodes(),
                rows,
                output_maximum.is_some() || options.output != OutputDiagramMethod::Partial,
                &cancellation,
            )?,
        });
    }
    let mut nodes = Vec::new();
    let inputs: Vec<_> = match problem.input() {
        Input::Trivial => vec![None],
        Input::Explicit(input) => (0..input.labels().len())
            .map(|i| Some(LabelId(i as u32)))
            .collect(),
    };
    for &input in &inputs {
        for &label in &labels {
            cancellation.check()?;
            if problem.output_allowed(input, label) {
                nodes.push(InputOutputLabel {
                    input,
                    output: label,
                });
            }
        }
    }
    let trivial_unrestricted = matches!(problem.input(), Input::Trivial)
        && matches!(problem.mapping(), OutputMapping::Unrestricted);
    let pair_rows = if trivial_unrestricted {
        // With no input restriction, the requested joint relation is output-only.
        output_only_rows(
            computation,
            options.input_output == InputOutputDiagramMethod::Exact
                || options.output == OutputDiagramMethod::Exact,
        )?
    } else if options.input_output == InputOutputDiagramMethod::Exact {
        // Exact joint replacement does not need a separate output-only diagram.
        computation.report(Phase::Preparing, 0)?;
        let prepared;
        let joint = if let Some(MaximizationResult::InputOutput(constraint)) = maximum {
            constraint
        } else {
            let input = Constraint::new(prepare_passive_inputs(problem, &cancellation)?);
            prepared = product_with(
                &input,
                output,
                |input, output| {
                    problem
                        .output_allowed(input, output)
                        .then_some(InputOutputLabel { input, output })
                },
                &cancellation,
            )?;
            &prepared
        };
        relation::compute(
            joint,
            &nodes,
            !matches!(maximum, Some(MaximizationResult::InputOutput(_))),
            |source| {
                let input = nodes[source].input;
                nodes.partition_point(|node| node.input < input)
                    ..nodes.partition_point(|node| node.input <= input)
            },
            computation,
        )?
    } else {
        // Output-only arrows are a dependency here; do not reduce or return them.
        let rows = output_only_rows(computation, options.output == OutputDiagramMethod::Exact)?;
        nodes
            .iter()
            .map(|a| {
                let start = nodes.partition_point(|node| node.input < a.input);
                let end = nodes.partition_point(|node| node.input <= a.input);
                (start..end)
                    .filter(|&index| {
                        rows[a.output.0 as usize]
                            .binary_search(&(nodes[index].output.0 as usize))
                            .is_ok()
                    })
                    .collect()
            })
            .collect()
    };
    let complete = options.input_output == InputOutputDiagramMethod::Exact
        || ((output_maximum.is_some() || options.output != OutputDiagramMethod::Partial)
            && trivial_unrestricted);
    if options.kind == DiagramKind::InputOutput {
        computation.report(Phase::Reducing, 0)?;
        return Ok(DiagramResult {
            options,
            diagram: diagram(nodes, pair_rows, complete, &cancellation)?,
        });
    }
    // Missing targets block an input-aware arrow: an input admitting A but forbidding
    // B must not disappear from an intersection over the pair diagrams.
    let pair_index: std::collections::BTreeMap<_, _> = nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(i, node)| (node, i))
        .collect();
    let mut input_aware_rows = Vec::new();
    for &a in &labels {
        let mut row = Vec::new();
        for (index, &b) in labels.iter().enumerate() {
            cancellation.check()?;
            if inputs.iter().all(|&input| {
                let Some(&source) = pair_index.get(&InputOutputLabel { input, output: a }) else {
                    return true;
                };
                pair_index
                    .get(&InputOutputLabel { input, output: b })
                    .is_some_and(|target| pair_rows[source].binary_search(target).is_ok())
            }) {
                row.push(index)
            }
        }
        input_aware_rows.push(row);
    }
    computation.report(Phase::Reducing, 0)?;
    let result = DiagramResult {
        options,
        diagram: diagram(output_nodes(), input_aware_rows, complete, &cancellation)?,
    };
    cancellation.check()?;
    Ok(result)
}

#[cfg(test)]
mod tests;
