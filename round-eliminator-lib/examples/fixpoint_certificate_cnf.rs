//! Export a fixed-bound certificate CNF, validate an external model, or solve
//! the same DIMACS with the exact native MiniSat core backend used by Loop.

#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use round_eliminator_lib::{algorithms::fixpoint_sat::certificate_cnf, problem::Problem};
    use rustsat::{
        instances::SatInstance,
        solvers::{Solve, SolverResult},
    };
    use std::{
        fs::OpenOptions,
        io::{BufWriter, Write},
        time::Instant,
    };
    fn output(path: &str) -> std::io::Result<BufWriter<std::fs::File>> {
        Ok(BufWriter::new(
            OpenOptions::new().write(true).create_new(true).open(path)?,
        ))
    }
    let mut args = Vec::new();
    let mut options = certificate_cnf::InstanceOptions::default();
    for arg in std::env::args().skip(1) {
        if let Some(depth) = arg.strip_prefix("--depth=") {
            if options.max_depth.is_some() {
                return Err("Duplicate depth option".into());
            }
            options.max_depth = Some(depth.parse()?);
        } else if arg == "--symmetry" {
            options.symmetry = true;
        } else if arg.starts_with("--") {
            return Err(format!("Unknown option: {arg}").into());
        } else {
            args.push(arg);
        }
    }
    let started = Instant::now();
    match args.first().map(String::as_str) {
        Some("export") if args.len() == 4 || args.len() == 6 => {
            let original = Problem::from_string(std::fs::read_to_string(&args[1])?)?;
            let steps = args[2].parse()?;
            let mut cnf = output(&args[3])?;
            let stats = if args.len() == 6 {
                let known = std::fs::read_to_string(&args[4])?;
                let mut model = output(&args[5])?;
                let stats = certificate_cnf::export_with_options(
                    &original,
                    steps,
                    &mut cnf,
                    Some((&known, &mut model)),
                    &options,
                )?;
                model.flush()?;
                stats
            } else {
                certificate_cnf::export_with_options(&original, steps, &mut cnf, None, &options)?
            };
            cnf.flush()?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Some("verify") if args.len() == 4 => {
            let original = Problem::from_string(std::fs::read_to_string(&args[1])?)?;
            println!(
                "{}",
                certificate_cnf::verify_with_options(
                    &original,
                    args[2].parse()?,
                    &std::fs::read_to_string(&args[3])?,
                    &options,
                )?
            );
            eprintln!("Every CNF clause and the decoded universal certificate verified");
        }
        Some("solve") if args.len() == 3 => {
            if options.max_depth.is_some() || options.symmetry {
                return Err("Restrictions belong to export/verify, not solve".into());
            }
            let mut model = output(&args[2])?;
            let instance: SatInstance = SatInstance::from_dimacs_path(&args[1])?;
            let (cnf, _) = instance.into_cnf();
            let variables = cnf
                .iter()
                .flat_map(|c| c.iter())
                .map(|l| l.var().idx32() + 1)
                .max()
                .unwrap_or(0);
            let mut solver = rustsat_minisat::core::Minisat::default();
            solver.add_cnf(cnf)?;
            eprintln!("Native MiniSat core: loaded CNF in {:?}", started.elapsed());
            match solver.solve()? {
                SolverResult::Sat => {
                    println!("s SATISFIABLE");
                    certificate_cnf::write_model(&solver.full_solution()?, variables, &mut model)?;
                }
                SolverResult::Unsat => println!("s UNSATISFIABLE"),
                SolverResult::Interrupted => println!("s UNKNOWN"),
            }
            model.flush()?;
        }
        _ => {
            let usage = "Usage: fixpoint_certificate_cnf export PROBLEM STEPS CNF [KNOWN_CERTIFICATE KNOWN_MODEL] [--depth=N] [--symmetry]\n       fixpoint_certificate_cnf verify PROBLEM STEPS MODEL [--depth=N] [--symmetry]\n       fixpoint_certificate_cnf solve CNF MODEL";
            return Err(usage.into());
        }
    }
    eprintln!("Elapsed: {:?}", started.elapsed());
    Ok(())
}

#[cfg(not(all(not(target_arch = "wasm32"), feature = "all")))]
fn main() {
    eprintln!("This tool requires a native default-feature build.");
}
