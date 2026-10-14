//! LLM: Exact active-side, deterministic zero-round solvability in the port-numbering
//! LLM: model. A node knows its degree, incident inputs, and local port order.
//! LLM: Plain problems of passive degree at most two use compatible-support search;
//! LLM: other problems use SAT with exact passive verification.
//! LLM: Rules may distinguish occurrences of the same input label. Passive validity
//! LLM: must hold for every compatible combination of outputs from active nodes.

use super::constraint_encoding as encoding;
use super::passive;
mod degree_two;
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
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::algorithms::maximize::MaximizationResult;
use crate::model::LabelId;
use crate::model::Problem;
use encoding::Choice;
use passive::{Emission, forbidden_with_maximum};

pub use super::passive::PassiveMode;

/// LLM: One zero-round action for an active node of a given degree and full local input
/// LLM: multiset. Its output vector assigns a label to each occurrence/port; repeated equal
/// LLM: inputs may receive different outputs. `None` is trivial input.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct LocalRule {
    pub degree: u32,
    pub input: Option<Vec<LabelId>>,
    pub output: Vec<LabelId>,
}

/// LLM: Either a complete deterministic active-side zero-round strategy, expressed as local
/// LLM: rules, or a proof that none exists under the problem's input promise and graph class.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ZeroRoundResult {
    Solvable { strategy: Vec<LocalRule> },
    Unsolvable,
}

/// LLM: A failed zero-round computation wrapping its original cause, such as cooperative
/// LLM: cancellation or a solver error. It is neither a negative solvability decision nor a
/// LLM: partial strategy.
#[derive(Debug)]
pub struct ZeroRoundError(anyhow::Error);

impl From<anyhow::Error> for ZeroRoundError {
    /// LLM: Convert the source value into ZeroRoundError, preserving its error explanation and
    /// LLM: any applicable location or cause. Used by the standard From conversion path.
    fn from(error: anyhow::Error) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self(error)
    }
}

impl From<crate::computation::Cancelled> for ZeroRoundError {
    /// LLM: Convert the source value into ZeroRoundError, preserving its error explanation and
    /// LLM: any applicable location or cause. Used by the standard From conversion path.
    fn from(error: crate::computation::Cancelled) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self(error.into())
    }
}

impl fmt::Display for ZeroRoundError {
    /// LLM: Write the stored explanation through Rust's Display formatter. Used when
    /// LLM: presenting or converting this error to a human-readable message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // LLM: Update the current working state before continuing this operation.
        write!(f, "Zero-round search failed: {}", self.0)
    }
}

impl std::error::Error for ZeroRoundError {
    /// LLM: Expose the wrapped error as the original failure cause. Used by Rust error
    /// LLM: chaining, including cancellation classification.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // LLM: Return the result of this completed search or conversion to its caller.
        Some(self.0.as_ref())
    }
}

/// LLM: Decide deterministic active-side zero-round solvability, with the selected passive
/// LLM: validity guarantee. Used by the API and coloring preflight; cancellation and backend
/// LLM: errors are distinct from a negative decision.
pub fn check(problem: &Problem, mode: PassiveMode) -> Result<ZeroRoundResult, ZeroRoundError> {
    // LLM: Connect phase/work updates to the caller while retaining cooperative cancellation.
    let mut report = |_| {};
    check_with(
        problem,
        mode,
        &mut Computation::new(Cancellation::default(), &mut report),
    )
}

/// LLM: Decide deterministic active-side zero-round solvability, with the selected passive
/// LLM: validity guarantee. Used by the API and coloring preflight; cancellation and backend
/// LLM: errors are distinct from a negative decision.
pub fn check_with(
    problem: &Problem,
    mode: PassiveMode,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    check_with_maximum(problem, mode, None, computation)
}

/// LLM: Decide zero-round solvability with a supplied completed maximum belonging to this
/// LLM: exact immutable snapshot. Used by the API and coloring preflight; no implicit
/// LLM: maximization occurs. Plain passive degrees at most two use support search, while other
/// LLM: cases retain SAT.
pub fn check_with_maximum(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    // LLM: Dispatch eligible plain problems to compatible-support search and retain SAT for
    // LLM: all other input/degree cases.
    if degree_two::applicable(problem) {
        return degree_two::check(problem, computation).map_err(ZeroRoundError::from);
    }
    let solver = backend(&computation.cancellation);
    search(problem, mode, maximum, solver, computation)
}

/// LLM: Solve the occurrence-choice SAT encoding and refine forbidden passive combinations
/// LLM: until a complete strategy or UNSAT. Used by the general zero-round path; low passive
/// LLM: degrees can be encoded eagerly.
fn search<S: SolveIncremental>(
    problem: &Problem,
    mode: PassiveMode,
    maximum: Option<&MaximizationResult>,
    solver: S,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    // LLM: in the Enumerating phase, we take the input problem
    // LLM: and we construct a `LocalRule` for each possible input the algorithm could see.
    // LLM: For each such input, the output is initialized as an empty vector.
    // LLM: We will use a sat solver to fill those vectors.

    computation.report(Phase::Enumerating, 0)?;
    let active = problem.output().active();
    let passive = problem.output().passive();
    let mut rules = prepare::prepare_local_rules(problem, &computation.cancellation)?;
    // LLM: now we start encoding "finding a good output for each input" as a sat instance

    let mut solver = Formula::new(solver, computation.cancellation.clone());
    computation.report(Phase::Encoding, 0)?;
    let mut positions = Vec::new();
    let mut sources: BTreeMap<(Option<LabelId>, LabelId), Vec<Lit>> = BTreeMap::new();
    let mut domains: BTreeMap<(u32, Option<LabelId>), Vec<LabelId>> = BTreeMap::new();
    // LLM: we must pick an output configuration for each possible input/degree

    for rule in &rules {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        computation.cancellation.check()?;
        let mut local = Vec::new();
        // LLM: so we must pick a label for each port

        for position in 0..rule.degree as usize {
            // LLM: Retain or prepare the original promised input semantics for this stage.
            let input = rule.input.as_ref().map(|labels| labels[position]);
            // LLM: Many rules share a degree and input label. Collect their
            // LLM: possible outputs once to avoid repeated work

            let domain = domains
                .entry((rule.degree, input))
                .or_insert_with(|| prepare::output_domain(problem, rule.degree, input));
            // LLM: on each port, we must choose exactly one output label.
            // LLM: so we first create a vector of possible choices,
            // LLM: where a possible output gets a fresh literal

            let choices: Vec<_> = domain
                .iter()
                .copied()
                .map(|label| {
                    let literal = solver.new_lit();
                    sources.entry((input, label)).or_default().push(literal);
                    Choice { label, literal }
                })
                .collect();
            // LLM: and then we require, on the current port,
            // LLM: to have exactly one chosen mapping from the input label to an output label

            solver.exactly_one(
                &choices
                    .iter()
                    .map(|choice| choice.literal)
                    .collect::<Vec<_>>(),
            )?;
            local.push(choices);
        }
        // LLM: then, we require the output configuration to be valid

        encoding::encode_constraint(&mut solver, active, rule.degree, &local, &[])?;
        positions.push(local);
    }

    // LLM: We create variables that encode "we mapped this input to this output at least once"

    let emissions: Vec<_> = sources
        .into_iter()
        .map(|((input, output), sources)| -> anyhow::Result<_> {
            // LLM: Allocate the output-choice or aggregated-emission literal and connect it to
            // LLM: the appropriate source choices.
            let literal = solver.new_lit();
            for &source in &sources {
                solver.add_clause(&[!source, literal])?;
            }
            let mut clause = vec![!literal];
            clause.extend(sources);
            solver.add_clause(&clause)?;
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(Emission {
                input,
                output,
                source: literal,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    // LLM: we now extract the input condensed configurations from the passive side

    let mut passive_inputs = prepare::prepare_passive_inputs(problem, &computation.cancellation)?;
    // LLM: And we fully encode the requirements for the configurations of degree <= 2

    for input in passive_inputs.iter().filter(|input| input.degree() <= 2) {
        // LLM: Encode every forbidden low-degree source combination needed by future SAT
        // LLM: candidates.
        passive::visit_forbidden(
            passive,
            input,
            &emissions,
            &computation.cancellation,
            |sources| {
                // LLM: Add the necessary SAT restriction; a refinement clause forbids the
                // LLM: selected violating combination.
                solver.add_clause(
                    &sources
                        .into_iter()
                        .map(|literal| !literal)
                        .collect::<Vec<_>>(),
                )
            },
        )?;
    }
    // LLM: For the others, we use counterexample refinement

    passive_inputs.retain(|input| input.degree() > 2);
    let mut iterations = 0;
    // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
    loop {
        // LLM: Update the current working state before continuing this operation.
        computation.report(Phase::Solving, iterations)?;
        match solver.solver.solve()? {
            SolverResult::Unsat => return Ok(ZeroRoundResult::Unsolvable),
            SolverResult::Interrupted => return Err(crate::computation::Cancelled.into()),
            SolverResult::Sat => {}
        }
        iterations = iterations.saturating_add(1);
        computation.report(Phase::Verifying, iterations)?;
        // LLM: Read one chosen source literal per port before reconstructing the whole local
        // LLM: rule.
        let mut selected = Vec::new();
        for &emission in &emissions {
            computation.cancellation.check()?;
            if solver.solver.lit_val(emission.source)? == TernaryVal::True {
                selected.push(emission);
            }
        }
        // LLM: we found a valid assignment.
        // LLM: Now we check if it is actually correct.
        // LLM: If not, we add a clause to exclude the broken part of the current solution

        if let Some(sources) = forbidden_with_maximum(
            passive,
            &passive_inputs,
            &selected,
            mode,
            maximum,
            |input| input,
            &computation.cancellation,
        )? {
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            solver.add_clause(
                &sources
                    .into_iter()
                    .map(|literal| !literal)
                    .collect::<Vec<_>>(),
            )?;
            continue;
        }
        // LLM: Process the relevant label choices while preserving their table identities.
        for (rule, local) in rules.iter_mut().zip(&positions) {
            // LLM: Fill the rule's output vector from true per-port choices, preserving local
            // LLM: port correspondence.
            rule.output = local
                .iter()
                .map(|choices| -> anyhow::Result<_> {
                    // LLM: Process the relevant label choices while preserving their table
                    // LLM: identities.
                    for choice in choices {
                        if solver.solver.lit_val(choice.literal)? == TernaryVal::True {
                            return Ok(choice.label);
                        }
                    }
                    anyhow::bail!("SAT model has no output for an occurrence")
                })
                .collect::<anyhow::Result<_>>()?;
        }
        // LLM: Return the complete strategy certificate after all required contexts have been
        // LLM: covered.
        return Ok(ZeroRoundResult::Solvable { strategy: rules });
    }
}

#[cfg(test)]
mod tests;
