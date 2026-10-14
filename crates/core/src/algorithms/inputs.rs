//! LLM: Original input configurations shared by diagram and solvability analyses.

use crate::algorithms::configurations::concrete_multisets_with;
use crate::computation::{Cancellation, Cancelled};
use crate::model::{CondensedConfiguration, Input, LabelId, LabelSet, Part, Problem};

/// LLM: One active-node input context at a specified degree. `Some(labels)` is the canonical
/// LLM: occurrence vector of an explicit input multiset; `None` represents trivial input
/// LLM: without allocating dummy occurrences.
pub(crate) struct LocalInput {
    pub degree: u32,
    pub labels: Option<Vec<LabelId>>,
}

/// LLM: Prepare canonical active input multisets at the declared degrees, or one implicit
/// LLM: context per degree for trivial input. Shared by solvability encodings and witness
/// LLM: checks.
pub(crate) fn active_inputs(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<LocalInput>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    Ok(match problem.input() {
        Input::Explicit(input) => {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            concrete_multisets_with(input.active(), problem.graph_class().active(), cancellation)?
                .into_iter()
                .map(|input| LocalInput {
                    degree: input.degree(),
                    labels: Some(input.occurrences().collect()),
                })
                .collect()
        }
        Input::Trivial => problem
            .graph_class()
            .active()
            .iter()
            .map(|&degree| LocalInput {
                degree,
                labels: None,
            })
            .collect(),
    })
}

/// LLM: Extract passive input families at the declared degrees and retain their condensed
/// LLM: parts. Shared by diagrams, maximization, solvability, and verification; `None`
/// LLM: represents trivial input.
pub(crate) fn prepare_passive_inputs(
    problem: &Problem,
    cancellation: &Cancellation,
) -> Result<Vec<CondensedConfiguration<Option<LabelId>>>, Cancelled> {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check()?;
    let graph = problem.graph_class();
    let mut configurations = Vec::new();
    // LLM: Select the representation path that preserves the supplied input promise and
    // LLM: incidence permissions.
    match problem.input() {
        Input::Explicit(input) => {
            // LLM: Process each condensed part without expanding its repeated occurrences.
            for &degree in graph.passive() {
                // LLM: Process each condensed part without expanding its repeated occurrences.
                for configuration in input.passive().configurations_at_degree(degree) {
                    // LLM: Observe cooperative interruption before continuing this unit of
                    // LLM: work.
                    cancellation.check()?;
                    configurations.push(
                        CondensedConfiguration::new(
                            configuration
                                .parts()
                                .iter()
                                .map(|part| {
                                    Part::new(
                                        LabelSet::new(part.labels().iter().map(Some).collect()),
                                        part.multiplicity(),
                                    )
                                    .expect("nonempty label set")
                                })
                                .collect(),
                        )
                        .expect("degree preserved"),
                    );
                }
            }
        }
        Input::Trivial => {
            // LLM: Process the relevant label choices while preserving their table identities.
            for &degree in graph.passive() {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                configurations.push(
                    CondensedConfiguration::new(vec![
                        Part::new(LabelSet::new(vec![None]), degree).expect("nonempty label set"),
                    ])
                    .expect("single count fits u32"),
                );
            }
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(configurations)
}
