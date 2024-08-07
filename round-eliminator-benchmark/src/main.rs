use round_eliminator_lib::algorithms::fixpoint::FixpointType;
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

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

fn fp(problem : &str, hash : &str) -> u128 {
    let problem = std::hint::black_box(problem);
    let eh = &mut EventHandler::null();
    let mut p = Problem::from_string(problem).unwrap();
    p.compute_partial_diagram(eh);

    let start = Instant::now();
    let mut p = p.fixpoint_generic(None,FixpointType::Basic, false, eh).unwrap().0;
    let duration = start.elapsed();

    p.active.lines.sort();
    p.passive.lines.sort();

    //println!("{}",sha256::digest(p.to_string()));
    assert!(sha256::digest(std::hint::black_box(p.to_string())) == hash);

    duration.as_millis()
}


fn re(problem : &str, steps : usize, hash : &str) -> u128 {
    let problem = std::hint::black_box(problem);
    let mut eh = &mut EventHandler::null();
    let mut p = Problem::from_string(problem).unwrap();

    let mut r = 0;

    for i in 0..steps-1 {

        p = p.speedup(eh);

        let start = Instant::now();
        p.passive.maximize(&mut eh);
        let duration = start.elapsed();
        if i == steps - 2 {
            r = duration.as_millis();
        }
        
        p.compute_partial_diagram(&mut eh);
        p.sort_active_by_strength();
        p.compute_passive_gen();
        p.rename_by_generators().unwrap();
        p.active.lines.sort();
        p.passive.lines.sort();


    }
    //println!("{}",sha256::digest(p.to_string()));
    assert!(sha256::digest(std::hint::black_box(p.to_string())) == hash);

    r
}

fn test_all() -> u128 {
    let mut r = fp("A^5 X^2
B^5 Y^2
C^5 Z^2

AX BYCZ
BY CZ
XYZ^2","f20f189c86b1fc3c53b55cb99292e819b49e3e84fc7b775057b447be7d6f5f8d");
    r += re("(0a) (00b) (00c) (00d) (00e)
(0a) (00b) (00c) (00d) (01e)
(0a) (00b) (00c) (01d) (10e)
(0a) (00b) (00c) (01d) (11e)
(0a) (00b) (01c) (10d) (00e)
(0a) (00b) (01c) (10d) (01e)
(0a) (00b) (01c) (11d) (10e)
(0a) (00b) (01c) (11d) (11e)
(0a) (01b) (10c) (00d) (00e)
(0a) (01b) (10c) (00d) (01e)
(0a) (01b) (10c) (01d) (10e)
(0a) (01b) (10c) (01d) (11e)
(0a) (01b) (11c) (10d) (00e)
(0a) (01b) (11c) (10d) (01e)
(0a) (01b) (11c) (11d) (10e)
(0a) (01b) (11c) (11d) (11e)
(1a) (10b) (00c) (00d) (00e)
(1a) (10b) (00c) (00d) (01e)
(1a) (10b) (00c) (01d) (10e)
(1a) (10b) (00c) (01d) (11e)
(1a) (10b) (01c) (10d) (00e)
(1a) (10b) (01c) (10d) (01e)
(1a) (10b) (01c) (11d) (10e)
(1a) (10b) (01c) (11d) (11e)
(1a) (11b) (10c) (00d) (00e)
(1a) (11b) (10c) (00d) (01e)
(1a) (11b) (10c) (01d) (10e)
(1a) (11b) (10c) (01d) (11e)
(1a) (11b) (11c) (10d) (00e)
(1a) (11b) (11c) (10d) (01e)
(1a) (11b) (11c) (11d) (10e)
(1a) (11b) (11c) (11d) (11e)

(0a) (0a) (1a)
(00b) (00b) (00b)
(00b) (01b) (01b)
(00b) (00b) (10b)
(00b) (00b) (11b)
(00b) (01b) (10b)
(00b) (01b) (11b)
(01b) (01b) (10b)
(01b) (01b) (11b)
(00b) (10b) (11b)
(01b) (10b) (10b)
(01b) (11b) (11b)
(10b) (10b) (10b)
(10b) (10b) (11b)
(10b) (11b) (11b)
(11b) (11b) (11b)
(00c) (00c) (00c)
(00c) (01c) (01c)
(00c) (00c) (10c)
(00c) (00c) (11c)
(00c) (01c) (10c)
(00c) (01c) (11c)
(01c) (01c) (10c)
(01c) (01c) (11c)
(00c) (10c) (11c)
(01c) (10c) (10c)
(01c) (11c) (11c)
(10c) (10c) (10c)
(10c) (10c) (11c)
(10c) (11c) (11c)
(11c) (11c) (11c)
(00d) (00d) (00d)
(00d) (01d) (01d)
(00d) (00d) (10d)
(00d) (00d) (11d)
(00d) (01d) (10d)
(00d) (01d) (11d)
(01d) (01d) (10d)
(01d) (01d) (11d)
(00d) (10d) (11d)
(01d) (10d) (10d)
(01d) (11d) (11d)
(10d) (10d) (10d)
(10d) (10d) (11d)
(10d) (11d) (11d)
(11d) (11d) (11d)
(00e) (00e) (00e)
(00e) (01e) (01e)
(00e) (00e) (10e)
(00e) (00e) (11e)
(00e) (01e) (10e)
(00e) (01e) (11e)
(01e) (01e) (10e)
(01e) (01e) (11e)
(00e) (10e) (11e)
(01e) (10e) (10e)
(01e) (11e) (11e)
(10e) (10e) (10e)
(10e) (10e) (11e)
(10e) (11e) (11e)
(11e) (11e) (11e)",2,"74f73f46ec50fe1d9ea196c55b480bcf3ce623f410aa0080904b1c0af84c55f8");
   /*let mut r = test_problem("M U^13\nP^14\n\nM UP^13\nU^14",11,"0100cb8310624dc11e281955c4ca195de1f5af70b53e95fc9b86ce6ba0c2dfca");
    r += test_problem("3^10
8 9^9
1^10
4^10
5^9 0
6^9 0
2^9 0
7^8 0^2

841902 30
341905627 90
8341905627 0
34906 190
390 41902
31905 490",4,"9a275179d50e7e192d44f1f5aa1c413b98ef8cbf437d7946f7c822872b0bcabd");
    r += test_problem("(0a) (00b) (00c) (00d) (00e)
(0a) (00b) (00c) (00d) (01e)
(0a) (00b) (00c) (01d) (10e)
(0a) (00b) (00c) (01d) (11e)
(0a) (00b) (01c) (10d) (00e)
(0a) (00b) (01c) (10d) (01e)
(0a) (00b) (01c) (11d) (10e)
(0a) (00b) (01c) (11d) (11e)
(0a) (01b) (10c) (00d) (00e)
(0a) (01b) (10c) (00d) (01e)
(0a) (01b) (10c) (01d) (10e)
(0a) (01b) (10c) (01d) (11e)
(0a) (01b) (11c) (10d) (00e)
(0a) (01b) (11c) (10d) (01e)
(0a) (01b) (11c) (11d) (10e)
(0a) (01b) (11c) (11d) (11e)
(1a) (10b) (00c) (00d) (00e)
(1a) (10b) (00c) (00d) (01e)
(1a) (10b) (00c) (01d) (10e)
(1a) (10b) (00c) (01d) (11e)
(1a) (10b) (01c) (10d) (00e)
(1a) (10b) (01c) (10d) (01e)
(1a) (10b) (01c) (11d) (10e)
(1a) (10b) (01c) (11d) (11e)
(1a) (11b) (10c) (00d) (00e)
(1a) (11b) (10c) (00d) (01e)
(1a) (11b) (10c) (01d) (10e)
(1a) (11b) (10c) (01d) (11e)
(1a) (11b) (11c) (10d) (00e)
(1a) (11b) (11c) (10d) (01e)
(1a) (11b) (11c) (11d) (10e)
(1a) (11b) (11c) (11d) (11e)

(0a) (1a)
(00b) (00b)
(01b) (01b)
(00b) (10b)
(01b) (11b)
(10b) (11b)
(00c) (00c)
(01c) (01c)
(00c) (10c)
(01c) (11c)
(10c) (11c)
(00d) (00d)
(01d) (01d)
(00d) (10d)
(01d) (11d)
(10d) (11d)
(00e) (00e)
(01e) (01e)
(00e) (10e)
(01e) (11e)
(10e) (11e)",2,"8cc0bbf43868b3255c23df6ee760590f999097f843809dbe347b48f63077126d");*/
    r
}


fn test_and_report(is_multi : bool) {
    let r = test_all();
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
