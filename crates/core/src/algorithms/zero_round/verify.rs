//! Complete witness verification, independent of SAT and maximization assumptions.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    LocalRule, PassiveMode,
    passive::{Emission, forbidden_combination},
    prepare,
};
use crate::algorithms::configurations::accepts;
use crate::computation::Cancellation;
use crate::model::{Configuration, Problem};

/// Independently check a witness, including context coverage, active validity,
/// mapped restrictions, and all compatible passive combinations. This checker
/// uses no SAT solver and also accepts witnesses obtained by other means.
pub fn verify_strategy(problem: &Problem, strategy: &[LocalRule]) -> bool {
    let cancellation = Cancellation::default();
    let active = problem.output().active();
    let passive = problem.output().passive();
    let mut expected: BTreeSet<_> = prepare::prepare_local_rules(problem, &cancellation)
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
        if !accepts(
            active,
            &Configuration::from_occurrences(rule.output.iter().copied())
                .expect("rule degree fits u32"),
        ) {
            return false;
        }
        for (position, &output) in rule.output.iter().enumerate() {
            let input = rule.input.as_ref().map(|input| input[position]);
            if !problem.output_allowed(input, output) {
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
        &prepare::prepare_passive_inputs(problem, &cancellation).expect("not cancelled"),
        &emissions.into_values().collect::<Vec<_>>(),
        PassiveMode::General,
        &cancellation,
    )
    .expect("not cancelled")
    .is_none()
}
