//! Native certificate search / standalone re-verification.
#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use round_eliminator_lib::{
        algorithms::{
            event::EventHandler,
            reversible_edges::{self, Certificate, Options, Report},
        },
        problem::Problem,
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    let usage = "reversible_edges search PROBLEM.txt [seconds] [threads]\nreversible_edges apply REPORT.json INDEX\nreversible_edges verify GUI_CERTIFICATE.json";
    if args.len() < 2 {
        return Err(usage.into());
    }
    let mut eh = EventHandler::with(|(message, a, b)| eprintln!("{message}: {a}/{b}"));
    match args[0].as_str() {
        "search" if args.len() <= 4 => {
            let p = Problem::from_string(std::fs::read_to_string(&args[1])?)?;
            let mut options = Options::default();
            if args.len() >= 3 {
                options.seconds = args[2].parse()?;
            }
            if args.len() == 4 {
                options.threads = args[3].parse()?;
            }
            let report = reversible_edges::search(&p, &options, &mut eh, |r| {
                eprintln!("Verified sets so far: {}", r.certificates.len());
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "apply" if args.len() == 3 => {
            let report: Report = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
            let index: usize = args[2].parse()?;
            let c = report
                .certificates
                .get(index)
                .ok_or("Certificate index out of range")?;
            println!("{}", reversible_edges::apply(&report.original, c, &mut eh)?);
        }
        "verify" if args.len() == 2 => {
            #[derive(serde::Deserialize)]
            struct Saved {
                problem: Problem,
                certificate: Certificate,
            }
            let saved: Saved = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
            println!(
                "{}",
                reversible_edges::apply(&saved.problem, &saved.certificate, &mut eh)?
            );
        }
        _ => return Err(usage.into()),
    }
    Ok(())
}
#[cfg(not(all(not(target_arch = "wasm32"), feature = "all")))]
fn main() {
    eprintln!("This example requires the native all-feature build.");
}
