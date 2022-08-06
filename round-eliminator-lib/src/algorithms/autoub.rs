use std::collections::{HashSet, HashMap};

use crate::{problem::Problem, group::Label};

use super::event::EventHandler;
use itertools::Itertools;
use permutator::Combination;
use rand::prelude::SliceRandom;


fn automatic_upper_bound_smaller_parameters(orig : &Problem, max_labels : usize, branching : usize, max_steps : usize) -> Option<Vec<(Vec<Label>,Problem,Problem)>> {
    for max_steps in 1..=max_steps {
        for branching in 1..=branching {
            for max_labels in 1..=max_labels {
                println!("trying max labels {}, branching {}, max steps {}",max_labels,branching,max_steps);
                let r =  automatic_upper_bound(orig, max_labels, branching,max_steps);
                if r.is_some() {
                    return r;
                }
            }
        }
    }
    None
}

fn automatic_upper_bound(orig : &Problem, max_labels : usize, branching : usize, max_steps : usize) -> Option<Vec<(Vec<Label>,Problem,Problem)>> {
    let mut eh = EventHandler::null();
    let eh = &mut eh;
    
    let mut problems = vec![];

    problems.push(vec![(0,vec![],orig.clone(),orig.clone())]);

    let mut seen = HashSet::new();

    for i in 0..max_steps {
        //println!("i = {}, there are {} problems",i,problems[i].len());
        if problems[i].is_empty() {
            return None;
        }
        problems.push(vec![]);
        let p_i = problems[i].clone();
        for (idx,(_,_,_,p)) in p_i.iter().enumerate() {

            let p_s = p.to_string();
            if seen.contains(&p_s) {
                //println!("skipping already seen problem");
                continue;
            } else {
                seen.insert(p_s);
            }

            //println!("handling problem {}",idx+1);
            let mut np = p.speedup(eh);
            //println!("performed speedup");
            if seen.contains(&np.to_string()) {
                //println!("skipping already seen problem");
                continue;
            }
            np.discard_useless_stuff(false, eh);
            np.sort_active_by_strength();
            //np.compute_partial_diagram(eh);
            //println!("computed partial diagram");
            /*np.compute_triviality(eh);
            println!("computed triviality");
            if np.trivial_sets.as_ref().unwrap().len() > 0 {
                println!("found a {} rounds upper bound",i+1);
                return;
            }*/
            let (old, new) = np.split_labels_original_new();
            if old.len() > max_labels {
                continue;
            }
            
            let mut tochoose = max_labels - old.len();
            if tochoose > new.len() {
                tochoose = new.len();
            }

            let map : HashMap<_,_> = np.mapping_label_generators().into_iter().collect();

            let mut candidates = vec![];
            if tochoose == 0 {
                let mut tokeep = Vec::new();
                tokeep.extend(old.iter().cloned());
                candidates.push(tokeep);
            } else {
                //println!("going over candidates {} {}",tochoose, new.len());
                //new.sort_by_key(|l|map[l].len());
                //let new : Vec<_> = new.iter().cloned().take(tochoose+branching).collect();
                for choice in new.combination(tochoose) {
                    let mut tokeep = Vec::new();
                    tokeep.extend(old.iter().cloned());
                    tokeep.extend(choice.iter().map(|x|**x));
                    candidates.push(tokeep);
                }
                //println!("done");
            }

            candidates.sort_by_cached_key(|labels|{
                labels.iter().map(|l|map[l].len()).sum::<usize>()
            });
            //println!("built candidates");


            for candidate in candidates.into_iter().take(branching) {
                //println!("candidate {}",candidate.len());
                let tokeep = candidate.iter().cloned().collect();
                let mut hardened = np.harden_keep(&tokeep, true);
                //println!("hardened");
                hardened.discard_useless_stuff(false, eh);
                //println!("discarded useless, remaining labels are {}",hardened.labels().len());
                hardened.sort_active_by_strength();
                hardened.compute_triviality(eh);
                //println!("computed triviality");
                if hardened.trivial_sets.as_ref().unwrap().len() > 0 {
                    let mut sequence = vec![(candidate,np,hardened)];
                    let mut idx = idx;
                    for j in (0..=i).rev() {
                        let (index,l,s,p) = problems[j][idx].clone();
                        sequence.push((l,s,p));
                        idx = index;
                    }
                    return Some(sequence);
                }
                /*
                if hardened.passive.degree == Degree::Finite(2) {
                    hardened.compute_coloring_solvability(eh);
                    if hardened.coloring_sets.as_ref().unwrap().len() > 4 {
                        println!("found a {} rounds upper bound for {} coloring",i+1,hardened.coloring_sets.as_ref().unwrap().len());
                        return true;
                    }
                }
                 */
                problems[i+1].push((idx,candidate,np.clone(),hardened));
            }


        }
    }

    None
}


impl Problem{
    fn split_labels_original_new(&self) -> (Vec<Label>,Vec<Label>){
        let map_label_oldlabels = self.mapping_label_generators();

        let mut old_labels = vec![];
        let mut new_labels = vec![];

        for l in self.labels() {
            let is_old =  map_label_oldlabels.iter().filter(|(_,v)|v.contains(&l)).any(|(_,v)|v.len() == 1);
            if is_old {
                old_labels.push(l);
            } else {
                new_labels.push(l);
            }
        }

        (old_labels,new_labels)
    }

}

fn biregular_graph(d1 : usize, d2 : usize, sz : usize) -> (Vec<Vec<usize>>,Vec<Vec<usize>>){
    let n_left = sz * d2;
    let n_right = sz * d1;
    let n_edges = sz * d1 * d2;

    let mut left = vec![vec![]; n_left];
    let mut right = vec![vec![]; n_right];

    let mut edges_dest : Vec<_> = (0..n_edges).collect();
    edges_dest.shuffle(&mut rand::thread_rng());
    
    for (i,dest) in edges_dest.into_iter().enumerate() {
        let v1 = i / d1;
        let p1 = i % d1;
        let v2 = dest / d2;
        let p2 = dest % d2;
        left[v1].push((v2,p1));
        right[v2].push((v1,p2));
    }

    for v in left.iter_mut().chain(right.iter_mut()) {
        v.sort_by_key(|x|x.1);
    }
    
    let left = left.into_iter().map(|v|v.into_iter().map(|x|x.0).collect()).collect();
    let right = right.into_iter().map(|v|v.into_iter().map(|x|x.0).collect()).collect();

    (left,right)
}

fn biregular_graph_non_parallel(d1 : usize, d2 : usize, sz : usize) -> (Vec<Vec<usize>>,Vec<Vec<usize>>) {
    loop {
        let (left,right) = biregular_graph(d1, d2, sz);
        if left.iter().all(|v|v.iter().unique().count() == d1) && right.iter().any(|v|v.iter().unique().count() == d2) {
            return (left,right);
        } 
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use itertools::Itertools;

    use crate::{algorithms::event::EventHandler, problem::Problem};

    use super::{automatic_upper_bound, automatic_upper_bound_smaller_parameters, biregular_graph_non_parallel};

    #[test]
    fn autoub() {
        let mut eh = EventHandler::null();
        let eh = &mut eh;


/* 
        let mut p = Problem::from_string("                        (0->) (0->) (0->)
        (1<-)                   (1->) (1->)
        (2<-) (2<-)             (2->)
        (3<-) (3<-) (3<-)  

(1<-) (1<-) (1<-)
(1<-)(2<-) (1<-)(2<-)    (0->)(1->)(2->)
(1<-)(2<-)(3<-)          (1->)(2->) (1->)(2->)
                         (2->) (2->) (2->)").unwrap();

let mut p : Problem = Problem::from_string(
"(0->) (0->) (0->) (0->) (0->) 
(1<-)                   (1->) (1->) (1->) (1->)
(2<-) (2<-)                 (2->) (2->) (2->)
(3<-) (3<-) (3<-)                (3->) (3->)
(4<-) (4<-) (4<-) (4<-)                (4->)
(5<-) (5<-) (5<-) (5<-) (5<-)

(1<-) (1<-) (1<-) (1<-) (1<-)
(1<-)(2<-) (1<-)(2<-) (1<-)(2<-) (1<-)(2<-)                                                     (0->)(1->)(2->)(3->)(4->)
(1<-)(2<-)(3<-) (1<-)(2<-)(3<-) (1<-)(2<-)(3<-)                  (1->)(2->)(3->)(4->) (1->)(2->)(3->)(4->)
(1<-)(2<-)(3<-)(4<-) (1<-)(2<-)(3<-)(4<-)                (2->)(3->)(4->) (2->)(3->)(4->) (2->)(3->)(4->)
(1<-)(2<-)(3<-)(4<-)(5<-)                                                   (3->)(4->) (3->)(4->) (3->)(4->) (3->)(4->)
                                                                                                                         (4->) (4->) (4->) (4->) (4->)").unwrap();


let mut p = Problem::from_string("z z z
                                    A a a
                                    B B b
                                    C C C

                                    z A
                                    a BA
                                    b CBA").unwrap();
*/
    let mut p = Problem::from_string("                        (0->) (0->) (0->) (0->)
        (1<-)                   (1->) (1->) (1->)
        (2<-) (2<-)             (2->) (2->)
        (3<-) (3<-) (3<-)       (3->)
        (4<-) (4<-) (4<-) (4<-)

        (1<-) (1<-) (1<-) (1<-)
(1<-)(2<-) (1<-)(2<-) (1<-)(2<-)    (0->)(1->)(2->)(3->)
(1<-)(2<-)(3<-) (1<-)(2<-)(3<-)     (1->)(2->)(3->) (1->)(2->)(3->)
(1<-)(2<-)(3<-)(4<-)                (2->)(3->) (2->)(3->) (2->)(3->)
                                    (3->) (3->) (3->) (3->)").unwrap();

        p.discard_useless_stuff(false, eh);
        p.sort_active_by_strength();

        //automatic_upper_bound_smaller_parameters(&p,10,2, 16);
        if let Some(sequence) = automatic_upper_bound(&p, 10, 2,10) {
            println!("found a {} rounds upper bound",sequence.len()-1);
            for (i,(l,s,p)) in sequence.iter().rev().enumerate() {
                println!("{}",i);
                if i != 0 {
                    let mapping : HashMap<_,_> = s.mapping_label_text.iter().cloned().collect();
                    let labels = l.iter().map(|l|&mapping[l]).sorted().join(" ");
                    println!("perform speedup, then keep labels {}",labels);
                    println!("problem after speedup is\n{}",s);
                }
                println!("{}",p.to_string());
                println!("");

            }
        }

        let b = biregular_graph_non_parallel(5, 3, 2);
        println!("{:?}",b);

    }
}