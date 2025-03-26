use std::collections::HashMap;

use crate::{constraint::Constraint, group::{Group, GroupType, Label}, line::Line, part::Part, problem::Problem};
use itertools::Itertools;

use super::event::EventHandler;





impl Problem {

    pub fn randomized_success_prob(&mut self, eh : &mut EventHandler) {
        self.passive.maximize(eh);

        let outputs = self.active.all_choices(true);

        let mut best_prob = 0f32;
        let mut best_subproblem = self.clone();

        //let sets = outputs.into_iter().powerset();
        let sets = (1..=4).flat_map(|size|outputs.iter().cloned().combinations(size));

        for lines in sets {
            let c = Constraint{ lines, is_maximized: false, degree: self.active.degree  };
            let labels = c.labels_appearing();
            let problem = self.harden_keep(&labels, false);
            let lines = problem.active.all_choices(true);

            if lines.is_empty() {
                continue;
            }

            let mut freq = HashMap::<Label,usize>::new();
            for line in lines {
                for part in &line.parts {
                    let label = part.group.first();
                    let exp = part.gtype.value();
                    *freq.entry(label).or_default() += exp;
                }
            }

            let mut good = 0;
            let mut bad = 0;

            for passive in (0..self.passive.finite_degree()).map(|_|freq.iter()).multi_cartesian_product() {
                let parts : Vec<_> = passive.iter().map(|lc|
                    Part {
                        group : Group::from(vec![*lc.0]),
                        gtype : GroupType::Many(1)
                    }).collect();
                let line = Line{ parts };
                let count = passive.iter().fold(1,|a,lc|a * lc.1);
                if self.passive.includes(&line) {
                    good += count;
                } else {
                    bad += count;
                }
                
            }

            let prob = good as f32 / (good + bad) as f32;
            if prob > best_prob {
                best_prob = prob;
                best_subproblem = problem;

                let e = std::f64::consts::E;
                let p = 1f32 - best_prob;
                let d = (self.active.finite_degree() -1) * self.passive.finite_degree();
                println!("epd {}",  e as f32 * p * d as f32);
                //println!("best prob {}\n{}\n",best_prob,best_subproblem);    
            }

        }

        let e = std::f64::consts::E;
        let p = 1f32 - best_prob;
        let d = (self.active.finite_degree() -1) * self.passive.finite_degree();
        println!("epd {}",  e as f32 * p * d as f32);
        //println!("best prob {}\n{}\n",best_prob,best_subproblem);

    }

}