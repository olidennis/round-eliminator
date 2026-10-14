//! LLM: Passive label replacement, independent of active output requirements.
//! LLM: Output-only diagrams ignore inputs. Input-output arrows keep the input fixed;
//! LLM: input-aware arrows must hold at every input admitting the source output.
//! LLM: Each request computes one relation and its reduced graph of equivalence components.

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

/// LLM: The output-only replacement test: an exact relation, or sound partial arrows from
/// LLM: occurrence containment. A compatible output maximum makes the occurrence test exact
/// LLM: without another maximization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputDiagramMethod {
    /// LLM: The occurrence test proves arrows but can omit valid ones.
    Partial,
    #[default]
    Exact,
}

/// LLM: The joint replacement test: lift established output arrows through input permissions
/// LLM: cheaply, or check every valid joint passive context exactly.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum InputOutputDiagramMethod {
    /// LLM: Lift established output arrows through the input permissions.

    #[default]
    Cheap,
    /// LLM: Check every admissible passive joint input/output context.
    Exact,
}

/// LLM: The meaning of diagram vertices and arrows. Output-only ignores inputs, pair vertices
/// LLM: keep the input fixed, and input-aware output arrows must hold at every input
/// LLM: permitting the source.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagramKind {
    /// LLM: Replace outputs without consulting the input promise or mapping.

    #[default]
    OutputOnly,
    /// LLM: Vertices are input-output pairs; replacements keep the input fixed.
    InputOutput,
    /// LLM: Vertices are outputs; replacements must work at every admitting input.
    InputAware,
}

/// LLM: The requested relation kind and its output/joint comparison methods. Only options that
/// LLM: affect that relation participate in cache equivalence; a compatible computed maximum
/// LLM: can make a partial occurrence test exact.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DiagramOptions {
    #[serde(default)]
    pub kind: DiagramKind,
    /// LLM: Output-only method, also used as the dependency of cheap input-output checks.
    pub output: OutputDiagramMethod,
    /// LLM: Used only for input-output pairs and input-aware outputs.
    pub input_output: InputOutputDiagramMethod,
}

impl DiagramOptions {
    /// LLM: A cached output maximum makes the occurrence test exact. Normalize the
    /// LLM: cache key too, so an earlier incomplete partial result is not reused. Used by
    /// LLM: label diagram computation.
    pub fn with_maximum(
        mut self,
        maximum: Option<&crate::algorithms::maximize::MaximizationResult>,
    ) -> Self {
        // LLM: A complete output-only maximum makes the occurrence shortcut exact, so
        // LLM: normalize both method and cache key.
        if matches!(
            maximum,
            Some(crate::algorithms::maximize::MaximizationResult::OutputOnly(
                _
            ))
        ) {
            self.output = OutputDiagramMethod::Exact;
        }
        // LLM: Update the current working state before continuing this operation.
        self
    }

    /// LLM: Ignore options that do not affect this requested relation. Used by label diagram
    /// LLM: computation.
    pub fn same_computation(self, other: Self) -> bool {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
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

/// LLM: A label replacement relation and its presentation graph. Nodes index the reflexive
/// LLM: successor rows; SCC lists group equivalent nodes, and reduced edges join SCCs. If
/// LLM: `complete` is false, absent arrows are unknown.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct Diagram {
    pub nodes: Vec<InputOutputLabel>,
    /// LLM: Full relation including reflexive arrows, indexed by `nodes`.
    pub successors: Vec<Vec<u32>>,
    /// LLM: SCC members are node indices. The UI displays one vertex per component.
    pub components: Vec<Vec<u32>>,
    /// LLM: Transitive reduction, indexed by `components`.
    pub edges: Vec<(u32, u32)>,
    /// LLM: False means missing arrows are unknown, not disproved.
    pub complete: bool,
}

/// LLM: The computed diagram paired with the effective options that produced it. The options
/// LLM: identify the artifact when storing or reusing it in client-owned analysis state.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct DiagramResult {
    pub options: DiagramOptions,
    pub diagram: Diagram,
}

/// LLM: Assemble the relation and its reduced SCC presentation into a diagram artifact. Used
/// LLM: after label comparison, retaining whether missing arrows are disproved or merely
/// LLM: unknown.
fn diagram(
    nodes: Vec<InputOutputLabel>,
    successors: Vec<Vec<usize>>,
    complete: bool,
    cancellation: &Cancellation,
) -> Result<Diagram, crate::computation::Cancelled> {
    // LLM: Reduce SCC display edges separately from the retained full reflexive relation.
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

/// LLM: Compute one requested diagram with a silent fresh computation context. Used by direct
/// LLM: Rust callers.
pub fn compute(problem: &Problem, options: DiagramOptions) -> anyhow::Result<DiagramResult> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    compute_with(
        problem,
        options,
        &mut Computation::new(Cancellation::default(), &mut |_| {}),
    )
}

/// LLM: Compute one requested diagram with progress/cancellation and no supplied maximum. Used
/// LLM: by direct callers and internal diagram consumers.
pub fn compute_with(
    problem: &Problem,
    options: DiagramOptions,
    computation: &mut Computation<'_>,
) -> anyhow::Result<DiagramResult> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    compute_with_maximum(problem, options, None, computation)
}

/// LLM: Compute only the requested relation and its necessary dependencies, reusing a
/// LLM: compatible supplied maximum. Used by the API; joint maxima certify joint relations,
/// LLM: never output-only ones.
pub fn compute_with_maximum(
    problem: &Problem,
    options: DiagramOptions,
    maximum: Option<&crate::algorithms::maximize::MaximizationResult>,
    computation: &mut Computation<'_>,
) -> anyhow::Result<DiagramResult> {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use crate::algorithms::maximize::MaximizationResult;
    let options = options.with_maximum(maximum);
    computation.report(Phase::Preparing, 0)?;
    let cancellation = computation.cancellation.clone();
    // LLM: Ignore output degrees outside the declared graph class, but retain every
    // LLM: output label as a vertex, including labels absent from passive constraints.

    let original_output = Constraint::new(
        problem
            .graph_class()
            .passive()
            .iter()
            .flat_map(|&degree| {
                // LLM: Update the current working state before continuing this operation.
                problem
                    .output()
                    .passive()
                    .configurations_at_degree(degree)
                    .iter()
                    .cloned()
            })
            .collect(),
    );
    // LLM: Reuse only an output-only maximum for output replacement; a joint certificate has a
    // LLM: different guarantee.
    let output_maximum = match maximum {
        Some(MaximizationResult::OutputOnly(constraint)) => Some(constraint),
        _ => None,
    };
    let output = output_maximum.unwrap_or(&original_output);
    let labels: Vec<_> = (0..problem.output().labels().len())
        .map(|i| LabelId(i as u32))
        .collect();
    // LLM: Create one output-only vertex per output ID, retaining labels absent from passive
    // LLM: rows.
    let output_nodes = || {
        // LLM: Update the current working state before continuing this operation.
        labels
            .iter()
            .map(|&output| InputOutputLabel {
                input: None,
                output,
            })
            .collect()
    };
    // LLM: Compute output arrows only when requested or needed by the cheap joint method.
    let output_only_rows = |computation: &mut Computation<'_>, exact| {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        relation::compute(
            output,
            &labels,
            exact && output_maximum.is_none(),
            |_| 0..labels.len(),
            computation,
        )
    };
    // LLM: Return the requested output-only diagram immediately without preparing input/output
    // LLM: relations.
    if options.kind == DiagramKind::OutputOnly {
        // LLM: Prepare counted rows or remaining row capacities for the next exact comparison.
        let rows = output_only_rows(computation, options.output == OutputDiagramMethod::Exact)?;
        computation.report(Phase::Reducing, 0)?;
        // LLM: Return the result of this completed search or conversion to its caller.
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
    // LLM: Create only input/output pair vertices admitted by the incidence mapping.
    let mut nodes = Vec::new();
    let inputs: Vec<_> = match problem.input() {
        Input::Trivial => vec![None],
        Input::Explicit(input) => (0..input.labels().len())
            .map(|i| Some(LabelId(i as u32)))
            .collect(),
    };
    // LLM: Process the relevant label choices while preserving their table identities.
    for &input in &inputs {
        // LLM: Process the relevant label choices while preserving their table identities.
        for &label in &labels {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if problem.output_allowed(input, label) {
                nodes.push(InputOutputLabel {
                    input,
                    output: label,
                });
            }
        }
    }
    // LLM: Recognize when joint replacement coincides with output replacement and no joint
    // LLM: product is needed.
    let trivial_unrestricted = matches!(problem.input(), Input::Trivial)
        && matches!(problem.mapping(), OutputMapping::Unrestricted);
    // LLM: Align counted input and output parts and retain only permitted joint label
    // LLM: combinations.
    let pair_rows = if trivial_unrestricted {
        // LLM: With no input restriction, the requested joint relation is output-only.

        output_only_rows(
            computation,
            options.input_output == InputOutputDiagramMethod::Exact
                || options.output == OutputDiagramMethod::Exact,
        )?
    } else if options.input_output == InputOutputDiagramMethod::Exact {
        // LLM: Exact joint replacement does not need a separate output-only diagram.

        computation.report(Phase::Preparing, 0)?;
        let prepared;
        // LLM: Align counted input and output parts and retain only permitted joint label
        // LLM: combinations.
        let joint = if let Some(MaximizationResult::InputOutput(constraint)) = maximum {
            constraint
        } else {
            // LLM: Retain the original passive promise as condensed input families.
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
            // LLM: Update the current working state before continuing this operation.
            &prepared
        };
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
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
        // LLM: Output-only arrows are a dependency here; do not reduce or return them.

        let rows = output_only_rows(computation, options.output == OutputDiagramMethod::Exact)?;
        // LLM: Update the current working state before continuing this operation.
        nodes
            .iter()
            .map(|a| {
                // LLM: Prepare `start` for the following computation; it stores the start for
                // LLM: this stage.
                let start = nodes.partition_point(|node| node.input < a.input);
                let end = nodes.partition_point(|node| node.input <= a.input);
                // LLM: Update the current working state before continuing this operation.
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
    // LLM: Mark whether missing arrows are disproved; cheap lifting can remain incomplete even
    // LLM: with exact output arrows.
    let complete = options.input_output == InputOutputDiagramMethod::Exact
        || ((output_maximum.is_some() || options.output != OutputDiagramMethod::Partial)
            && trivial_unrestricted);
    // LLM: Return the pair relation before doing the universal intersection needed only by
    // LLM: input-aware outputs.
    if options.kind == DiagramKind::InputOutput {
        computation.report(Phase::Reducing, 0)?;
        return Ok(DiagramResult {
            options,
            diagram: diagram(nodes, pair_rows, complete, &cancellation)?,
        });
    }
    // LLM: Missing targets block an input-aware arrow: an input admitting A but forbidding
    // LLM: B must not disappear from an intersection over the pair diagrams.

    let pair_index: std::collections::BTreeMap<_, _> = nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(i, node)| (node, i))
        .collect();
    let mut input_aware_rows = Vec::new();
    // LLM: Process the relevant label choices while preserving their table identities.
    for &a in &labels {
        // LLM: An output arrow must exist at every input permitting its source, including a
        // LLM: check that the target is permitted there.
        let mut row = Vec::new();
        for (index, &b) in labels.iter().enumerate() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            if inputs.iter().all(|&input| {
                // LLM: Stop this alternative when the required choice is absent, leaving the
                // LLM: enclosing traversal to backtrack or continue.
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
        // LLM: Retain the newly constructed choices in the result for this stage.
        input_aware_rows.push(row);
    }
    // LLM: Update the current working state before continuing this operation.
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
