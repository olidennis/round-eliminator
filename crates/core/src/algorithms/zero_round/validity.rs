//! Shared zero-round validity checks and output domains for labels and pairs.

use std::collections::BTreeSet;

use crate::algorithms::configurations::accepts_with;
use crate::computation::{Cancellation, Cancelled};
use crate::labels::{LabelId, LabelPair};
use crate::problem::{Configuration, Constraint, Problem};

/// Validity can constrain outputs alone or input/output pairs. Both search and
/// witness checking borrow the original constraints through this small selector.
#[derive(Clone, Copy)]
pub(super) enum Validity<'a> {
    Labels(&'a Constraint),
    Pairs(&'a Constraint<LabelPair>),
}

impl<'a> Validity<'a> {
    /// Return active and passive validity, in that order.
    pub(super) fn for_problem(problem: &'a Problem) -> (Self, Self) {
        match problem {
            Problem::Plain(problem) => (
                Self::Labels(problem.output().active()),
                Self::Labels(problem.output().passive()),
            ),
            Problem::Independent(problem) => (
                Self::Labels(problem.output().active()),
                Self::Labels(problem.output().passive()),
            ),
            Problem::Mapped(problem) => (
                Self::Labels(problem.output().active()),
                Self::Labels(problem.output().passive()),
            ),
            Problem::Paired(problem) => (
                Self::Pairs(problem.constraints().active()),
                Self::Pairs(problem.constraints().passive()),
            ),
        }
    }

    pub(super) fn accepts(self, input: &[LabelId], output: &[LabelId]) -> bool {
        self.accepts_with(input, output, &Cancellation::default())
            .expect("not cancelled")
    }

    pub(super) fn accepts_with(
        self,
        input: &[LabelId],
        output: &[LabelId],
        cancellation: &Cancellation,
    ) -> Result<bool, Cancelled> {
        match self {
            Self::Labels(constraint) => accepts_with(
                constraint,
                &Configuration::from_occurrences(output.iter().copied())
                    .expect("rule degree fits u32"),
                cancellation,
            ),
            Self::Pairs(constraint) => accepts_with(
                constraint,
                &Configuration::from_occurrences(
                    input
                        .iter()
                        .zip(output)
                        .map(|(&input, &output)| LabelPair { input, output }),
                )
                .expect("rule degree fits u32"),
                cancellation,
            ),
        }
    }

    pub(super) fn domain(self, degree: u32, input: Option<LabelId>) -> BTreeSet<LabelId> {
        match self {
            Self::Labels(constraint) => constraint
                .configurations_at_degree(degree)
                .iter()
                .flat_map(|configuration| configuration.parts())
                .flat_map(|part| part.labels().iter())
                .collect(),
            Self::Pairs(constraint) => constraint
                .configurations_at_degree(degree)
                .iter()
                .flat_map(|configuration| configuration.parts())
                .flat_map(|part| part.labels().iter())
                .filter(|pair| Some(pair.input) == input)
                .map(|pair| pair.output)
                .collect(),
        }
    }
}

pub(super) fn mapped_output_allowed(
    problem: &Problem,
    input: Option<LabelId>,
    output: LabelId,
) -> bool {
    problem
        .allowed_outputs()
        .is_none_or(|mapping| input.is_some_and(|input| mapping[input.0 as usize].contains(output)))
}
