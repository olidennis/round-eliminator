use super::*;
use crate::{
    algorithms::event::EventHandler,
    group::{Group, GroupType},
    line::{Degree, Line},
    part::Part,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

mod annotations;
mod mapping;
mod parallel_targets;
mod portfolio;
mod re_target;
mod repair;
mod schedule;
mod synthesis;
use annotations::Input;

pub(super) const LIMIT: &str = "Reversible edge search budget reached";
const CANCELLED: &str = "Reversible edge search cancelled";
// Resource guards, not limits of the label representation or SAT solver.
const MAX_LABELS: usize = 64;
const MAX_EDGE_PAIRS: usize = MAX_LABELS * (MAX_LABELS + 1) / 2;
const CERTIFICATE_BYTES: usize = 2_000_000;
const REPORT_BYTES: usize = 16_000_000;

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else {
        "non-string panic payload; see the server terminal".into()
    }
}

fn certificate_cost(c: &Certificate) -> usize {
    let graph_cost = |g: &Subgraph| match g {
        Subgraph::All => 32,
        Subgraph::Pairs(p) => 64 + p.len() * 32,
    };
    let recipe_cost: usize = c
        .recipe
        .iter()
        .map(|s| match s {
            Step::Mis(g) | Step::Matching(g) | Step::GreedyColoring(g) | Step::RulingSet(g) => {
                graph_cost(g)
            }
            Step::PriorityMis { graph, order } => {
                graph_cost(graph) + order.iter().map(|r| 32 + r.len() * 16).sum::<usize>()
            }
            Step::RepairPairs(p) => 64 + p.len() * 32,
            _ => 32,
        })
        .sum();
    let target_cost = c.target.as_ref().map_or(0, |target| {
        serde_json::to_vec(target).map_or(CERTIFICATE_BYTES + 1, |bytes| bytes.len())
    });
    1024 + recipe_cost
        + target_cost
        + c.mapping
            .iter()
            .map(|r| {
                r.input
                    .iter()
                    .map(|s| s.len().saturating_mul(6) + 32)
                    .sum::<usize>()
                    + r.output.len() * 16
            })
            .sum::<usize>()
}

fn fits_report(report: &Report, c: &Certificate) -> bool {
    report
        .certificates
        .iter()
        .map(certificate_cost)
        .sum::<usize>()
        + certificate_cost(c)
        <= REPORT_BYTES
}

pub(super) struct Budget<'a> {
    options: &'a Options,
    deadline: Instant,
}
impl Budget<'_> {
    fn check(&self, eh: &EventHandler) -> Result<(), String> {
        if eh.is_cancelled() {
            return Err(CANCELLED.into());
        }
        if Instant::now() >= self.deadline {
            return Err(LIMIT.into());
        }
        Ok(())
    }
}

pub(super) fn pair(a: Label, b: Label) -> [Label; 2] {
    [a.min(b), a.max(b)]
}
pub(super) fn edge_line([a, b]: [Label; 2]) -> Line {
    let mut line = Line {
        parts: vec![
            Part {
                group: Group::from(vec![a]),
                gtype: GroupType::Many(1),
            },
            Part {
                group: Group::from(vec![b]),
                gtype: GroupType::Many(1),
            },
        ],
    };
    line.normalize();
    line
}

fn validate(p: &Problem, options: &Options) -> Result<(), String> {
    if !matches!(p.active.degree, Degree::Finite(1..=6)) || p.passive.degree != Degree::Finite(2) {
        return Err(
            "Reversible edge additions currently require node degree 1–6 and edge degree 2".into(),
        );
    }
    let label_count = p.labels().len();
    if label_count == 0 || label_count > MAX_LABELS || p.active.lines.is_empty() {
        return Err(format!(
            "Reversible edge additions require 1–{MAX_LABELS} labels and a nonempty node constraint"
        ));
    }
    if !(1..=86400).contains(&options.seconds)
        || !(1..=60000).contains(&options.attempt_ms)
        || !(1..=4096).contains(&options.max_candidates)
        || !(1..=32768).contains(&options.max_configurations)
        || !(1..=4096).contains(&options.max_states)
        || !(1..=1_000_000).contains(&options.max_variables)
        || options.threads > 32
    {
        return Err("Invalid reversible-edge search limits".into());
    }
    Ok(())
}

fn relaxation(p: &Problem, added: &[[Label; 2]]) -> Result<Problem, String> {
    let labels: BTreeSet<_> = p.labels().into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut passive = p.passive.clone();
    passive.is_maximized = false;
    if added.is_empty() {
        return Err("An edge-addition certificate must add at least one pair".into());
    }
    for &[a, b] in added {
        if !labels.contains(&a) || !labels.contains(&b) || a > b || !seen.insert([a, b]) {
            return Err("Invalid, reversed, or duplicate added edge pair".into());
        }
        let line = edge_line([a, b]);
        if p.passive.includes(&line) {
            return Err("An added edge pair is already allowed".into());
        }
        passive.lines.push(line);
    }
    Ok(p.replace_passive(passive))
}

fn recipes(q: &Problem, added: &[[Label; 2]]) -> Vec<Vec<Step>> {
    let same: Vec<_> = q
        .labels()
        .into_iter()
        .filter(|&a| q.passive.includes(&edge_line([a, a])))
        .map(|a| Step::Mis(Subgraph::Pairs(vec![[a, a]])))
        .collect();
    let mixed: Vec<_> = added
        .iter()
        .map(|&e| Step::Mis(Subgraph::Pairs(vec![e])))
        .collect();
    let mut recipes = vec![vec![], vec![Step::Coloring], vec![Step::Mis(Subgraph::All)]];
    recipes.extend(mixed.iter().cloned().map(|s| vec![s]));
    recipes.extend(same.iter().cloned().map(|s| vec![s]));
    recipes.push(same.clone());
    recipes.push(mixed.clone());
    recipes.push(vec![Step::Mis(Subgraph::Pairs(added.to_vec()))]);
    let mut combined = same;
    combined.extend(mixed);
    combined.dedup();
    recipes.push(combined);
    // One communication round can expose the neighbor's MIS/color status.
    let base = recipes.clone();
    for mut recipe in base {
        if recipe.len() <= 8 {
            recipe.push(Step::Exchange);
            recipes.push(recipe);
        }
    }
    let mut seen = BTreeSet::new();
    recipes.retain(|r| r.len() <= 16 && seen.insert(r.clone()));
    recipes
}

fn transformed(
    q: &Problem,
    recipe: &[Step],
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Input, String> {
    let mut input = Input::new(q, budget, eh)?;
    for (i, step) in recipe.iter().enumerate() {
        budget.check(eh)?;
        eh.notify(
            "Reversible edges: annotating MIS/coloring states",
            i + 1,
            recipe.len(),
        );
        input = input.step(step, i + 1, budget, eh)?;
    }
    Ok(input)
}

fn attempt(
    p: &Problem,
    q: &Problem,
    added: &[[Label; 2]],
    recipe: &[Step],
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<Certificate>, String> {
    attempt_target(p, q, added, recipe, None, budget, eh)
}

fn attempt_target(
    p: &Problem,
    q: &Problem,
    added: &[[Label; 2]],
    recipe: &[Step],
    re2: Option<&Re2Target>,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<Certificate>, String> {
    let destination = re2.map_or(p, |target| &target.second);
    if let [Step::FindMis(stages)] = recipe {
        let Some(graphs) = synthesis::mis_graphs(destination, q, *stages, budget, eh)? else {
            return Ok(None);
        };
        let recipe = graphs.into_iter().map(Step::Mis).collect::<Vec<_>>();
        return attempt_target(p, q, added, &recipe, re2, budget, eh);
    }
    let input = match transformed(q, recipe, budget, eh) {
        Err(e) if e == repair::NOT_REPAIRABLE => return Ok(None),
        result => result?,
    };
    let target = Input::new(destination, budget, eh)?;
    let Some(outputs) = mapping::find(&input, &target, budget, eh)? else {
        return Ok(None);
    };
    // The checker uses the constraints, not SAT's internal selector variables.
    mapping::verify(&input, &target, &outputs, budget, eh)?;
    let cost: usize = input
        .nodes
        .iter()
        .map(|r| {
            r.iter()
                .map(|&s| input.names[s].len() * 6 + 48)
                .sum::<usize>()
        })
        .sum();
    if cost + 1024 > CERTIFICATE_BYTES {
        return Err(LIMIT.into());
    }
    let certificate = Certificate {
        added: added.to_vec(),
        recipe: recipe.to_vec(),
        target: re2.cloned().map(Box::new),
        mapping: input
            .nodes
            .iter()
            .zip(outputs)
            .map(|(row, output)| MappingRow {
                input: row.iter().map(|&s| input.names[s].clone()).collect(),
                output,
            })
            .collect(),
    };
    if certificate_cost(&certificate) > CERTIFICATE_BYTES {
        return Err(LIMIT.into());
    }
    Ok(Some(certificate))
}

/// Stream cumulative reports: STOP can leave already verified results visible.
pub fn search(
    p: &Problem,
    options: &Options,
    eh: &mut EventHandler,
    publish: impl FnMut(&Report),
) -> Result<Report, String> {
    validate(p, options)?;
    parallel_targets::search(p, options, eh, publish)
}

fn search_branch(
    p: &Problem,
    options: &Options,
    re2: Option<&Re2Target>,
    started: Instant,
    deadline: Instant,
    eh: &mut EventHandler,
    mut publish: impl FnMut(&Report),
) -> Result<Report, String> {
    let mut report = Report {
        original: p.clone(),
        certificates: vec![],
        stats: Stats::default(),
        complete: false,
        message: "Searching; only verified additions are listed.".into(),
    };
    let labels = p.labels();
    let missing: Vec<_> = labels
        .iter()
        .enumerate()
        .flat_map(|(i, &a)| labels[i..].iter().map(move |&b| [a, b]))
        .filter(|&e| !p.passive.includes(&edge_line(e)))
        .collect();
    let candidates: Vec<_> = missing
        .iter()
        .take(options.max_candidates)
        .map(|&e| vec![e])
        .collect();
    let mut schedules = Vec::new();
    let planning = Budget { options, deadline };
    let mut budget_hit = candidates.len() < missing.len();
    for (i, added) in candidates.iter().enumerate() {
        eh.notify(
            "Reversible edges: building preprocessing portfolios",
            i + 1,
            candidates.len(),
        );
        let q = relaxation(p, added)?;
        match schedule::recipes(&q, added, &planning, eh) {
            Ok(recipes) => schedules.push(recipes),
            Err(e) if e == LIMIT => {
                budget_hit = true;
                schedules.push(vec![]);
            }
            Err(e) => return Err(e),
        }
    }
    let summary = portfolio::run(
        p,
        re2,
        &candidates,
        &schedules,
        options,
        deadline,
        eh,
        |_, c, stats| {
            if !fits_report(&report, &c) {
                return false;
            }
            report.stats.candidates = stats.touched.len();
            report.stats.mapping_attempts = stats.attempts;
            report.stats.bounded_attempts = stats.limited;
            report.stats.elapsed_ms = started.elapsed().as_millis() as u64;
            report.certificates.push(c);
            publish(&report);
            true
        },
    )?;
    budget_hit |= summary.incomplete;
    report.stats.candidates = summary.touched.len();
    report.stats.mapping_attempts = summary.attempts;
    report.stats.bounded_attempts = summary.limited;
    // Grow several deterministic chains, never assuming independent additions
    // can be combined. Every published union has its own reverse certificate.
    let mut singles: Vec<_> = report.certificates.iter().map(|c| c.added[0]).collect();
    singles.sort();
    let mut seen = BTreeSet::new();
    'joint: for first in 0..singles.len().min(4) {
        let mut added = vec![singles[first]];
        for offset in 1..singles.len() {
            let edge = singles[(first + offset) % singles.len()];
            let mut proposed = added.clone();
            proposed.push(edge);
            proposed.sort();
            if !seen.insert(proposed.clone()) {
                continue;
            }
            if Instant::now() >= deadline || report.stats.candidates >= options.max_candidates {
                budget_hit = true;
                break;
            }
            report.stats.candidates += 1;
            eh.notify(
                "Reversible edges: verifying a joint addition",
                proposed.len(),
                missing.len(),
            );
            let q = relaxation(p, &proposed)?;
            let recipes = match schedule::recipes(&q, &proposed, &planning, eh) {
                Ok(r) => r,
                Err(e) if e == LIMIT => {
                    budget_hit = true;
                    break 'joint;
                }
                Err(e) => return Err(e),
            };
            let before = report.stats.clone();
            let summary = portfolio::run(
                p,
                re2,
                &[proposed.clone()],
                &[recipes],
                options,
                deadline,
                eh,
                |_, c, stats| {
                    if !fits_report(&report, &c) {
                        return false;
                    }
                    added = proposed.clone();
                    report.stats.mapping_attempts = before.mapping_attempts + stats.attempts;
                    report.stats.bounded_attempts = before.bounded_attempts + stats.limited;
                    report.stats.elapsed_ms = started.elapsed().as_millis() as u64;
                    report.certificates.push(c);
                    publish(&report);
                    true
                },
            )?;
            report.stats.mapping_attempts = before.mapping_attempts + summary.attempts;
            report.stats.bounded_attempts = before.bounded_attempts + summary.limited;
            budget_hit |= summary.incomplete;
        }
    }
    report.stats.elapsed_ms = started.elapsed().as_millis() as u64;
    report.complete = !budget_hit && report.stats.bounded_attempts == 0;
    report.message = format!("{} verified additions/sets. {} Unlisted additions are not certified, NOT proved irreversible. Joint sets are searched heuristically, not exhaustively.",
        report.certificates.len(), if report.complete { "The scheduled search finished." } else { "Some searches reached their limits." });
    publish(&report);
    Ok(report)
}

/// Reconstruct and independently verify even certificates received from the GUI.
pub fn apply(
    p: &Problem,
    certificate: &Certificate,
    eh: &mut EventHandler,
) -> Result<Problem, String> {
    let options = Options {
        seconds: 60,
        attempt_ms: 60000,
        max_configurations: 10000,
        max_states: 4096,
        max_variables: 1_000_000,
        ..Default::default()
    };
    validate(p, &options)?;
    if certificate.recipe.len() > 16 || certificate.mapping.len() > options.max_configurations {
        return Err("Certificate exceeds verification limits".into());
    }
    if certificate_cost(certificate) > CERTIFICATE_BYTES {
        return Err("Certificate exceeds storage limits".into());
    }
    let labels: BTreeSet<_> = p.labels().into_iter().collect();
    for step in &certificate.recipe {
        let graph = match step {
            Step::Mis(g) | Step::Matching(g) | Step::GreedyColoring(g) | Step::RulingSet(g) => {
                Some(g)
            }
            Step::PriorityMis { graph, order } => {
                if order.is_empty()
                    || order.len() > 16
                    || order.iter().any(|r| {
                        r.len() != p.active.finite_degree()
                            || r.windows(2).any(|v| v[0] > v[1])
                            || r.iter().any(|l| !labels.contains(l))
                    })
                    || order.iter().collect::<BTreeSet<_>>().len() != order.len()
                {
                    return Err("Invalid MIS priority order in certificate".into());
                }
                Some(graph)
            }
            Step::RepairPairs(pairs) => {
                if pairs.is_empty()
                    || pairs.len() > MAX_EDGE_PAIRS
                    || pairs
                        .iter()
                        .any(|&[a, b]| a > b || !labels.contains(&a) || !labels.contains(&b))
                {
                    return Err("Invalid repair edge pairs in certificate".into());
                }
                None
            }
            _ => None,
        };
        if let Some(Subgraph::Pairs(pairs)) = graph {
            if pairs.len() > MAX_EDGE_PAIRS
                || pairs
                    .iter()
                    .any(|&[a, b]| a > b || !labels.contains(&a) || !labels.contains(&b))
            {
                return Err("Invalid preprocessing subgraph in certificate".into());
            }
        }
    }
    let q = relaxation(p, &certificate.added)?;
    let budget = Budget {
        options: &options,
        deadline: Instant::now() + Duration::from_secs(options.seconds),
    };
    let destination = if let Some(target) = &certificate.target {
        re_target::verify(p, target, &budget, eh)?;
        &target.second
    } else {
        p
    };
    let input = transformed(&q, &certificate.recipe, &budget, eh)?;
    if input.nodes.len() != certificate.mapping.len() {
        return Err("Certificate has missing or extra node contexts".into());
    }
    for (row, saved) in input.nodes.iter().zip(&certificate.mapping) {
        if row.iter().map(|&s| &input.names[s]).ne(saved.input.iter()) {
            return Err("Certificate annotation/context mismatch".into());
        }
    }
    let outputs = certificate
        .mapping
        .iter()
        .map(|r| r.output.clone())
        .collect::<Vec<_>>();
    mapping::verify(
        &input,
        &Input::new(destination, &budget, eh)?,
        &outputs,
        &budget,
        eh,
    )?;
    // Do not invoke fix_problem: it can also change the node constraint/labels.
    Ok(q)
}

#[cfg(test)]
mod tests;
