#[cfg(not(target_os = "linux"))]
use mimalloc::MiMalloc;
#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use round_eliminator_lib::algorithms::fixpoint::FixpointType;
use round_eliminator_lib::algorithms::event::EventHandler;
use round_eliminator_lib::problem::Problem;
use std::collections::HashMap;
use std::time::Instant;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Use a single core.
    #[arg(short, long, action)]
    single: bool,
    /// Use all cores.
    #[arg(short, long, action)]
    multi : bool,
    /// Do only one run and terminate.
    #[arg(short, long, action)]
    dontloop : bool,
    /// Number of threads. The real number of spawned threads is twice this parameter, but each half will do a different type of work).
    #[arg(short, long)]
    threads : Option<usize>,
}


fn test_and_report(is_multi : bool) {
    let r = round_eliminator_lib::test_all();
    let score = 601318308 / r;
    if is_multi {
        println!("Multi Thread Score (higher is better): {}", score);
    } else {
        println!("Single Thread Score (higher is better): {}", score);
    }
}

fn main() {
    let args = Args::parse();

    let threads = args.threads.unwrap_or(num_cpus::get());    

    loop {

        if args.multi || (!args.single && !args.multi) {
            std::env::set_var("RE_NUM_THREADS", format!("{}",threads));  
            std::env::set_var("RAYON_NUM_THREADS", format!("{}",threads));              
            test_and_report(true);
        }

        if args.single || (!args.single && !args.multi) {
            std::env::set_var("RE_NUM_THREADS", "0");
            std::env::set_var("RAYON_NUM_THREADS", "1");   

            test_and_report(false); 
        }

        if args.dontloop {
            break;
        }
    }

    
}
