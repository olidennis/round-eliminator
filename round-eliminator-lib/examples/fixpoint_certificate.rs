//! Standalone proof synthesis: problem.txt [max_steps] [conflict_limit].
//! Native GUI Loop runs this search alongside diagram synthesis automatically.

#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use round_eliminator_lib::{
        algorithms::{
            event::EventHandler,
            fixpoint_sat::{CertificateSearchOptions, CertificateSearchOutcome},
        },
        problem::Problem,
    };
    let mut args = std::env::args().skip(1);
    let file = args
        .next()
        .ok_or("Usage: fixpoint_certificate problem.txt [max_steps] [conflict_limit]")?;
    let options = CertificateSearchOptions {
        max_steps: args.next().map(|s| s.parse()).transpose()?,
        conflict_limit: args.next().map(|s| s.parse()).transpose()?,
    };
    if args.next().is_some() {
        return Err("Too many arguments".into());
    }
    let original = Problem::from_string(std::fs::read_to_string(file)?)?;
    let mut events = EventHandler::with(|(message, current, total)| {
        if message.starts_with("Proof:") {
            eprintln!("{message}: {current}/{total}");
        }
    });
    let started = std::time::Instant::now();
    match original.fixpoint_certificate(&options, &mut events)? {
        CertificateSearchOutcome::Found {
            certificate,
            steps,
            shared_lines,
        } => {
            println!("{certificate}");
            eprintln!("Verified certificate: {steps} steps, {shared_lines} shared derivations");
        }
        outcome => println!("{outcome:?}; no conclusion about existence of a good diagram."),
    }
    eprintln!("Proof search elapsed: {:?}", started.elapsed());
    Ok(())
}

#[cfg(not(all(not(target_arch = "wasm32"), feature = "all")))]
fn main() {
    eprintln!("This example requires a native build with the 'all' feature.");
}
