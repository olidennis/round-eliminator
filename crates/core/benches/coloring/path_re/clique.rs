use rustsat::{
    encodings::card::{BoundLower, Totalizer},
    instances::{BasicVarManager, Cnf, ManageVars},
    solvers::{ControlSignal, Solve, SolveIncremental, SolverResult, Terminate},
    types::TernaryVal,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

/// LLM: Solve a preprocessed exact compatibility-graph clique instance incrementally.
/// LLM: Each selected graph vertex is one color behavior; nonedges forbid simultaneous selection.
fn main() -> anyhow::Result<()> {
    // LLM: Read the graph and keep original labels for independently checked certificates.
    let instance = PathBuf::from(std::env::args().nth(1).expect("instance JSON path"));
    let output = PathBuf::from(std::env::args().nth(2).expect("result JSON path"));
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(instance)?)?;
    let ids: Vec<u32> = serde_json::from_value(data["ids"].clone())?;
    let extra: Vec<u32> = serde_json::from_value(data["extra"].clone())?;
    let mut variables = BasicVarManager::default();
    let choices: Vec<_> = ids.iter().map(|_| variables.new_lit()).collect();
    let mut solver = rustsat_cadical::CaDiCaL::default();
    let started = Instant::now();
    solver.attach_terminator(move || {
        if started.elapsed() > Duration::from_secs(60) {
            ControlSignal::Terminate
        } else {
            ControlSignal::Continue
        }
    });
    for edge in data["nonedges"].as_array().unwrap() {
        solver.add_clause_ref(&[
            !choices[edge[0].as_u64().unwrap() as usize],
            !choices[edge[1].as_u64().unwrap() as usize],
        ])?;
    }
    // LLM: Query consecutive cardinalities; only a completed UNSAT result certifies the maximum.
    // LLM: A clique uses at most one vertex in each independently certified color class.
    // LLM: Count used classes instead of every vertex, preserving exact cardinality.
    let cardinality = if let Some(classes) = data["classes"].as_array() {
        let mut used = Vec::new();
        for class in classes {
            let literal = variables.new_lit();
            used.push(literal);
            let mut clause = vec![!literal];
            for vertex in class.as_array().unwrap() {
                let choice = choices[vertex.as_u64().unwrap() as usize];
                clause.push(choice);
                solver.add_clause_ref(&[!choice, literal])?;
            }
            solver.add_clause_ref(&clause)?;
        }
        used
    } else {
        choices.clone()
    };
    let mut encoding = Totalizer::from(cardinality);
    let lower = data["lower"].as_u64().unwrap() as usize;
    let upper = data["upper"].as_u64().unwrap() as usize;
    let mut best: Vec<u32> = serde_json::from_value(data["initial_clique"].clone())?;
    for palette in lower + 1..=upper + 1 {
        let mut clauses = Cnf::new();
        encoding.encode_lb(palette..=palette, &mut clauses, &mut variables)?;
        for clause in &clauses {
            solver.add_clause_ref(clause)?;
        }
        let result = solver.solve_assumps(&encoding.enforce_lb(palette)?)?;
        println!(
            "{}",
            serde_json::json!({"tested":palette+extra.len(),"result":format!("{result:?}"),"seconds":started.elapsed().as_secs_f64()})
        );
        match result {
            SolverResult::Interrupted => {
                std::fs::write(
                    &output,
                    serde_json::to_vec(&serde_json::json!({"status":"interrupted","clique":best}))?,
                )?;
                break;
            }
            SolverResult::Unsat => {
                std::fs::write(
                    &output,
                    serde_json::to_vec(
                        &serde_json::json!({"status":"complete","maximum":palette+extra.len()-1,"clique":best,"solver_seconds":started.elapsed().as_secs_f64()}),
                    )?,
                )?;
                break;
            }
            SolverResult::Sat => {
                let mut witness = extra.clone();
                for (&id, &lit) in ids.iter().zip(&choices) {
                    if solver.lit_val(lit)? == TernaryVal::True {
                        witness.push(id);
                    }
                }
                best = witness;
            }
        }
    }
    Ok(())
}
