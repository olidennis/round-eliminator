//! Exact universal passive checking and counterexample clauses.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use crate::computation::Cancellation;
use rustsat::types::Lit;

use super::{
    LocalRule, prepare,
    validity::{self, Validity},
};
use crate::labels::LabelId;
use crate::problem::{Configuration, Problem};

// The search attaches a SAT literal as provenance. Independent witness
// verification uses (), sharing the exact check without constructing literals.
#[derive(Clone, Copy)]
pub(super) struct Emission<T = Lit> {
    pub input: Option<LabelId>,
    pub output: LabelId,
    pub source: T,
}

pub(super) fn forbidden_combination<T: Copy + Ord>(
    validity: Validity<'_>,
    inputs: &[Configuration<Option<LabelId>>],
    emissions: &[Emission<T>],
    cancellation: &Cancellation,
) -> anyhow::Result<Option<Vec<T>>> {
    for input in inputs {
        let mut found = None;
        visit(validity, input, emissions, cancellation, &mut |clause| {
            found = Some(clause);
            Ok(ControlFlow::Break(()))
        })?;
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

pub(super) fn visit_forbidden(
    validity: Validity<'_>,
    input: &Configuration<Option<LabelId>>,
    emissions: &[Emission],
    cancellation: &Cancellation,
    mut visitor: impl FnMut(Vec<Lit>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    visit(validity, input, emissions, cancellation, &mut |clause| {
        visitor(clause)?;
        Ok(ControlFlow::Continue(()))
    })
}

fn visit<T: Copy + Ord>(
    validity: Validity<'_>,
    input: &Configuration<Option<LabelId>>,
    emissions: &[Emission<T>],
    cancellation: &Cancellation,
    visitor: &mut impl FnMut(Vec<T>) -> anyhow::Result<ControlFlow<()>>,
) -> anyhow::Result<()> {
    cancellation.check()?;
    let input: Vec<_> = input.occurrences().collect();
    let choices: Vec<Vec<_>> = input
        .iter()
        .map(|input| {
            emissions
                .iter()
                .filter(|emission| emission.input == *input)
                .collect()
        })
        .collect();
    if choices.iter().any(Vec::is_empty) {
        // No compatible active neighbor can realize this passive configuration.
        return Ok(());
    }
    let input_labels: Vec<_> = input.iter().flatten().copied().collect();
    let mut indices = vec![0; choices.len()];
    loop {
        cancellation.check()?;
        let selected: Vec<_> = choices
            .iter()
            .zip(&indices)
            .map(|(choices, &index)| choices[index])
            .collect();
        let output: Vec<_> = selected.iter().map(|emission| emission.output).collect();
        if !validity.accepts_with(&input_labels, &output, cancellation)? {
            let mut clause: Vec<_> = selected.iter().map(|emission| emission.source).collect();
            clause.sort_unstable();
            clause.dedup();
            if visitor(clause)?.is_break() {
                return Ok(());
            }
        }
        let Some(position) = indices
            .iter()
            .enumerate()
            .rposition(|(position, &index)| index + 1 < choices[position].len())
        else {
            break;
        };
        indices[position] += 1;
        // Inputs arrive sorted. For repeated inputs, permutations of the
        // outputs describe the same pair multiset, so keep indices sorted
        // within each equal-input block instead of revisiting permutations.
        for next in position + 1..indices.len() {
            indices[next] = if input[next] == input[next - 1] {
                indices[next - 1]
            } else {
                0
            };
        }
    }
    Ok(())
}

/// Independently check a witness, including context coverage, active validity,
/// mapped restrictions, and all compatible passive combinations. This checker
/// uses no SAT solver and also accepts witnesses obtained by other means.
pub fn verify_strategy(problem: &Problem, strategy: &[LocalRule]) -> bool {
    let cancellation = Cancellation::default();
    let (active, passive) = Validity::for_problem(problem);
    let mut expected: BTreeSet<_> = prepare::contexts(problem, &cancellation)
        .expect("not cancelled")
        .into_iter()
        .map(|rule| (rule.degree, rule.input))
        .collect();
    if expected.len() != strategy.len() {
        return false;
    }
    let mut emissions = BTreeMap::new();
    for rule in strategy {
        if !expected.remove(&(rule.degree, rule.input.clone()))
            || rule.output.len() != rule.degree as usize
        {
            return false;
        }
        let input = rule.input.as_deref().unwrap_or(&[]);
        if !active.accepts(input, &rule.output) {
            return false;
        }
        for (position, &output) in rule.output.iter().enumerate() {
            let input = rule.input.as_ref().map(|input| input[position]);
            if !validity::mapped_output_allowed(problem, input, output) {
                return false;
            }
            emissions.entry((input, output)).or_insert(Emission {
                input,
                output,
                source: (),
            });
        }
    }
    forbidden_combination(
        passive,
        &prepare::passive_inputs(problem, &cancellation).expect("not cancelled"),
        &emissions.into_values().collect::<Vec<_>>(),
        &cancellation,
    )
    .expect("not cancelled")
    .is_none()
}
