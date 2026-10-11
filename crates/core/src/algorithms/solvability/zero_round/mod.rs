//! Exact active-side, deterministic zero-round solvability in the port-numbering
//! model. A node knows its degree, incident inputs, and local port order.
//! Rules may distinguish occurrences of the same input label. Passive validity
//! must hold for every compatible combination of outputs from active nodes.

use super::constraint_encoding as encoding;
use super::passive;
mod prepare;
mod verify;

pub use verify::verify_strategy;

use std::collections::BTreeMap;
use std::fmt;

use crate::algorithms::sat::{Formula, backend};
use crate::computation::{Cancellation, Computation, Phase};
use rustsat::{
    solvers::{SolveIncremental, SolverResult},
    types::{Lit, TernaryVal},
};
use serde::Serialize;
use ts_rs::TS;

use crate::model::LabelId;
use crate::model::Problem;
use encoding::Choice;
use passive::{Emission, forbidden_combination};

pub use super::passive::PassiveMode;

/// A `LocalRule` specifies, for a node of a certain `degree` and a certain `input`, to give a specific `output`.
/// We use many of these rules to describe a zero round algorithm (one rule for each degree/input).
/// For problems with no input, `input` is `None`.
/// The input configuration is stored explicitly (not with multiplicities),
/// because we may need to map the same label to different labels.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct LocalRule {
    pub degree: u32,
    pub input: Option<Vec<LabelId>>,
    pub output: Vec<LabelId>,
}

/// A zero round algorithm is just a vector of local rules
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ZeroRoundResult {
    Solvable { strategy: Vec<LocalRule> },
    Unsolvable,
}

/// zero round solvability check could fail because of the sat solver running out of memory,
/// or the check being interrupted
#[derive(Debug)]
pub struct ZeroRoundError(anyhow::Error);

impl From<anyhow::Error> for ZeroRoundError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

impl From<crate::computation::Cancelled> for ZeroRoundError {
    fn from(error: crate::computation::Cancelled) -> Self {
        Self(error.into())
    }
}

impl fmt::Display for ZeroRoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zero-round search failed: {}", self.0)
    }
}

impl std::error::Error for ZeroRoundError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// In order to check if a problem is zero round solvable in the considered setting,
/// we can call this function. `General` accepts arbitrary passive constraints;
/// `Maximized` requires the caller to guarantee their maximization.
pub fn check(problem: &Problem, mode: PassiveMode) -> Result<ZeroRoundResult, ZeroRoundError> {
    let mut report = |_| {};
    check_with(
        problem,
        mode,
        &mut Computation::new(Cancellation::default(), &mut report),
    )
}

/// Run the same search with streamed progress and cooperative cancellation.
pub fn check_with(
    problem: &Problem,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    let solver = backend(&computation.cancellation);
    search(problem, mode, solver, computation)
}

/// This is the function that actually looks for a zero round algorithm.
/// We use a sat solver.
/// For passive degree 2, we directly ask for a solution.
/// In the other cases, we use counterexample refinement: this avoids having to encode
/// the whole passive constraint as a sat formula.
/// Note: the result is under the assumption that the input graph
/// has a valid solution for the input problem,
/// and if this is not doable, we could get unsat even if we would consider the problem trivial.
fn search<S: SolveIncremental>(
    problem: &Problem,
    mode: PassiveMode,
    solver: S,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    // in the Enumerating phase, we take the input problem
    // and we construct a `LocalRule` for each possible input the algorithm could see.
    // For each such input, the output is initialized as an empty vector.
    // We will use a sat solver to fill those vectors.
    computation.report(Phase::Enumerating, 0)?;
    let active = problem.output().active();
    let passive = problem.output().passive();
    let mut rules = prepare::prepare_local_rules(problem, &computation.cancellation)?;
    // now we start encoding "finding a good output for each input" as a sat instance
    let mut solver = Formula::new(solver, computation.cancellation.clone());
    computation.report(Phase::Encoding, 0)?;
    let mut positions = Vec::new();
    let mut sources: BTreeMap<(Option<LabelId>, LabelId), Vec<Lit>> = BTreeMap::new();
    let mut domains: BTreeMap<(u32, Option<LabelId>), Vec<LabelId>> = BTreeMap::new();
    // we must pick an output configuration for each possible input/degree
    for rule in &rules {
        computation.cancellation.check()?;
        let mut local = Vec::new();
        // so we must pick a label for each port
        for position in 0..rule.degree as usize {
            let input = rule.input.as_ref().map(|labels| labels[position]);
            // Many rules share a degree and input label. Collect their
            // possible outputs once to avoid repeated work
            let domain = domains
                .entry((rule.degree, input))
                .or_insert_with(|| prepare::output_domain(problem, rule.degree, input));
            // on each port, we must choose exactly one output label.
            // so we first create a vector of possible choices,
            // where a possible output gets a fresh literal
            let choices: Vec<_> = domain
                .iter()
                .copied()
                .map(|label| {
                    let literal = solver.new_lit();
                    sources.entry((input, label)).or_default().push(literal);
                    Choice { label, literal }
                })
                .collect();
            // and then we require, on the current port,
            // to have exactly one chosen mapping from the input label to an output label
            solver.exactly_one(
                &choices
                    .iter()
                    .map(|choice| choice.literal)
                    .collect::<Vec<_>>(),
            )?;
            local.push(choices);
        }
        // then, we require the output configuration to be valid
        encoding::encode_constraint(&mut solver, active, rule.degree, &local, &[])?;
        positions.push(local);
    }

    // We create variables that encode "we mapped this input to this output at least once"
    let emissions: Vec<_> = sources
        .into_iter()
        .map(|((input, output), sources)| -> anyhow::Result<_> {
            let literal = solver.new_lit();
            for &source in &sources {
                solver.add_clause(&[!source, literal])?;
            }
            let mut clause = vec![!literal];
            clause.extend(sources);
            solver.add_clause(&clause)?;
            Ok(Emission {
                input,
                output,
                source: literal,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    // we now extract the input condensed configurations from the passive side
    let mut passive_inputs = prepare::prepare_passive_inputs(problem, &computation.cancellation)?;
    // And we fully encode the requirements for the configurations of degree <= 2
    for input in passive_inputs.iter().filter(|input| input.degree() <= 2) {
        passive::visit_forbidden(
            passive,
            input,
            &emissions,
            &computation.cancellation,
            |sources| {
                solver.add_clause(
                    &sources
                        .into_iter()
                        .map(|literal| !literal)
                        .collect::<Vec<_>>(),
                )
            },
        )?;
    }
    // For the others, we use counterexample refinement
    passive_inputs.retain(|input| input.degree() > 2);
    let mut iterations = 0;
    loop {
        computation.report(Phase::Solving, iterations)?;
        match solver.solver.solve()? {
            SolverResult::Unsat => return Ok(ZeroRoundResult::Unsolvable),
            SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
            SolverResult::Sat => {}
        }
        iterations = iterations.saturating_add(1);
        computation.report(Phase::Verifying, iterations)?;
        let mut selected = Vec::new();
        for &emission in &emissions {
            computation.cancellation.check()?;
            if solver.solver.lit_val(emission.source)? == TernaryVal::True {
                selected.push(emission);
            }
        }
        // we found a valid assignment.
        // Now we check if it is actually correct.
        // If not, we add a clause to exclude the broken part of the current solution
        if let Some(sources) = forbidden_combination(
            passive,
            &passive_inputs,
            &selected,
            mode,
            &computation.cancellation,
        )? {
            solver.add_clause(
                &sources
                    .into_iter()
                    .map(|literal| !literal)
                    .collect::<Vec<_>>(),
            )?;
            continue;
        }
        for (rule, local) in rules.iter_mut().zip(&positions) {
            rule.output = local
                .iter()
                .map(|choices| -> anyhow::Result<_> {
                    for choice in choices {
                        if solver.solver.lit_val(choice.literal)? == TernaryVal::True {
                            return Ok(choice.label);
                        }
                    }
                    anyhow::bail!("SAT model has no output for an occurrence")
                })
                .collect::<anyhow::Result<_>>()?;
        }
        return Ok(ZeroRoundResult::Solvable { strategy: rules });
    }
}

#[cfg(test)]
mod tests;
