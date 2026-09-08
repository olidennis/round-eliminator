//! Extract (or verify) an algorithm from an existing certificate, without
//! running any fixed-point or certificate search.
#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use round_eliminator_lib::{
        algorithms::{
            event::EventHandler,
            nofixpoint::algorithm::{self, Algorithm, Options, Outcome},
        },
        problem::Problem,
    };
    use std::io::Write;
    let args: Vec<_> = std::env::args().skip(1).collect();
    let usage = "Usage: fixpoint_certificate_algorithm extract PROBLEM CERTIFICATE OUTPUT.json [seconds] [--sat-only]\n       fixpoint_certificate_algorithm normalize PROBLEM CERTIFICATE OUTPUT.txt [seconds]\n       fixpoint_certificate_algorithm verify PROBLEM ALGORITHM.json";
    if args.len() < 3 {
        return Err(usage.into());
    }
    let problem = Problem::from_string(std::fs::read_to_string(&args[1])?)?;
    if args[0] == "verify" && args.len() == 3 {
        let saved: Algorithm = serde_json::from_str(&std::fs::read_to_string(&args[2])?)?;
        algorithm::verify(&problem, &saved)?;
        println!(
            "Verified {} edge games and {} priorities, with {} colors.",
            saved.colors * (saved.colors - 1) / 2 * saved.degree * saved.degree,
            saved.ranks.len(),
            saved.colors
        );
        return Ok(());
    }
    if !matches!(args[0].as_str(), "extract" | "normalize") || args.len() < 4 {
        return Err(usage.into());
    }
    if std::path::Path::new(&args[3]).exists() {
        return Err("Refusing to overwrite output file".into());
    }
    let mut options = Options::default();
    for arg in &args[4..] {
        if arg == "--sat-only" {
            options.try_simple_orders = false;
        } else {
            options.time_limit = Some(std::time::Duration::from_secs(arg.parse()?));
        }
    }
    let certificate = std::fs::read_to_string(&args[2])?;
    let mut eh =
        EventHandler::with(|(message, current, total)| eprintln!("{message}: {current}/{total}"));
    if args[0] == "normalize" {
        match algorithm::normalize_certificate(&problem, &certificate, options.time_limit, &mut eh)?
        {
            Some(normalized) => {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&args[3])?;
                file.write_all(normalized.as_bytes())?;
                println!("Verified synchronized certificate saved to {}", args[3]);
            }
            None => println!("Certificate reconstruction budget reached; inconclusive."),
        }
        return Ok(());
    }
    match algorithm::extract(&problem, &certificate, &options, &mut eh)? {
        Outcome::Found(found) => {
            algorithm::verify(&problem, &found)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args[3])?;
            serde_json::to_writer_pretty(&mut file, &found)?;
            writeln!(file)?;
            println!(
                "Verified algorithm: {} colors, {} priority phases after coloring. Saved to {}.",
                found.colors,
                found.ranks.len(),
                args[3]
            );
            println!("{}", serde_json::to_string_pretty(&found.stats)?);
        }
        Outcome::NoSchedule(stats) => {
            println!("No schedule exists for this certificate's chosen synchronized derivation in the priority-based extraction scheme. This is NOT a lower bound on distributed algorithms, nor does it exclude other reconstructions.\n{}",serde_json::to_string_pretty(&stats)?);
        }
        Outcome::Inconclusive(stats) => println!(
            "Algorithm extraction budget reached (time or reconstruction limit); inconclusive.\n{}",
            serde_json::to_string_pretty(&stats)?
        ),
    }
    Ok(())
}

#[cfg(not(all(not(target_arch = "wasm32"), feature = "all")))]
fn main() {
    eprintln!("This example requires a native build with the all feature.");
}
