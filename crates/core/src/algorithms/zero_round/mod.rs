//! Exact active-side, deterministic zero-round solvability in the port-numbering
//! model. A node knows its degree, incident inputs, and local port order.
//! Rules may distinguish occurrences of the same input label. Passive validity
//! must hold for every compatible combination of outputs from active nodes.

mod encoding;
mod prepare;
mod validity;
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

use crate::labels::{LabelId, LabelPair};
use crate::problem::Problem;
use encoding::Choice;
use validity::Validity;
use verify::{Emission, forbidden_combination};

/// Input occurrences are sorted by label ID, breaking ties by local port order.
/// Outputs use the same occurrence order. For plain problems, input is absent
/// and outputs are in local port order. IDs refer to the original label tables.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct LocalRule {
    pub degree: u32,
    pub input: Option<Vec<LabelId>>,
    pub output: Vec<LabelId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ZeroRoundResult {
    Solvable { strategy: Vec<LocalRule> },
    Unsolvable,
}

/// An execution failure is distinct from a proof of unsolvability.
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

/// Find a local strategy or prove that none exists. Degree-two passive checks
/// are encoded eagerly. Higher degrees use exact counterexample refinement;
/// neither case requires maximization or expansion of output configurations.
/// As usual for this local criterion, the input promise is used locally; this
/// does not decide whether the promise has any global graph realizations.
pub fn check(problem: &Problem) -> Result<ZeroRoundResult, ZeroRoundError> {
    let mut report = |_| {};
    check_with(
        problem,
        &mut Computation::new(Cancellation::default(), &mut report),
    )
}

/// Execute with progress reporting and cooperative cancellation.
pub fn check_with(
    problem: &Problem,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    let solver = backend(&computation.cancellation);
    search(problem, solver, computation)
}

// A different RustSAT backend can run this same encoding and refinement loop.
fn search<S: SolveIncremental>(
    problem: &Problem,
    solver: S,
    computation: &mut Computation<'_>,
) -> Result<ZeroRoundResult, ZeroRoundError> {
    computation.report(Phase::Enumerating, 0)?;
    let (active, passive) = Validity::for_problem(problem);
    let mut rules = prepare::contexts(problem, &computation.cancellation)?;
    let mut solver = Formula::new(solver, computation.cancellation.clone());
    computation.report(Phase::Encoding, 0)?;
    let mut positions = Vec::new();
    let mut sources: BTreeMap<(Option<LabelId>, LabelId), Vec<Lit>> = BTreeMap::new();
    let mut domains: BTreeMap<(u32, Option<LabelId>), Vec<LabelId>> = BTreeMap::new();
    for rule in &rules {
        computation.cancellation.check()?;
        let mut local = Vec::new();
        for position in 0..rule.degree as usize {
            let input = rule.input.as_ref().map(|labels| labels[position]);
            // Many contexts share a degree and input label. Collect their
            // possible outputs once; occurrence choices still remain separate.
            let domain = domains.entry((rule.degree, input)).or_insert_with(|| {
                active
                    .domain(rule.degree, input)
                    .into_iter()
                    .filter(|&output| validity::mapped_output_allowed(problem, input, output))
                    .collect()
            });
            let choices: Vec<_> = domain
                .iter()
                .copied()
                .map(|label| {
                    let literal = solver.new_lit();
                    sources.entry((input, label)).or_default().push(literal);
                    Choice { label, literal }
                })
                .collect();
            solver.exactly_one(
                &choices
                    .iter()
                    .map(|choice| choice.literal)
                    .collect::<Vec<_>>(),
            )?;
            local.push(choices);
        }
        match active {
            Validity::Labels(constraint) => encoding::encode_active(
                &mut solver,
                constraint,
                rule.degree,
                &local,
                |_, output| output,
            )?,
            Validity::Pairs(constraint) => encoding::encode_active(
                &mut solver,
                constraint,
                rule.degree,
                &local,
                |position, output| LabelPair {
                    input: rule.input.as_ref().unwrap()[position],
                    output,
                },
            )?,
        }
        positions.push(local);
    }

    // An emission means that at least one active occurrence can produce this
    // output on this input. Equivalence makes passive clauses independent of
    // which context or port is responsible, and strengthens each refinement.
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
    let mut passive_inputs = prepare::passive_inputs(problem, &computation.cancellation)?;
    // Small passive degrees are cheap enough to encode in full before search.
    for input in passive_inputs.iter().filter(|input| input.degree() <= 2) {
        verify::visit_forbidden(
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
    // Eager clauses already prove these degrees valid in every SAT model.
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
        if let Some(sources) = forbidden_combination(
            passive,
            &passive_inputs,
            &selected,
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
