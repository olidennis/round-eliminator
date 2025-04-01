use std::{cmp::Ordering, collections::HashSet, usize};

use crate::{group::{Group, GroupType, Label}, line::Line, part::Part, problem::Problem};
use chrono::format::parse;
use genevo::{operator::prelude::*, population::*, prelude::*, types::fmt::Display};
use itertools::{max, Itertools};

use super::{diagram::diagram_indirect_to_reachability_adj, event::EventHandler, fixpoint::right_closed_subsets};


type ProblemGenome = Vec<Label>;

#[derive(Debug,Clone,Copy)]
struct Params{
    max_labels : usize,
    max_labels_before_re : usize,
    max_labels_for_zero_check : usize,
    max_rcs : usize,
    re_steps : usize
}

trait AsPhenotype {
    fn as_problem(&self, base_problem : &Problem, params : Params) -> (Vec<Problem>,FitnessScore);
}
enum ProblemScore {
    ZERO,
    Score(usize)
}

impl Problem {
    fn genetic_score(&mut self) -> ProblemScore {
        let eh = &mut EventHandler::null();
        self.discard_useless_stuff(false, eh);


        self.trivial_sets = None;
        //println!("triviality");
        if self.passive.finite_degree() == 2 {
            self.compute_triviality(eh);
            let is_trivial = self.trivial_sets.as_ref().unwrap().len() > 0;
            if is_trivial {
                return ProblemScore::ZERO;
            }
        }
        //println!("done");


        if self.passive.finite_degree() == 2 {
            self.coloring_sets = None;
            //println!("coloring triviality");
            self.compute_coloring_solvability(eh);
            //println!("done");
            if self.coloring_sets.as_ref().unwrap().len() > 2 /*self.active.finite_degree()*/ {
                return ProblemScore::ZERO;
            }
        }

        let labels = self.labels();
        self.diagram_indirect = None;
        //println!("merging");
        self.compute_partial_diagram(eh);
        *self = self.merge_equivalent_labels();
        self.compute_partial_diagram(eh);
        //println!("done");
        //println!("rcs");
        let diagram_indirect = self.diagram_indirect.as_ref().unwrap();
        let successors =  diagram_indirect_to_reachability_adj(&labels, diagram_indirect);
        //println!("{}",successors.len());
        let start = std::time::Instant::now();
        let rcs = right_closed_subsets(&labels, &successors);
        ProblemScore::Score(rcs.len())
    }

    fn new_labeling(&mut self) {
        let labels = self.labels();
        if labels.is_empty() {
            return;
        }
        let mapping_label_text = labels.into_iter().map(|i|(i as Label,format!("({})",i))).collect();
        self.mapping_label_text = mapping_label_text;
    }
}


fn parse_relax(it : &mut impl Iterator<Item=Label>, p : &Problem) -> Option<(Label,Label)> {
    let labels = p.labels();
    //let a = it.next().unwrap_or(0) as usize % labels.len();
    //let b = it.next().unwrap_or(0) as usize % labels.len();
    let a = it.next().unwrap_or(0);
    let b = it.next().unwrap_or(0);
    if labels.contains(&a) && labels.contains(&b) {
        Some((a,b))
    } else {
        None
    }
}

fn parse_relax_on_arrow(it : &mut impl Iterator<Item=Label>, p : &mut Problem) -> Option<(Label,Label)>{
    let eh = &mut EventHandler::null();
    let a = it.next().unwrap_or(0);
    let b = it.next().unwrap_or(0);
    if p.diagram_direct.is_none(){
        p.compute_partial_diagram(eh);
    }
    let diag = p.diagram_direct_to_succ_adj();
    if !diag.is_empty() {
        //let max_key = diag.keys().max().unwrap();
        //let a = a % (max_key+1);
        //let h = HashSet::new();
        //let succ_a = diag.get(&a).unwrap_or(&h).into_iter().collect_vec();
        if let Some(succ_a) = diag.get(&a) {
            if succ_a.len() > 0 {
                //let b = *succ_a[b as usize % succ_a.len()];
                if succ_a.contains(&b) {
                    return Some((a,b));
                }
            }
        }
    }
    return None;
}

fn parse_configuration(it : &mut impl Iterator<Item=Label>, max_labels : usize, d : usize) -> Option<Line> {
    let mut v : Vec<_> = it.take(d).map(|x| (x % max_labels as Label)).collect();
    for _ in v.len()..d {
        v.push(0);
    }
    let line = Line{ parts: v.into_iter().map(|l|{
        Part{ gtype: GroupType::Many(1), group: Group::from(vec![l])  }
    }).collect()};

    Some(line)
}

impl AsPhenotype for ProblemGenome {
    fn as_problem(&self, base_problem : &Problem, params : Params) -> (Vec<Problem>,FitnessScore) {
        let max_labels = params.max_labels;
        let max_labels_before_re = params.max_labels_before_re;
        let max_rcs = params.max_rcs;
        let re_steps = params.re_steps;
        let max_labels_for_zero_check = params.max_labels_for_zero_check;

        let mut scores = vec![];
        let mut problems = vec![base_problem.clone()];

        let eh = &mut EventHandler::null();
        let mut p = base_problem.clone();
        let mut it = self.iter().cloned().chain(std::iter::repeat(33).take(30));

        while let Some(x) = it.next() {
            let it = &mut it;
            let it = &mut it.map(|x|x % max_labels as Label);
            match x % 100 { 
                0..15 => {
                    if let Some((a,b)) = parse_relax(it,&p) {
                        p = p.relax_merge(a, b);
                    }
                },
                15..30 => {
                    if let Some((a,b)) = parse_relax_on_arrow(it, &mut p) {
                        p = p.relax_merge(a, b);
                    }
                }
                30 => {
                    if let Some((a,b)) = parse_relax(it,&p) {
                        p = p.relax_addarrow(a, b);
                    }
                },
                31 => {
                    let d = p.active.finite_degree();
                    if let Some(line) = parse_configuration(it, max_labels, d) {
                        p.active.lines.push(line);
                    }
                },
                32 => {
                    let d = p.passive.finite_degree();
                    if let Some(line) = parse_configuration(it, max_labels, d) {
                        p.passive.lines.push(line);
                    }
                },
                33..40 => {
                    p.new_labeling();
                    let p_before_speedup = p.clone();

                    if p.labels().len() > max_labels_for_zero_check {
                        problems.push(p_before_speedup);
                        return (problems,FitnessScore::Score(scores));
                    }

                    //println!("computing score 1");
                    match p.genetic_score() {
                        ProblemScore::ZERO => { 
                            //println!("done");
                            return (problems,FitnessScore::ZERO); },
                        ProblemScore::Score(score) => {
                            //println!("done");
                            scores.push(score);
                            if score > max_rcs {
                                problems.push(p_before_speedup);
                                return (problems,FitnessScore::Score(scores));
                            }
                        }
                    }

                    if p.labels().len() > max_labels {
                        problems.push(p_before_speedup);
                        return (problems,FitnessScore::Score(scores));
                    }

                    for i in 0..2 {
                        if p.diagram_direct.is_none() {
                            p.compute_partial_diagram(eh);
                        }
                        p = p.merge_equivalent_labels();
                        if p.labels().len() > max_labels_before_re {
                            problems.push(p_before_speedup);
                            return (problems,FitnessScore::Score(scores));
                        }
                        //println!("speedup");
                        p = p.speedup(eh);
                        //println!("done");
                    }
                    p.new_labeling();
                    if p.diagram_direct.is_none() {
                        p.compute_partial_diagram(eh);
                    }
                    p = p.merge_equivalent_labels();

                    if p.labels().len() > max_labels_for_zero_check {
                        problems.push(p_before_speedup);
                        return (problems,FitnessScore::Score(scores));
                    }

                    problems.push(p.clone());

                    //println!("computing score 2");
                    match p.genetic_score() {
                        ProblemScore::ZERO => { 
                            //println!("done");
                            return (problems,FitnessScore::ZERO); },
                        ProblemScore::Score(score) => {
                            //println!("done");
                            scores.push(score);
                            if score > max_rcs {
                                return (problems,FitnessScore::Score(scores));
                            }
                        }
                    }

                    if p.labels().len() > max_labels {
                        problems.push(p_before_speedup);
                        return (problems,FitnessScore::Score(scores));
                    }

                    
                    if problems.len() >= 2 && problems[problems.len()-1] == problems[problems.len()-2] {
                        return (problems,FitnessScore::FP);
                    }
                    if problems.len() >= re_steps {
                        return (problems,FitnessScore::Score(scores));
                    }
                },
                _ => {}
            }
        }

        return (problems,FitnessScore::Score(scores));
    }
}



#[derive(Debug, Clone)]
struct GeneticProblem {
    problem: Problem,
    params : Params
}

impl GeneticProblem {
    fn new(p : Problem, params : Params) -> Self {
        Self{ problem : p.clone(), params }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum FitnessScore{
    ZERO,
    Score(Vec<usize>),
    FP
}

impl Ord for FitnessScore {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self,other) {
            (FitnessScore::ZERO, FitnessScore::ZERO) => Ordering::Equal,
            (FitnessScore::ZERO, FitnessScore::Score(_)) => Ordering::Less,
            (FitnessScore::ZERO, FitnessScore::FP) => Ordering::Less,
            (FitnessScore::Score(_), FitnessScore::ZERO) => Ordering::Greater,
            (FitnessScore::Score(items), FitnessScore::Score(items2)) => {
                if items.len() != items2.len() {
                    return items.len().cmp(&items2.len());
                }
                let items = items.into_iter().rev().collect_vec();
                let items2 = items2.into_iter().rev().collect_vec();
                items2.cmp(&items)
            },
            (FitnessScore::Score(_), FitnessScore::FP) => Ordering::Less,
            (FitnessScore::FP, FitnessScore::ZERO) => Ordering::Greater,
            (FitnessScore::FP, FitnessScore::Score(_)) => Ordering::Greater,
            (FitnessScore::FP, FitnessScore::FP) => Ordering::Equal,
        }
    }
}

impl PartialOrd for FitnessScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Fitness for FitnessScore {
    fn zero() -> Self {
        FitnessScore::ZERO
    }

    fn abs_diff(&self, other: &Self) -> Self {
        unimplemented!()
    }
}

impl<'a> FitnessFunction<ProblemGenome, FitnessScore> for &'a GeneticProblem {
    fn fitness_of(&self, genome: &ProblemGenome) -> FitnessScore {
        let (_,score) = genome.as_problem(&self.problem, self.params);
        score
    }

    fn average(&self, _fitness_values: &[FitnessScore]) -> FitnessScore {
        return FitnessScore::ZERO;
    }

    fn highest_possible_fitness(&self) -> FitnessScore {
        FitnessScore::FP
    }

    fn lowest_possible_fitness(&self) -> FitnessScore {
        FitnessScore::ZERO
    }
}


impl Problem{
    pub fn find_fixpoint_with_genetic(&self){
        let genome_length = 100;
        let max_labels = 20;
        let max_labels_before_re = 15;
        let max_labels_for_zero_check = 100;
        let max_rcs = 50000;
        let population = 10;
        let selection_ratio = 0.85;
        let individuals_per_parent = 5;
        let crossover_points = 1;
        let reinsertion_ratio = 0.85;
        let re_steps = 20;
        let mutation_rate = 0.3;//20.1 / genome_length as f64;

        let params = Params{max_labels, max_labels_before_re, max_rcs, re_steps, max_labels_for_zero_check};

        let gp = GeneticProblem::new(self.clone(), params);

        let initial_population: Population<ProblemGenome> = build_population()
            .with_genome_builder(ValueEncodedGenomeBuilder::new(genome_length, 0, 100000))
            .of_size(population)
            .uniform_at_random();

        let mut sim = simulate(
            genetic_algorithm()
                .with_evaluation(&gp)
                .with_selection(MaximizeSelector::new(selection_ratio, individuals_per_parent))
                .with_crossover(MultiPointCrossBreeder::new(crossover_points))
                .with_mutation(RandomValueMutator::new(mutation_rate, 0, 100000))
                .with_reinsertion(ElitistReinserter::new(&gp, true, reinsertion_ratio))
                .with_initial_population(initial_population)
                .build(),
        )
        .until(FitnessLimit::new(FitnessScore::FP))
        .build();


        println!("starting genetic algorithm");

        loop {
            let result = sim.step();
            match result {
                Ok(SimResult::Intermediate(step)) => {
                    //let evaluated_population = step.result.evaluated_population;
                    let best_solution = step.result.best_solution;
                    println!(
                        "Step: generation: {}, \
                        best fitness: {:?}, duration: {}, processing_time: {}",
                        step.iteration,
                        best_solution.solution.fitness,
                        step.duration.fmt(),
                        step.processing_time.fmt()
                    );
                    let (p,_) = best_solution.solution.genome.as_problem(self, params);
                    println!("{}", p.last().unwrap());
                }
                Ok(SimResult::Final(step, processing_time, duration, stop_reason)) => {
                    let best_solution = step.result.best_solution;
                    println!("{}", stop_reason);
                    println!(
                        "Final result after {}: generation: {}, \
                         best solution with fitness {:?} found in generation {}, processing_time: {}",
                        duration.fmt(),
                        step.iteration,
                        best_solution.solution.fitness,
                        best_solution.generation,
                        processing_time.fmt()
                    );
                    let (p,_) = best_solution.solution.genome.as_problem(self, params);
                    println!("{}", p.last().unwrap());
                    break;
                }
                Err(error) => {
                    println!("{}", error);
                    break;
                },
            }
        }
    }
}

#[test]
fn genetic_test(){
    let p = Problem::from_string("A A A A
B B B B
C C C C
D D D D
E X X X

X XABCDE
A BCD
B CD
C D").unwrap();


let p = Problem::from_string("A A A
B B B
C C C

A BC
B C").unwrap();

let p = Problem::from_string("A A X
B B Y

AX BY
XY XY").unwrap();

p.find_fixpoint_with_genetic();


}