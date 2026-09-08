//! Standalone, solver-independent certificate CNF export and model validation.
//! Known derivations are used ONLY to check satisfiability of the exported
//! formula, never as constraints or additional leaves of the benchmark.

use super::*;
use std::io::Write;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod restrictions;

/// Optional benchmark restrictions. These do not change the GUI Loop search.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct InstanceOptions {
    /// Maximum parent-chain length, with original configurations at depth 0.
    pub max_depth: Option<usize>,
    /// Canonical meet-coordinate order and identical input occurrences.
    pub symmetry: bool,
}

#[derive(serde::Serialize)]
pub struct InstanceStats {
    pub steps: usize,
    pub variables: u32,
    pub clauses: usize,
    pub original_configurations: usize,
    pub known_derivation_steps: Option<usize>,
    pub known_model_verified: bool,
    pub restrictions: InstanceOptions,
}

fn prepare(problem: &Problem) -> Result<Problem> {
    validate(problem, &Default::default())?;
    let mut problem = problem.clone();
    if problem.diagram_indirect.is_none() {
        problem.compute_diagram(&mut EventHandler::null());
    }
    Ok(problem)
}

fn build<'a>(
    problem: &Problem,
    steps: usize,
    control: &'a SearchControl,
) -> Result<ProofEncoding<'a>> {
    if steps == 0 {
        return Err("Standalone CNF export requires a positive step bound".into());
    }
    let mut encoding = ProofEncoding::new_recorded(problem, &input_terms(problem), control, true)?;
    let mut goal = encoding.circuit.truth;
    for _ in 0..steps {
        goal = encoding.step()?;
    }
    encoding.circuit.clause([goal])?;
    Ok(encoding)
}

fn build_with_options<'a>(
    problem: &Problem,
    steps: usize,
    control: &'a SearchControl,
    options: &InstanceOptions,
) -> Result<ProofEncoding<'a>> {
    let mut encoding = build(problem, steps, control)?;
    restrictions::apply(&mut encoding, options)?;
    Ok(encoding)
}

fn check_clauses(encoding: &ProofEncoding, model: &Assignment) -> Result<()> {
    for (i, clause) in encoding
        .circuit
        .recorded
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
    {
        if !clause
            .iter()
            .any(|&lit| model.lit_value(lit) == TernaryVal::True)
        {
            return Err(format!("Model does not satisfy CNF clause {}", i + 1).into());
        }
    }
    Ok(())
}

pub fn write_model(model: &Assignment, variables: u32, writer: &mut dyn Write) -> Result<()> {
    writeln!(writer, "s SATISFIABLE")?;
    if variables == 0 {
        writeln!(writer, "v 0")?;
    }
    for start in (0..variables).step_by(16) {
        write!(writer, "v")?;
        for variable in start..variables.min(start + 16) {
            let positive = Lit::new(variable, false);
            let lit = if model.lit_value(positive) == TernaryVal::True {
                positive
            } else {
                !positive
            };
            write!(writer, " {}", lit.to_ipasir())?;
        }
        // A model is one zero-terminated sequence, even across several v
        // lines. External readers such as CaDiCaL stop at the first zero.
        if start + 16 >= variables {
            write!(writer, " 0")?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

fn canonical(term: &Term) -> Term {
    match term {
        Term::Terminal(_) => term.clone(),
        Term::Expr(a, b, op) => {
            let (mut a, mut b) = (canonical(a), canonical(b));
            if a == b {
                return a;
            }
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            Term::Expr(Box::new(a), Box::new(b), *op)
        }
    }
}

fn tuple_key(terms: &[Term]) -> Vec<Term> {
    let mut key: Vec<_> = terms.iter().map(canonical).collect();
    key.sort();
    key
}

struct KnownStep {
    parents: [usize; 2],
    permutations: [Vec<usize>; 2],
}

struct KnownPlan {
    values: Vec<Vec<Term>>,
    ids: HashMap<Vec<Term>, usize>,
    steps: Vec<KnownStep>,
    symmetry: bool,
}

impl KnownPlan {
    fn new(inputs: &[Vec<Term>]) -> Self {
        Self {
            values: inputs.to_vec(),
            ids: inputs
                .iter()
                .enumerate()
                .map(|(i, t)| (tuple_key(t), i))
                .collect(),
            steps: Vec::new(),
            symmetry: false,
        }
    }

    fn derive(&mut self, terms: &[Term]) -> Result<usize> {
        let key = tuple_key(terms);
        if let Some(&id) = self.ids.get(&key) {
            return Ok(id);
        }
        let pivots: Vec<_> = terms
            .iter()
            .enumerate()
            .filter_map(|(i, t)| matches!(t, Term::Expr(_, _, Operation::Union)).then_some(i))
            .collect();
        if pivots.len() != 1 || terms.iter().any(|t| matches!(t, Term::Terminal(_))) {
            return Err("Known certificate is not a whole-configuration active derivation".into());
        }
        let mut order: Vec<_> = (0..terms.len()).collect();
        order.swap(0, pivots[0]);
        let mut desired = [Vec::new(), Vec::new()];
        for &i in &order {
            if let Term::Expr(a, b, _) = &terms[i] {
                desired[0].push(a.as_ref().clone());
                desired[1].push(b.as_ref().clone());
            }
        }
        let mut parents = [self.derive(&desired[0])?, self.derive(&desired[1])?];
        // Match the unrestricted encoding's commutative-parent symmetry break.
        if parents[0] > parents[1] {
            parents.swap(0, 1);
            desired.swap(0, 1);
        }
        let mut permutations = [Vec::new(), Vec::new()];
        for side in 0..2 {
            let source = &self.values[parents[side]];
            let mut used = vec![false; source.len()];
            for term in &desired[side] {
                let target = canonical(term);
                let pos = source
                    .iter()
                    .enumerate()
                    .position(|(i, t)| !used[i] && canonical(t) == target)
                    .ok_or("Cannot match known derivation occurrences")?;
                used[pos] = true;
                permutations[side].push(pos);
            }
        }
        if self.symmetry {
            // Output meet coordinates can be jointly reordered. Adjust every
            // later use via self.values, rather than constraining the known
            // trace's arbitrary old coordinate numbering.
            let mut rows: Vec<_> = (0..terms.len()).collect();
            rows[1..].sort_by_key(|&row| permutations[0][row]);
            order = rows.iter().map(|&row| order[row]).collect();
            permutations[0] = rows.iter().map(|&row| permutations[0][row]).collect();
            let source = &self.values[parents[1]];
            let mut used = vec![false; source.len()];
            permutations[1].clear();
            // Re-canonicalize identical occurrences in the right parent
            // after reordering rows. The sorted left parent is already canonical.
            for row in rows {
                let target = canonical(&desired[1][row]);
                let pos = source
                    .iter()
                    .enumerate()
                    .position(|(i, t)| !used[i] && canonical(t) == target)
                    .ok_or("Cannot normalize known right occurrences")?;
                used[pos] = true;
                permutations[1].push(pos);
            }
        }
        let id = self.values.len();
        self.values
            .push(order.iter().map(|&i| canonical(&terms[i])).collect());
        self.ids.insert(key, id);
        self.steps.push(KnownStep {
            parents,
            permutations,
        });
        Ok(id)
    }
}

fn parse_known(problem: &Problem, text: &str) -> Result<Vec<Term>> {
    fn parse<'a>(input: &mut &'a str, labels: &[(Label, String)]) -> Result<Term> {
        *input = input.trim_start();
        if input.starts_with('[') {
            *input = &input[1..];
            let a = parse(input, labels)?;
            *input = input.trim_start();
            let op = if let Some(rest) = input.strip_prefix('→') {
                *input = rest;
                Operation::Union
            } else if let Some(rest) = input.strip_prefix('←') {
                *input = rest;
                Operation::Intersection
            } else {
                return Err("Missing known-expression operator".into());
            };
            let b = parse(input, labels)?;
            *input = input
                .trim_start()
                .strip_prefix(']')
                .ok_or("Missing known-expression bracket")?;
            Ok(Term::Expr(Box::new(a), Box::new(b), op))
        } else {
            for (label, name) in labels {
                if let Some(rest) = input.strip_prefix(name) {
                    *input = rest;
                    return Ok(Term::Terminal(*label));
                }
            }
            Err("Unknown label in known certificate".into())
        }
    }
    let mut labels = problem.mapping_label_text.clone();
    labels.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));
    let original = text
        .split_once("Original expressions:")
        .ok_or("Missing Original expressions section")?
        .1;
    let mut terms = Vec::new();
    for line in original.lines().filter(|l| !l.trim().is_empty()) {
        let mut remaining = line;
        terms.push(parse(&mut remaining, &labels)?);
        if !remaining.trim().is_empty() {
            return Err("Trailing known-expression text".into());
        }
    }
    if terms.len() != problem.active.finite_degree() {
        return Err("Wrong known-certificate degree".into());
    }
    Ok(terms)
}

/// Parse and verify an externally supplied whole-configuration certificate.
/// Used by the native algorithm extractor; does not search for a certificate.
pub(crate) fn validated_certificate_terms(problem: &Problem, text: &str) -> Result<Vec<Term>> {
    let problem = prepare(problem)?;
    let terms = parse_known(&problem, text)?;
    KnownPlan::new(&input_terms(&problem)).derive(&terms)?;
    if NonexistenceOracle::new(&problem).check(&terms).is_none() {
        return Err("Expressions are not a universal nonexistence certificate".into());
    }
    Ok(terms)
}

/// Export the exact, unrestricted current proof encoding at one fixed bound.
/// Optional known terms are used as SAT ASSUMPTIONS in a separate validation
/// solve. Neither those assumptions nor learned clauses enter the DIMACS file.
pub fn export(
    problem: &Problem,
    steps: usize,
    cnf: &mut dyn Write,
    known: Option<(&str, &mut dyn Write)>,
) -> Result<InstanceStats> {
    export_with_options(problem, steps, cnf, known, &InstanceOptions::default())
}

pub fn export_with_options(
    problem: &Problem,
    steps: usize,
    cnf: &mut dyn Write,
    known: Option<(&str, &mut dyn Write)>,
    options: &InstanceOptions,
) -> Result<InstanceStats> {
    let problem = prepare(problem)?;
    let control = SearchControl::default();
    let mut encoding = build_with_options(&problem, steps, &control, options)?;
    let inputs = input_terms(&problem);
    let mut known_steps = None;
    if let Some((text, writer)) = known {
        let terms = parse_known(&problem, text)?;
        let mut plan = KnownPlan::new(&inputs);
        plan.symmetry = options.symmetry;
        let mut root = plan.derive(&terms)?;
        known_steps = Some(plan.steps.len());
        if plan.steps.len() > steps || (root + 1 != plan.values.len() && plan.steps.len() == steps)
        {
            return Err(format!("The known trace does not fit {steps} steps").into());
        }
        while plan.steps.len() < steps {
            let permutation: Vec<_> = (0..terms.len()).collect();
            plan.steps.push(KnownStep {
                parents: [root, root],
                permutations: [permutation.clone(), permutation],
            });
            plan.values.push(plan.values[root].clone());
            root = plan.values.len() - 1;
        }
        let mut assumptions = Vec::new();
        for (i, step) in plan.steps.iter().enumerate() {
            let Source::Combine(parents) = &encoding.tuples[inputs.len() + i].source else {
                unreachable!()
            };
            for side in 0..2 {
                assumptions.push(parents[side].tuple[step.parents[side]]);
                for (row, &column) in step.permutations[side].iter().enumerate() {
                    assumptions.push(parents[side].permutation[row][column]);
                }
            }
        }
        if control.solve(1, &mut encoding.circuit.solver, Some(&assumptions))? != SolverResult::Sat
        {
            return Err("Known derivation does not satisfy the actual SAT encoding".into());
        }
        let model = encoding.circuit.solver.full_solution()?;
        check_clauses(&encoding, &model)?;
        restrictions::check_depth(&encoding, &model, options)?;
        let replay = encoding.replay(&model, encoding.tuples.len() - 1)?;
        if NonexistenceOracle::new(&problem).check(&replay).is_none() {
            return Err("Known model failed independent certificate verification".into());
        }
        write_model(&model, encoding.circuit.next_var, writer)?;
    }
    let clauses = encoding.circuit.recorded.as_ref().unwrap();
    let qualifier = if options.max_depth.is_some() || options.symmetry {
        "Restricted"
    } else {
        "Unrestricted"
    };
    writeln!(cnf, "c {qualifier} active-derivation certificate search: {steps} steps, original leaves only")?;
    if options.max_depth.is_some() || options.symmetry {
        writeln!(
            cnf,
            "c Benchmark restrictions: {}",
            serde_json::to_string(options)?
        )?;
    }
    writeln!(cnf, "p cnf {} {}", encoding.circuit.next_var, clauses.len())?;
    for clause in clauses {
        for lit in clause {
            write!(cnf, "{} ", lit.to_ipasir())?;
        }
        writeln!(cnf, "0")?;
    }
    Ok(InstanceStats {
        steps,
        variables: encoding.circuit.next_var,
        clauses: clauses.len(),
        original_configurations: inputs.len(),
        known_derivation_steps: known_steps,
        known_model_verified: known_steps.is_some(),
        restrictions: options.clone(),
    })
}

/// Check a DIMACS solver model against every clause and then replay and verify
/// its active derivation using the existing nonexistence oracle. No SAT solve.
pub fn verify(problem: &Problem, steps: usize, text: &str) -> Result<String> {
    verify_with_options(problem, steps, text, &InstanceOptions::default())
}

pub fn verify_with_options(
    problem: &Problem,
    steps: usize,
    text: &str,
    options: &InstanceOptions,
) -> Result<String> {
    let problem = prepare(problem)?;
    let control = SearchControl::default();
    let encoding = build_with_options(&problem, steps, &control, options)?;
    let mut model = Assignment::default();
    let mut sat = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "s SATISFIABLE" || line == "SAT" {
            sat = true;
            continue;
        }
        let data = if let Some(rest) = line.strip_prefix("v ") {
            rest
        } else if sat
            && line
                .chars()
                .next()
                .is_some_and(|c| c == '-' || c.is_ascii_digit())
        {
            line
        } else {
            continue;
        };
        for word in data.split_whitespace() {
            let number: i32 = word.parse()?;
            if number == 0 {
                continue;
            }
            let lit = Lit::from_ipasir(number)?;
            if lit.var().idx() >= encoding.circuit.next_var as usize {
                return Err("Model variable out of range".into());
            }
            if model.lit_value(lit) == TernaryVal::False {
                return Err("Conflicting model literals".into());
            }
            model.assign_lit(lit);
        }
    }
    if !sat {
        return Err("No SAT result in solver output".into());
    }
    check_clauses(&encoding, &model)?;
    restrictions::check_depth(&encoding, &model, options)?;
    let terms = encoding.replay(&model, encoding.tuples.len() - 1)?;
    NonexistenceOracle::new(&problem)
        .check(&terms)
        .ok_or_else(|| "Model failed independent certificate verification".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hard_certificate_fits_twenty_one_original_leaf_steps() {
        let original = prepare(
            &Problem::from_string(include_str!(
                "../../../../examples/fixpoint_sat/hard_nonexistence.txt"
            ))
            .unwrap(),
        )
        .unwrap();
        let inputs = input_terms(&original);
        let terms = parse_known(&original, include_str!("known_certificate.txt")).unwrap();
        let mut plan = KnownPlan::new(&inputs);
        let root = plan.derive(&terms).unwrap();
        assert_eq!(inputs.len(), 5);
        assert_eq!(plan.steps.len(), 21);
        assert_eq!(root, inputs.len() + 20);
        let mut depths = vec![0; inputs.len()];
        for (i, step) in plan.steps.iter().enumerate() {
            assert!(step.parents[0] <= step.parents[1]);
            assert!(step.parents[1] < inputs.len() + i);
            depths.push(1 + depths[step.parents[0]].max(depths[step.parents[1]]));
            for permutation in &step.permutations {
                let mut sorted = permutation.clone();
                sorted.sort();
                assert_eq!(sorted, (0..terms.len()).collect::<Vec<_>>());
            }
        }
        assert_eq!(depths[root], 8);
    }

    #[test]
    fn known_assumptions_never_change_exported_cnf_and_models_are_checked() {
        let original = Problem::from_string("A A\n\nA A").unwrap();
        let known = "Original expressions:\r\n[A→A]\r\n[A←A]\r\n";
        let (mut cnf, mut hinted_cnf, mut model) = (Vec::new(), Vec::new(), Vec::new());
        let stats = export(&original, 2, &mut cnf, None).unwrap();
        assert!(!stats.known_model_verified);
        let checked = export(&original, 2, &mut hinted_cnf, Some((known, &mut model))).unwrap();
        assert!(checked.known_model_verified);
        assert_eq!(checked.known_derivation_steps, Some(0));
        assert_eq!(cnf, hinted_cnf);
        let model = String::from_utf8(model).unwrap();
        assert_eq!(model.split_whitespace().filter(|s| *s == "0").count(), 1);
        assert!(verify(&original, 2, &model)
            .unwrap()
            .contains("No fixed point can be found"));
        assert!(verify(&original, 2, "s SATISFIABLE\nv -1 0\n").is_err());
        assert!(verify(&original, 2, &format!("{model}v -1 0\n")).is_err());
        assert!(verify(&original, 2, &model.replace("s SATISFIABLE", "s UNKNOWN")).is_err());
    }

    #[test]
    fn bounded_unsat_is_exported_without_being_called_a_certificate() {
        let original = prepare(&Problem::from_string("A A\nB B\n\nA B").unwrap()).unwrap();
        let control = SearchControl::default();
        let mut encoding = build(&original, 2, &control).unwrap();
        assert_eq!(
            encoding.circuit.solver.solve().unwrap(),
            SolverResult::Unsat
        );
        let stats = export(&original, 2, &mut Vec::new(), None).unwrap();
        assert!(!stats.known_model_verified);
        assert!(stats.clauses > 0);
        assert!(build(&original, 0, &control).is_err());
        assert!(
            ProofEncoding::new(&original, &input_terms(&original), &control)
                .unwrap()
                .circuit
                .recorded
                .is_none()
        );
    }
}
