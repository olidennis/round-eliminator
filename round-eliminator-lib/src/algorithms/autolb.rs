use std::collections::{HashSet, HashMap};

use crate::{problem::Problem, group::Label, serial::AutoOperation, line::Degree};

use super::event::EventHandler;
use itertools::Itertools;
use permutator::Combination;


impl Problem {
    pub fn autolb<F>(&self, max_labels : usize, branching : usize, min_steps : usize, max_steps : usize, coloring : Option<usize>, coloring_passive : Option<usize>, mut handler : F, eh: &mut EventHandler)  -> bool  where F : FnMut(usize, Vec<(AutoOperation,Problem)>){
        let mut problems = vec![(vec![],self.clone(),self.clone(),self.to_string())];
        let mut best = usize::MAX;
        let mut seen = HashMap::new();
    
        automatic_lower_bound_rec(&mut seen, &mut problems, &mut best, max_labels, branching, min_steps, max_steps, coloring, coloring_passive, &mut handler, eh);

        return best >= max_steps;
    }


    pub fn autoautolb<F>(&self, b_max_labels : bool, max_labels : usize, b_branching : bool, branching : usize, b_max_steps : bool, max_steps : usize, coloring : Option<usize>, coloring_passive : Option<usize>, mut handler : F, eh: &mut EventHandler) where F : FnMut(usize, Vec<(AutoOperation,Problem)>) {
        if b_max_labels && b_branching && b_max_steps {
            self.autolb(max_labels, branching, 1, max_steps, coloring, coloring_passive, handler, eh);
            return;
        }

        let mut min_steps = 1;
        for i in 1.. {
            let i_max_labels = if b_max_labels { max_labels } else { self.labels().len() + i };
            let i_branching = if b_branching { branching } else { i };
            let max_steps = if b_max_steps { max_steps } else { 15 };

            if self.autolb(i_max_labels, i_branching, min_steps, max_steps, coloring, coloring_passive, |len,seq|{
                if len >= min_steps {
                    min_steps = len+1;
                    handler(len,seq);
                }
            },eh) {
                return;
            }
        }
    }
}

fn best_merges(np : &Problem, branching : usize, max_labels : usize, coloring : Option<usize>, eh: &mut EventHandler) -> Vec<Vec<(Label,Label)>> {
    if np.mapping_label_oldlabels.is_none() {
        return unimplemented!();
    }
    
    let (old, new) = np.split_labels_original_new();
    let old : HashSet<_> = old.into_iter().collect();
    let new : HashSet<_> = new.into_iter().collect();
    let labels = np.labels();
    let map : HashMap<_,_> = np.mapping_label_generators().into_iter().collect();

    let mut pair_weights : Vec<_> = labels.iter().flat_map(|&l1|{
        let new = &new;
        let map = &map;
        labels.iter().map(move |&l2|{
            let both_new = new.contains(&l1) && new.contains(&l2);
            let one_new = new.contains(&l1) || new.contains(&l2);
            let gen1 : HashSet<_> = map[&l1].iter().cloned().collect();
            let gen2 : HashSet<_> = map[&l2].iter().cloned().collect();
            let d1 : HashSet<_> = gen1.difference(&gen2).cloned().collect();
            let d2 : HashSet<_> = gen2.difference(&gen1).cloned().collect();
            let distance = d1.union(&d2).count();
            let weight = match (both_new,one_new) {
                (true,_) => { distance },
                (_, true) => { distance + 2 },
                _ => { distance + 100 }
            };
            ((l1,l2),weight)
        })
    }).filter(|((l1,l2),w)|l1 < l2).collect();
    pair_weights.sort_by_key(|(_,w)|*w);

    let mut candidates = HashSet::new();

    let max_labels = std::cmp::min(max_labels,labels.len());
    let mut min_labels = old.len();
    if min_labels > max_labels {
        min_labels = max_labels;
    }

    for max_labels_i in min_labels..=max_labels {
        let to_merge = labels.len() - max_labels_i;
        if to_merge == 0 {
            candidates.insert(vec![]);
        } else {
            let mut grouped_pair_weights = HashMap::new();
            let mut next_pair = pair_weights.iter();
            let target_candidates = 10*branching;
            'outer: while candidates.len() < target_candidates {
                if let Some(((from,to),w)) = next_pair.next() {
                    if grouped_pair_weights.entry(from).or_insert(vec![]).len() < branching {
                        grouped_pair_weights.entry(from).or_insert(vec![]).push((to,w));
                        if grouped_pair_weights.len() >= to_merge {
                            let v : Vec<_> = grouped_pair_weights.keys().collect();
                            //println!("calling combination {} {}",v.len(),to_merge);
                            for froms in v.combination(to_merge) {
                                for choice in froms.into_iter().map(|&&&from|grouped_pair_weights[&from].iter().map(move |(to,w)|(from,to,w))).multi_cartesian_product() {
                                    let choice : Vec<_> = choice.into_iter().map(|(from,&&to,&&w)|((from,to),w)).collect();
                                    candidates.insert(choice);
                                    if candidates.len() >= target_candidates {
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
    
    let mut candidates : Vec<_> = candidates.into_iter().collect();
    candidates.sort_by_cached_key(|choice|{
        choice.iter().map(|p|p.1).sum::<usize>() as isize - choice.len() as isize
    });

    candidates.into_iter().take(branching).map(|v|v.into_iter().map(|(p,_)|p).collect()).collect()
}

fn automatic_lower_bound_rec<F>(seen : &mut HashMap<String,usize>, problems : &mut Vec<(Vec<(Label,Label)>,Problem,Problem,String)>, best : &mut usize, max_labels : usize, branching : usize, min_steps : usize, max_steps : usize, coloring : Option<usize>, coloring_passive : Option<usize>, handler : &mut F, eh: &mut EventHandler) where F : FnMut(usize, Vec<(AutoOperation,Problem)>) {

    let mut send_sequence = |len : usize, problems : &Vec<(Vec<(Label,Label)>,Problem,Problem,String)>|{
        *best = len + 1;
        let mut sequence = vec![];
        sequence.push((AutoOperation::Initial,problems[0].2.clone()));
        for (merges,after_speedup, after_merge,_) in problems.iter().skip(1) {
            sequence.push((AutoOperation::Speedup,after_speedup.clone()));
            if !merges.is_empty() {
                sequence.push((AutoOperation::Merge(merges.clone(),after_speedup.clone()),after_merge.clone()));
            }
        }
        handler(len,sequence);
    };

    {
        let p_s = &problems.last().unwrap().3;
        if problems.len() >=2 {
            for i in (0..problems.len()-2).rev() {
                if &problems[i].3 == p_s {
                    send_sequence(999, problems);
                    return;
                }
            }
        }
        if seen.contains_key(p_s) && seen[p_s] >= problems.len() {
            return;
        }
        if problems.len() < 6 && seen.len() < 100_000 {
            seen.insert(p_s.clone(),problems.len());
        }

        let p = &mut problems.last_mut().unwrap().2;   

        if p.trivial_sets.is_none() {
            p.compute_triviality(eh);
        }
        if coloring.is_some() && p.coloring_sets.is_none() {
            p.compute_coloring_solvability(eh);
        }
    }

    let p = &problems.last().unwrap().2;  

    if problems.len() > max_steps || p.trivial_sets.as_ref().unwrap().len() > 0 || (coloring.is_some() && p.coloring_sets.is_some() && p.coloring_sets.as_ref().unwrap_or(&vec![]).len() >= coloring.unwrap()) {
        send_sequence(problems.len()-1, problems);
        return;
    }
    
    let (coloring,coloring_passive) = (coloring_passive,coloring);
    let mut np = p.speedup(eh);
    np.discard_useless_stuff(false, eh);
    np.sort_active_by_strength();
    if coloring.is_some() {
        np.compute_coloring_solvability(eh);
    }

    let candidates = best_merges(&np, branching, max_labels, coloring, eh);

    for candidate in candidates.into_iter().take(branching) {
        let merges : Vec<(Label,Label)> = candidate;
        let mut merged = np.relax_many_merges(&merges);
        merged.discard_useless_stuff(false, eh);
        merged.sort_active_by_strength();
        merged.compute_triviality(eh);
        if coloring.is_some() {
            merged.compute_coloring_solvability(eh);
        }
        let m_s = merged.to_string();

        problems.push((merges,np.clone(),merged.clone(),m_s));
        automatic_lower_bound_rec(seen, problems, best, max_labels, branching, min_steps, max_steps, coloring, coloring_passive, handler, eh);
        problems.pop();
        if *best > max_steps {
            return;
        }
    }

    
}

