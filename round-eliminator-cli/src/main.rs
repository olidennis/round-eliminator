use clap::Parser;
use round_eliminator_lib::problem::Problem;
use std::thread;
use round_eliminator_lib::line::Degree;
use round_eliminator_lib::algorithms::event::EventHandler;
use std::sync::Arc;
use std::sync::Mutex;
use std::fmt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    file: String,
    #[arg(short, long)]
    coloring : Option<usize>
}

#[derive(Copy,Clone,Eq,PartialEq)]
enum Bound {
    Rounds(usize),
    LogStar,
    Log,
    Unknown
}


struct BoundRange {
    lb : Bound,
    ub : Bound
}

fn get_eh(bound : Arc<Mutex<BoundRange>>) -> EventHandler<'static> {
    EventHandler::with(move |x: (String, usize, usize)| {
        let bound = bound.lock().unwrap();
        if bound.lb == bound.ub && bound.lb != Bound::Unknown {
            //this is ugly
            std::process::exit(0);
        }
    })
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Bound::Rounds(x) => { write!(f, "Rounds({})", x) }
            Bound::LogStar => { write!(f, "LogStar") }
            Bound::Log => { write!(f, "Log") }
            Bound::Unknown => { write!(f, "Unknown") }
        }
        
    }
}
impl fmt::Display for BoundRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.lb, self.ub)
    }
}

impl BoundRange {
    fn new() -> Self {
        Self { lb : Bound::Unknown, ub : Bound::Unknown }
    }
    fn new_lb(&mut self, b : Bound) {
        match (self.lb,b) {
            (Bound::Unknown, _) => { self.lb = b; },
            (Bound::Log, _) => {},
            (_, Bound::Log | Bound::LogStar) => { self.lb = b; },
            (Bound::Rounds(x), Bound::Rounds(y)) if x < y => { self.lb = b; },
            _ => {}
        }
    }

    fn new_ub(&mut self, b : Bound) {
        match (self.ub,b) {
            (Bound::Unknown, _) => { self.ub = b; },
            (Bound::Rounds(x), Bound::Rounds(y)) if y < x => { self.ub = b; },
            (Bound::Log | Bound::LogStar, Bound::Rounds(_) | Bound::LogStar) => { self.ub = b; },
            _ => {}
        }
    }

}

fn automatic_upper_bound(p : &Problem, c : Option<usize>, bound : Arc<Mutex<BoundRange>>) {
    let mut eh = get_eh(bound.clone());
    p.autoautoub(false, 0, false, 0, false, 0, c, |len,is_trivial,mut sequence|{
        if is_trivial {
            bound.lock().unwrap().new_ub(Bound::Rounds(len));
        } else {
            bound.lock().unwrap().new_ub(Bound::LogStar);
        }
        println!("Upper bound of {} rounds",len);
        println!("{}", bound.lock().unwrap());
    }, &mut eh);
}

fn automatic_lower_bound_1(p : &Problem, c : Option<usize>, bound : Arc<Mutex<BoundRange>>) {
    let mut eh = get_eh(bound.clone());
    p.autoautolb(false, 0, false, 0, true, 30, c, |len,mut sequence|{
        bound.lock().unwrap().new_lb(Bound::Rounds(len));
        println!("{}", bound.lock().unwrap());
    }, &mut eh);
}

fn automatic_lower_bound_2(p : &Problem, c : Option<usize>, bound : Arc<Mutex<BoundRange>>) {
    let mut eh = get_eh(bound.clone());
    p.autoautolb(false, 0, true, 100, true, 30, c, |len,mut sequence|{
        bound.lock().unwrap().new_lb(Bound::Rounds(len));
        println!("{}", bound.lock().unwrap());
    }, &mut eh);
}

fn automatic_fixed_point(p : &Problem, c : Option<usize>, bound : Arc<Mutex<BoundRange>>) {
    let mut eh = get_eh(bound.clone());
    match p.fixpoint_loop(&mut eh) {
        Ok(mut new) => {
            bound.lock().unwrap().new_lb(Bound::Log);
            println!("{}", bound.lock().unwrap());
        }
        Err(s) => {  }
    }
}

fn just_speedups(p : &Problem, c : Option<usize>, bound : Arc<Mutex<BoundRange>>) {
    let mut eh = get_eh(bound.clone());
    let mut p = p.clone();
    let mut i = 0;
    loop {
        p.compute_triviality(&mut eh);
        let is_trivial = if c.is_some() && p.passive.degree == Degree::Finite(2) {
            p.compute_coloring_solvability(&mut eh);
            p.coloring_sets.as_ref().unwrap().len() >= c.unwrap()
        } else {
            p.trivial_sets.as_ref().unwrap().len() > 0 
        };
        if is_trivial {
            bound.lock().unwrap().new_ub(Bound::Rounds(i));
        } else {
            bound.lock().unwrap().new_lb(Bound::Rounds(i+1));
        }
        println!("{}", bound.lock().unwrap());
        p = p.speedup(&mut eh);
        i += 1;
    }
}

fn automatic_bounds(p : &Problem, c : Option<usize>) {
    let bound = Arc::new(Mutex::new(BoundRange::new()));
    thread::scope(|s| {
        let b1 = bound.clone();
        let b2 = bound.clone();
        let b3 = bound.clone();
        let b4 = bound.clone();
        let b5 = bound.clone();
        s.spawn(|| {
            automatic_upper_bound(p,c,b1);
        });
        s.spawn(|| {
            automatic_lower_bound_1(p,c,b2);
        });
        s.spawn(|| {
            automatic_lower_bound_2(p,c,b3);
        });
        s.spawn(|| {
            automatic_fixed_point(p,c,b4);
        });
        s.spawn(|| {
            just_speedups(p,c,b5);
        });
    });
}

fn main() {
    let args = Args::parse();
    let file = args.file;
    let coloring = args.coloring;
    let problem = if file != "-" {
        std::fs::read_to_string(file).unwrap()
    } else {
        std::io::read_to_string(std::io::stdin()).unwrap()
    };
    
    let mut problem = Problem::from_string(problem).unwrap();
    println!("{}", problem);
    if let Some(c) = coloring {
        println!("A {} coloring is given\n", c);
    }
    problem.compute_partial_diagram(&mut EventHandler::null());
    automatic_bounds(&problem, coloring);
}
