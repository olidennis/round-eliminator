use std::collections::{HashSet, HashMap};

use crate::{problem::Problem, group::Label, serial::AutoOperation};

use super::event::EventHandler;
use itertools::Itertools;
use permutator::Combination;
use rand::prelude::SliceRandom;


impl Problem {
    pub fn autoub<F>(&self, max_labels : usize, branching : usize, mut max_steps : usize, allow_discard_old : bool, mut handler : F, eh: &mut EventHandler) where F : FnMut(usize, Vec<(AutoOperation,Problem)>) {
        let mut best_ub = usize::MAX;
        let (limited_by_branching, r) = automatic_upper_bound(self, max_labels, branching, max_steps, allow_discard_old, eh);
        if let Some((len,v)) = r {
            let mut sequence = vec![];
            sequence.push((AutoOperation::Initial,v[v.len()-1].2.clone()));
            for (kept_labels,after_speedup, after_harden) in v.into_iter().rev().skip(1) {
                sequence.push((AutoOperation::Speedup,after_speedup));
                sequence.push((AutoOperation::Harden(kept_labels),after_harden));
            }
            if len < best_ub {
                handler(len,sequence);
                best_ub = len;
                max_steps = best_ub - 1;
            }
        }
    }

    pub fn autoautoub<F>(&self, allow_discard_old : bool, mut handler : F, eh: &mut EventHandler) where F : FnMut(usize, Vec<(AutoOperation,Problem)>) {
        let mut max_steps = usize::MAX;
        for i in 1.. {
            self.autoub(i,i,std::cmp::min(2*i,max_steps),allow_discard_old,|len,seq|{
                if len < max_steps {
                    max_steps = len-1;
                    handler(len,seq);
                }
            },eh);
        }
    }
}

fn automatic_upper_bound_smaller_parameters(orig : &Problem, max_labels : usize, branching : usize, max_steps : usize, eh: &mut EventHandler) -> Option<Vec<(Vec<Label>,Problem,Problem)>> {
    for max_steps in 1..=max_steps {
        for branching in 1..=branching {
            for max_labels in 1..=max_labels {
                println!("trying max labels {}, branching {}, max steps {}",max_labels,branching,max_steps);
                let (limited_by_branching,r) =  automatic_upper_bound(orig, max_labels, branching,max_steps, false, eh);
                if r.is_some() {
                    return Some(r.unwrap().1);
                }
            }
        }
    }
    None
}

fn automatic_upper_bound(orig : &Problem, max_labels : usize, branching : usize, max_steps : usize,  allow_discard_old : bool, eh: &mut EventHandler) -> (bool,Option<(usize,Vec<(Vec<Label>,Problem,Problem)>)>) {
    //let mut eh = EventHandler::null();
    //let eh = &mut eh;
    
    let mut problems = vec![];

    problems.push(vec![(0,orig.labels(),orig.clone(),orig.clone())]);

    let mut seen = HashSet::new();
    
    let mut limited_by_branching = false;

    for i in 0..max_steps {
        //println!("i = {}, there are {} problems",i,problems[i].len());
        if problems[i].is_empty() {
            return (limited_by_branching,None);
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

            let (old, new) = if !allow_discard_old {
                np.split_labels_original_new()
            } else {
                (vec![],np.labels())
            };

            if old.len() > max_labels {
                //println!("old len > max labels, skipping");
                continue;
            }
            
            let mut tochoose = max_labels - old.len();
            //println!("need to chose {} labels ({} {})", tochoose, max_labels, old.len());
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

            if candidates.len() > branching {
                //println!("got limited by branching: {} {}",candidates.len(),branching);
                limited_by_branching = true;
            }

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
                    return (limited_by_branching,Some((i+1,sequence)));
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

    (limited_by_branching,None)
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

    use std::{collections::HashMap, fs::File};

    use itertools::Itertools;

    use std::io::Write;

    use crate::{algorithms::event::EventHandler, problem::Problem, group::{Group, GroupType}, line::Line, part::Part};

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


    /*let mut p = Problem::from_string("A B C
    A F B
    A H H
    I I I
    
    ABC I
BC H
C F").unwrap();*/

/*

M U^2
H^2 U
P^2 Q
P^3

M PU^2
HU U^2
HQ^2 PU

this gives an algorithm, but by changing P^2 Q to Q P^2 does not
if let Some(sequence) = automatic_upper_bound_smaller_parameters(&p,8,4, 12) {

 */

let mut p = Problem::from_string("M U^4
H^2 U^3
P^4 Q
P^5

M PU^4
HU U^4
HQ^2 PU^3").unwrap();

let mut p = Problem::from_string("(1s)^3 U
(2s) (1s) U^2
(3s) U^3
P^4
P^3 (1u)
P^2 (1u)^2
P^3 (2u)

(3s) PU^3
(2s)(2u) (1s)(1u) PU^2
(1s)(1u)^3 PU
U^4
(2s)(1s) U^3
(1s)^2 U^2").unwrap();

        p.discard_useless_stuff(false, eh);
        p.sort_active_by_strength();

        if let Some(sequence) = automatic_upper_bound_smaller_parameters(&p,10,12, 5) {
        //if let Some(sequence) = automatic_upper_bound(&p, 10, 2,10) {
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
            return;

            'outer: loop {
                let d1 = sequence[0].2.active.finite_degree();
                let d2 = sequence[0].2.passive.finite_degree();

                let b = biregular_graph_non_parallel(d1,d2, 8);
                let n = b.0.len();
                //println!("{:?}",b);
                let mut state = HashMap::new();
                let trivial_set = Group(sequence[0].2.trivial_sets.as_ref().unwrap()[0].clone()); 
                let trivial_line = sequence[0].1.active.lines.iter().find(|line|{
                    let set = line.line_set();
                    trivial_set.is_superset(&set)
                }).unwrap();

                let mut starting_labels = vec![];
                for part in &trivial_line.parts {
                    for _ in 0..part.gtype.value() {
                        starting_labels.push(part.group[0]);
                    }
                }

                for (u,neighbors) in b.0.iter().enumerate() {
                    for (j,&v) in neighbors.iter().enumerate() {
                        state.insert((u,v),vec![starting_labels[j]]);
                    }
                }

                let mut file = File::create("graph.txt").unwrap();

                for (u,v) in state.keys() {
                    writeln!(file,"ae {} {}",u,n+v).unwrap();
                }
                for i in 0..n {
                    writeln!(file,"ln {} \"\"",i).unwrap();
                    writeln!(file,"hn {}",i).unwrap();
                }
                for i in n..2*n {
                    writeln!(file,"ln {} \"\"",i).unwrap();
                }
                //println!("ns");

                for i in 0..sequence.len() {
                    //let mapping : HashMap<_,_> = sequence[i].1.mapping_label_text.iter().cloned().collect();
                    let colors = ["darkgreen","darkblue","maroon3","red","gold","lawngreen","aqua","fuchsia","cornflowerblue","peachpuff"];
                    let labelidx : HashMap<_,_> = sequence[i].0.iter().enumerate().map(|(a,b)|(b,a)).collect();
                    for ((u,v),labels) in state.iter() {
                        //writeln!(file,"le {} {} \"{: >4}\"",u,n+v,mapping[&labels[0]]).unwrap();
                        writeln!(file,"he {} {} \"{}\"",u,n+v,colors[labelidx[&labels[0]]]).unwrap();
                    }
                    for i in 0..n {
                        writeln!(file,"hn {}",i).unwrap();
                    }
                    writeln!(file,"ns").unwrap();

                    if i == sequence.len() - 1 {
                        break;
                    }

                    let map_label_oldlabels : HashMap<_,_> = sequence[i].1.mapping_label_oldlabels.as_ref().unwrap().iter().cloned().collect();
                    for (_,v) in state.iter_mut() {
                        *v = map_label_oldlabels[&v[0]].clone();
                    }
                    
                    /*let mapping : HashMap<_,_> = sequence[i+1].1.mapping_label_text.iter().cloned().collect();
                    for ((u,v),labels) in state.iter() {
                        writeln!(file,"le {} {} \"{: >4}\"",u,n+v,labels.iter().map(|x|&mapping[x]).join(",")).unwrap();
                    }
                    for i in 0..n {
                        writeln!(file,"hn {}",i).unwrap();
                    }
                    writeln!(file,"ns").unwrap();*/
                    
                    let order = i%2 == 0;
                    for (v,neighbors) in (if order {&b.1} else {&b.0}).iter().enumerate() {
                        let configuration = Line{ parts : neighbors.iter().map(|&u|{
                            Part{group : Group(state[& if order {(u,v)} else {(v,u)}].clone()), gtype : GroupType::Many(1)}
                        }).collect()};
                        let choice = sequence[i+1].2.active.lines.iter().filter_map(|line|configuration.pick_existing_choice(line)).next().unwrap();
                        for (j,&u) in neighbors.iter().enumerate() {
                            state.insert(if order {(u,v)} else {(v,u)},vec![choice[j]]);
                        }
                    }
                }

                for i in 0..n {
                    writeln!(file,"hn {}",i).unwrap();
                }
                let mapping : HashMap<_,_> = sequence[sequence.len()-1].1.mapping_label_text.iter().cloned().collect();
                for ((u,v),labels) in state.iter() {
                    let out = mapping[&labels[0]].contains("->");
                    writeln!(file,"he {} {} \"{}\"",u,n+v,if out {"red"} else {"black"}).unwrap();
                }
                
                for (v,neighbors) in b.1.iter().enumerate() {
                    if neighbors.iter().all(|&u|mapping[&state[&(u,v)][0]].contains("->")) {
                        break 'outer;
                    }
                    if neighbors.iter().all(|&u|mapping[&state[&(u,v)][0]].contains("<-")) {
                        break 'outer;
                    }
                }
                for (u,neighbors) in b.0.iter().enumerate() {
                    if neighbors.iter().all(|&v|mapping[&state[&(u,v)][0]].contains("->")) {
                        break 'outer;
                    }
                    if neighbors.iter().all(|&v|mapping[&state[&(u,v)][0]].contains("<-")) {
                        break 'outer;
                    }
                }
                //if state.iter().any(|(_,v)|mapping[&v[0]].contains("0->")) {
                //    break;
                //}
            }
        }



    }
}