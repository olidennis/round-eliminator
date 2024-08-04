use round_eliminator_lib::algorithms::event::EventHandler;
use round_eliminator_lib::problem::Problem;
use std::time::Instant;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, action)]
    single: bool,
    #[arg(short, long, action)]
    multi : bool,
    #[arg(short, long, action)]
    dontloop : bool,
}

fn test_problem(problem : &str, steps : usize, hash : &str) -> u128 {
    let mut eh = &mut EventHandler::null();
    let mut p = Problem::from_string(problem).unwrap();

    let mut r = 0;

    for i in 0..steps-1 {
        let start = Instant::now();

        p = p.speedup(eh);
        p.passive.maximize(&mut eh);
        p.compute_partial_diagram(&mut eh);
        p.sort_active_by_strength();
        p.compute_passive_gen();
        p.rename_by_generators().unwrap();
        p.active.lines.sort();
        p.passive.lines.sort();

        let duration = start.elapsed();
        if i == steps - 2 {
            r = duration.as_millis();
        }
    }
    assert!(sha256::digest(std::hint::black_box(p.to_string())) == hash);

    r
}

fn test_all() -> u128 {
    let mut r = test_problem("M U^9\nP^10\n\nM UP^9\nU^10",11,"d6f90abf897c0ba1bcc4bcb96debf68e56d716d4096c444cd9da0e93f5213219");
    r += test_problem("M^10\nP U^9\n\nM UP\nU^2",6,"eb762856d26b16c3b0030971133fb46cf2f6da97089110a8823ae1b0c221fea2");
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
(10e) (11e)",2,"8cc0bbf43868b3255c23df6ee760590f999097f843809dbe347b48f63077126d");
    r
}


fn test_and_report(is_multi : bool) {
    let r = test_all();
    let score = 101553830 / r;
    if is_multi {
        println!("Multi Thread Score (higher is better): {}", score);
    } else {
        println!("Single Thread Score (higher is better): {}", score);
    }
}

fn main() {
    let args = Args::parse();
    loop {

        if args.multi || (!args.single && !args.multi) {
            test_and_report(true);
        }

        if args.single || (!args.single && !args.multi) {
            let old = std::env::var("RE_NUM_THREADS");
            std::env::set_var("RE_NUM_THREADS", "1");   
            test_and_report(false); 
            if let Ok(var) = old {
                std::env::set_var("RE_NUM_THREADS", var);   
            } else {
                std::env::remove_var("RE_NUM_THREADS"); 
            }
        }

        if args.dontloop {
            break;
        }
    }

    
}
