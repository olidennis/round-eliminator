//! cargo run --release --example fixpoint_sat -- problem.txt [max_nodes] [max_candidates] [--full-checker] [--parallel]

#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use round_eliminator_lib::{
        algorithms::{
            event::EventHandler,
            fixpoint_sat::{SatSearchOptions, SatSearchOutcome},
        },
        problem::Problem,
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    let parallel = args.iter().any(|s| s == "--parallel");
    let full = args.iter().any(|s| s == "--full-checker");
    let mut args = args
        .into_iter()
        .filter(|s| s != "--parallel" && s != "--full-checker");
    let usage = "Usage: fixpoint_sat problem.txt [max_nodes] [max_candidates] [--full-checker] [--parallel]";
    let file = args.next().ok_or(usage)?;
    let mut options = SatSearchOptions {
        max_nodes: args.next().map(|s| s.parse()).transpose()?,
        max_candidates: args.next().map(|s| s.parse()).transpose()?,
        ..Default::default()
    };
    options.use_game = !full;
    if args.next().is_some() {
        return Err(usage.into());
    }
    let original = Problem::from_string(std::fs::read_to_string(file)?)
        .map_err(|e| format!("Invalid problem: {e}"))?;
    let mut events = EventHandler::with(|(message, current, total)| {
        if message != "Loop: searches running"
            && (message.starts_with("SAT:")
                || message.starts_with("Proof:")
                || message.starts_with("Loop:"))
        {
            eprintln!("{message}: {current}/{total}");
        }
    });
    let started = std::time::Instant::now();
    let outcome = if parallel {
        original.fixpoint_search(&options, &Default::default(), &mut events)?
    } else {
        original.fixpoint_sat(&options, &mut events)?
    };
    match outcome {
        SatSearchOutcome::Found(found) => {
            println!(
                "Found a nontrivial fixed point with {} lattice nodes.\n{}",
                found.nodes, found.problem
            );
            println!("Diagram (can be pasted into the custom fixed-point procedure):");
            print!("{}", found.diagram_text);
            eprintln!("{:?}", found.stats);
        }
        SatSearchOutcome::NoFixedPoint { certificate, stats } => {
            println!("{certificate}");
            eprintln!("{stats:?}");
        }
        SatSearchOutcome::Exhausted {
            min_nodes,
            max_nodes,
            stats,
        } => {
            println!("No good diagram with {min_nodes} through {max_nodes} nodes. Larger sizes remain undecided.");
            eprintln!("{stats:?}");
        }
        SatSearchOutcome::Inconclusive { nodes, stats } => {
            println!("Search budget reached at {nodes} nodes; this size remains undecided.");
            eprintln!("{stats:?}");
        }
    }
    eprintln!("Search elapsed: {:?}", started.elapsed());
    Ok(())
}

#[cfg(not(all(not(target_arch = "wasm32"), feature = "all")))]
fn main() {
    eprintln!("This example requires a native build with the 'all' feature (the default).");
    std::process::exit(1);
}
