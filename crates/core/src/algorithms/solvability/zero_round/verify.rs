//! LLM: Complete witness verification, independent of SAT and maximization assumptions.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    LocalRule, PassiveMode,
    passive::{Emission, forbidden_combination},
    prepare,
};
use crate::algorithms::configurations::accepts;
use crate::computation::Cancellation;
use crate::model::{Configuration, Problem};

/// LLM: Independently verify a zero-round witness against all active contexts, input
/// LLM: permissions, and compatible passive output combinations. Used by tests and callers
/// LLM: checking certificates without SAT.
pub fn verify_strategy(problem: &Problem, strategy: &[LocalRule]) -> bool {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = Cancellation::default();
    let active = problem.output().active();
    let passive = problem.output().passive();
    let mut expected: BTreeSet<_> = prepare::prepare_local_rules(problem, &cancellation)
        .expect("not cancelled")
        .into_iter()
        .map(|rule| (rule.degree, rule.input))
        .collect();
    // LLM: Require exactly one rule for each promised active context; omitted or duplicate
    // LLM: contexts invalidate the certificate.
    if expected.len() != strategy.len() {
        return false;
    }
    let mut emissions = BTreeMap::new();
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for rule in strategy {
        // LLM: Check rule identity, output length, and output references before validating
        // LLM: local constraints.
        if !expected.remove(&(rule.degree, rule.input.clone()))
            || rule.output.len() != rule.degree as usize
        {
            return false;
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if !accepts(
            active,
            &Configuration::from_occurrences(rule.output.iter().copied())
                .expect("rule degree fits u32"),
        ) {
            return false;
        }
        // LLM: Check each output's actual input permission and collect its possible passive
        // LLM: emission.
        for (position, &output) in rule.output.iter().enumerate() {
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let input = rule.input.as_ref().map(|input| input[position]);
            if !problem.output_allowed(input, output) {
                return false;
            }
            // LLM: Update the current working state before continuing this operation.
            emissions.entry((input, output)).or_insert(Emission {
                input,
                output,
                source: (),
            });
        }
    }
    // LLM: Find the emission origins of a passive violation so refinement blocks that
    // LLM: simultaneous choice.
    forbidden_combination(
        passive,
        &prepare::prepare_passive_inputs(problem, &cancellation).expect("not cancelled"),
        &emissions.into_values().collect::<Vec<_>>(),
        PassiveMode::General,
        &cancellation,
    )
    .expect("not cancelled")
    .is_none()
}
