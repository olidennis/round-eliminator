use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering, AtomicUsize};
use dashmap::DashMap as CHashMap;
use parking_lot::RwLock;
use streaming_iterator::StreamingIterator;
use std::time::Instant;

use crate::{
    algorithms::multisets_pairing::Pairings,
    constraint::Constraint,
    group::{Group, GroupType},
    line::Line,
    part::Part,
};

use super::event::EventHandler;

impl Constraint {

    
    pub fn maximize_custom<FS,FU,FI>(
        &mut self,
        eh: &mut EventHandler,
        allow_empty : bool,
        track_unions : bool,
        tracking : Option<&CHashMap<Line, (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>)>>,
        f_is_superset : FS,
        f_union : FU,
        f_intersection : FI
    ) where FS : Fn(&Group,&Group) -> bool + Copy + Send + Sync, FU : Fn(&Group,&Group) -> Group + Copy + Send + Sync, FI : Fn(&Group,&Group) -> Group + Copy + Send + Sync {
 
        if self.is_maximized || self.lines.is_empty() {
            self.is_maximized = true;
            return;
        }

        let becomes_star = 100;

        let seen = CHashMap::new();
        let mut seen_pairs = CHashMap::<(usize,usize),()>::new();
        let next_id = AtomicUsize::new(1);

        let lines = std::mem::take(&mut self.lines);
        let empty = self.clone();
        for mut line in lines {
            line.normalize();
            seen.insert(line.clone(),next_id.fetch_add(1,Ordering::SeqCst));
            self.add_line_and_discard_non_maximal_with_custom_supersets(line, Some(f_is_superset));
        }

        loop {
            let lines = &self.lines;
            let useful_ids : HashSet<usize> = lines.iter().map(|line|*seen.get(line).unwrap()).collect();
            seen_pairs = seen_pairs.into_iter().filter(|((p1,p2),_)| useful_ids.contains(p1) && useful_ids.contains(p2)).collect();

            let without_one = without_one(lines);

            let mut f_newconstraint = ||{
                let mut newconstraint = self.clone();
                for i in 0..lines.len() {
                    let mut candidates2 = empty.clone();
                    let len = lines.len();
                    for j in 0..=i {
                        eh.notify("combining line pairs", (2. * (i * (i+1)/2 + j) as f64).sqrt() as usize, len);

                        let id1 = *seen.get(&lines[i]).unwrap();
                        let id2 = *seen.get(&lines[j]).unwrap();
                        let pair = (id1,id2);
                        if seen_pairs.contains_key(&pair)
                            || seen_pairs.contains_key(&(pair.1.clone(), pair.0.clone()))
                        {
                            continue;
                        }
                        seen_pairs.insert(pair,());

                        let (candidates,_,how) = combine_lines_custom(
                            &lines[i],
                            &lines[j],
                            &without_one[i],
                            &without_one[j],
                            &seen,
                            Some(&next_id),
                            becomes_star,
                            allow_empty,
                            track_unions,
                            tracking.is_some(),
                            f_is_superset, f_union, f_intersection
                        );
                        if let Some(tracking) = tracking {
                            for (a,b) in how.into_iter() {
                                tracking.insert(a,b);
                            }
                        }
                        for newline in candidates {
                            candidates2.add_line_and_discard_non_maximal_with_custom_supersets(newline,Some(f_is_superset));
                        }
                    }

                    for newline in candidates2.lines {
                        newconstraint.add_line_and_discard_non_maximal_with_custom_supersets(newline,Some(f_is_superset));
                    }
                }
                newconstraint
            };

            #[cfg(target_arch = "wasm32")]
            let newconstraint = f_newconstraint();

            #[cfg(not(target_arch = "wasm32"))]
            let newconstraint = {
                let n_workers = if let Ok(val) = std::env::var("RE_NUM_THREADS") {
                    val.parse::<usize>().unwrap()
                } else {
                    num_cpus::get()
                };

                if n_workers == 0 {
                    f_newconstraint()
                } else {

                    let v = append_only_vec::AppendOnlyVec::<_>::new();
                    for line in self.lines.iter() {
                        v.push((AtomicBool::new(false),line.clone()));
                    }
                    let newconstraint = std::sync::Arc::new(RwLock::new(v));

                    crossbeam::scope(|s| {
                        let (in_tx, in_rx) =  crossbeam_channel::unbounded();
                        let (out_tx, out_rx) =  crossbeam_channel::unbounded();
                        let (progress_tx, progress_rx) : (crossbeam_channel::Sender<()>,crossbeam_channel::Receiver<()>)  =  crossbeam_channel::unbounded();

                        let seen_pairs = &seen_pairs;
                        let seen = &seen;
                        let lines = &lines;
                        let without_one = &without_one;
        

                        for thread_num in 0..n_workers {
                            let (in_tx, in_rx) : (crossbeam_channel::Sender<(usize,usize)>,crossbeam_channel::Receiver<(usize,usize)>) = (in_tx.clone(), in_rx.clone());
                            let (out_tx, out_rx) = (out_tx.clone(), out_rx.clone());
                            let next_id = &next_id;
                            s.spawn(move |_|{
                                while let Ok((i,j)) = in_rx.recv() {
                                    let id1 = *seen.get(&lines[i]).unwrap();
                                    let id2 = *seen.get(&lines[j]).unwrap();
                                    let pair = (id1,id2);
                                    //let pair = (lines[i].clone(), lines[j].clone());
                                    if seen_pairs.contains_key(&pair)
                                        || seen_pairs.contains_key(&(pair.1.clone(), pair.0.clone()))
                                    {
                                        out_tx.send(vec![]).unwrap();
                                        continue;
                                    }
                                    seen_pairs.insert(pair,());

                                    let (candidates,_,how) = combine_lines_custom(
                                        &lines[i],
                                        &lines[j],
                                        &without_one[i],
                                        &without_one[j],
                                        &seen,
                                        Some(&next_id),
                                        becomes_star,
                                        allow_empty,
                                        track_unions,
                                        tracking.is_some(),
                                        f_is_superset, f_union, f_intersection
                                    );
                                    if let Some(tracking) = tracking {
                                        for (a,b) in how.into_iter() {
                                            tracking.insert(a,b);
                                        }
                                    }
                                    out_tx.send(candidates).unwrap();
                                }
                                //println!("done producing new lines");
                            });
                        }

                        //println!("number of lines: {}",lines.len());
                        for i in 0..lines.len() {
                            for j in 0..=i {
                                in_tx.send((i,j)).unwrap();
                            }
                        }
                        drop(in_tx);

                        for thread_num in 0..n_workers {
                            let (out_tx, out_rx) = (out_tx.clone(), out_rx.clone());
                            let (progress_tx, progress_rx) = (progress_tx.clone(), progress_rx.clone());
                            let newconstraint = newconstraint.clone();
                            s.spawn(move |_|{
                                let mut times = 0;
                                while let Ok(candidates) = out_rx.recv() {
                                    times += 1;
                                    if thread_num == 0 && times % 128 == 0 {
                                        //let (bad,tot) = {
                                        //    let newconstraint = newconstraint.read();
                                        //    let bad = newconstraint.iter().filter(|(removed,_)|removed.load(Ordering::SeqCst)).count();
                                        //    let tot = newconstraint.len();
                                        //    (bad,tot)
                                        //};
                                        //if bad > 10 && 4*bad > tot {
                                            let mut newconstraint = newconstraint.write();
                                            let mut cleaned = append_only_vec::AppendOnlyVec::<_>::new();
                                            for (b,line) in newconstraint.iter().filter(|(removed,_)|!removed.load(Ordering::SeqCst)) {
                                                cleaned.push((AtomicBool::new(false),line.clone()));
                                            }
                                            *newconstraint = cleaned;
                                        //}
                                    }

                                    let newconstraint = newconstraint.read();

                                    for newline in candidates {
                                        let (is_not_included,checked_len) = 'outer : {
                                            let len = newconstraint.len();
                                            (newconstraint.iter().take(len).rev()
                                                .filter(|(removed,_)|!removed.load(Ordering::Relaxed))
                                                .all(|(_,oldline)| !oldline.includes_with_custom_supersets(&newline, Some(f_is_superset))),
                                            len)
                                        };
                                        if is_not_included {
                                            let added_pos = newconstraint.push((AtomicBool::new(false),newline.clone()));
                                            if newconstraint.iter().skip(checked_len).take(added_pos-checked_len)
                                                    .any(|(_,oldline)| oldline.includes_with_custom_supersets(&newline, Some(f_is_superset))) {
                                                newconstraint[added_pos].0.store(true,Ordering::Relaxed);
                                            } else {
                                                for (removed,oldline) in newconstraint.iter().take(added_pos) {
                                                    if !removed.load(Ordering::Relaxed) {
                                                        if newline.includes_with_custom_supersets(oldline, Some(f_is_superset)) {
                                                            removed.store(true,Ordering::Relaxed);
                                                        }
                                                    }
                                                }
                                            }

                                        }
                                    }
                                    progress_tx.send(()).unwrap();
                                }
                            });
                        }

                        //let now = std::time::Instant::now();
                        let len = lines.len();
                        let total = len * (len+1)/2;
                        let mut last_notify = Instant::now();
                        for received in 0..total {
                            progress_rx.recv().unwrap();
                            if last_notify.elapsed().as_millis() > 100 {
                                eh.notify("combining line pairs", (2. *received as f64).sqrt() as usize, len);
                                last_notify = Instant::now();
                            }
                        }
                        //println!("It took {}s",now.elapsed().as_secs());

                    }).unwrap();

                    let newconstraint = newconstraint.read();
                    //let c1 = newconstraint.iter().filter(|(removed,_)|!removed.load(Ordering::SeqCst)).count();
                    //let c2 = newconstraint.iter().filter(|(removed,_)|removed.load(Ordering::SeqCst)).count();
                    //println!("bad {}, good {}",c2,c1);
                    Constraint{ lines: newconstraint.iter().filter(|(removed,_)|!removed.load(Ordering::SeqCst)).map(|(_,line)|line.clone()).collect(), is_maximized: false, degree: self.degree }
                }
            };

            //println!("seen elements: {}, seen_pairs elements: {}",seen.len(),seen_pairs.len());

            if &newconstraint == self {
                break;
            }
            *self = newconstraint;
        }

        self.is_maximized = true;
    }

    pub fn maximize(&mut self, eh: &mut EventHandler) {
        let f_is_superset = |g1 : &Group ,g2 : &Group |{ g1.is_superset(g2) };
        let f_union = |g1 : &Group ,g2 : &Group |{ g1.union(g2) };
        let f_intersection = |g1 : &Group ,g2 : &Group |{ g1.intersection(g2) };
        self.maximize_custom(eh,false,false,None,f_is_superset,f_union,f_intersection);
    }
}


pub fn without_one(lines : &Vec<Line>) -> Vec<Vec<Line>> {
    let mut without_one = vec![];
    for line in lines {
        let mut current = vec![];
        for (i, part) in line.parts.iter().enumerate() {
            let before = &line.parts[0..i];
            let mid = match part.gtype {
                GroupType::Many(0) => panic!("group of size 0, should not happen"),
                /*GroupType::ONE => Some(Part { //this is needed to not break the part that keeps track of the operations
                    group: part.group.clone(),
                    gtype: GroupType::Many(0),
                }),*/
                GroupType::Many(x) => Some(Part {
                    group: part.group.clone(),
                    gtype: GroupType::Many(x - 1),
                }),
                GroupType::Star => Some(part.clone()),
            };
            let after = &line.parts[i + 1..];
            let line = Line {
                parts: before
                    .iter()
                    .cloned()
                    .chain(mid.iter().cloned())
                    .chain(after.iter().cloned())
                    .collect(),
            };
            current.push(line);
        }
        without_one.push(current);
    }
    without_one
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, Hash)]
pub enum Operation {
    Union,
    Intersection,
}

#[inline(never)]
#[allow(clippy::needless_range_loop)]
fn intersections<FS,FI>(union: (usize,usize,Operation,Part), c1: &Line, c2: &Line, allow_empty : bool, f_is_superset : FS, f_intersection : FI) -> Vec<Vec<(usize,usize,Operation,Part)>> where FS : Fn(&Group,&Group) -> bool, FI : Fn(&Group,&Group) -> Group {
    let t1 = c1.degree_without_star();
    let t2 = c2.degree_without_star();
    let d = if c1.has_star() { t1 + t2 } else { t1 };
    let star1 = d - t1;
    let star2 = d - t2;

    let line_to_counts = |line: &Line, starvalue : usize| -> Vec<usize> {
        line.parts
            .iter()
            .map(|part| match part.gtype {
                //GroupType::ONE => 1,
                GroupType::Many(x) => x as usize,
                GroupType::Star => starvalue,
            })
            .collect()
    };

    let star_intersection = if let (Some(s1), Some(s2)) = (c1.get_star(), c2.get_star()) {
        let group = f_intersection(&s1.1.group,&s2.1.group);
        if !allow_empty && group.is_empty() {
            return vec![]; 
        }
        Some((s1.0,s2.0,Operation::Intersection,Part {
            gtype: GroupType::Star,
            group,
        }))
    } else {
        None
    };

    let mut result = vec![];

    let v1 = line_to_counts(c1, star1);
    let v2 = line_to_counts(c2, star2);

    let mut pairings = Pairings::new(v1, v2);

    let mut oldbad: Option<(usize, usize, usize, usize)> = None;

    'outer: while let Some(pairing) = pairings.next() {
        if let Some((i1, i2, j1, j2)) = oldbad {
            if pairing[i1][j1] != 0 && pairing[i2][j2] != 0 {
                continue 'outer;
            }
        }
        for i1 in 0..c1.parts.len() {
            for j1 in 0..c2.parts.len() {
                if pairing[i1][j1] != 0 {
                    for i2 in i1 + 1..c1.parts.len() {
                        for j2 in 0..c2.parts.len() {
                            if pairing[i2][j2] != 0 {
                                let u1 = f_intersection(&c1.parts[i1].group,&c2.parts[j1].group);
                                let u2 = f_intersection(&c1.parts[i2].group,&c2.parts[j2].group);
                                let u3 = f_intersection(&c1.parts[i1].group,&c2.parts[j2].group);
                                let u4 = f_intersection(&c1.parts[i2].group,&c2.parts[j1].group); 

                                if (f_is_superset(&u4,&u1)
                                    && f_is_superset(&u3,&u2) 
                                    && (u1 != u4 || u2 != u3))
                                    || (f_is_superset(&u3,&u1) 
                                        && f_is_superset(&u4,&u2)
                                        && (u1 != u3 || u2 != u4))
                                {
                                    oldbad = Some((i1, i2, j1, j2));
                                    continue 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut parts = vec![union.clone()];
        for (i, pa) in c1.parts.iter().enumerate() {
            for (j, pb) in c2.parts.iter().enumerate() {
                if pairing[i][j] > 0 {
                    let value = GroupType::Many(pairing[i][j] as crate::group::Exponent);
                    let intersection = f_intersection(&pa.group,&pb.group); 
                    if !allow_empty && intersection.is_empty() { 
                        continue 'outer;
                    }
                    parts.push((i,j,Operation::Intersection,Part {
                        gtype: value,
                        group: intersection,
                    }));
                }
            }
        }
        if let Some(star_intersection) = &star_intersection {
            parts.push(star_intersection.clone());
        }
        result.push(parts);
    }

    result
}

#[inline(never)]
fn good_unions<FS,FU>(l1: &Line, l2: &Line, f_is_superset : FS, f_union : FU) -> HashMap<Group, Vec<(usize, usize)>> where FS : Fn(&Group,&Group) -> bool, FU : Fn(&Group,&Group) -> Group {
    let s1: Vec<_> = l1.parts.iter().map(|part| &part.group).collect();
    let s2: Vec<_> = l2.parts.iter().map(|part| &part.group).collect();

    let mut unions = HashMap::new();

    for (i, x) in s1.iter().enumerate() {
        for (j, y) in s2.iter().enumerate() {
            if f_is_superset(x,y) || f_is_superset(y,x) { 
                continue;
            }

            let union = f_union(x,y);
            let same_union: &mut Vec<(usize, usize)> = unions.entry(union).or_default();

            let len = same_union.len();
            same_union.retain(|&(xc, yc)| !(f_is_superset(s1[xc],x) && f_is_superset(s2[yc],y))); 
            if same_union.len() != len
                || same_union
                    .iter()
                    .all(|&(xc, yc)| !(f_is_superset(x,&s1[xc]) && f_is_superset(y,&s2[yc]))) 
            {
                same_union.push((i, j));
            }
        }
    }

    unions
}

#[inline(never)]
pub fn combine_lines_custom<FS,FU,FI>(
    l1: &Line,
    l2: &Line,
    l1_without_one: &[Line],
    l2_without_one: &[Line],
    seen: &CHashMap<Line,usize>,
    line_id: Option<&AtomicUsize>,
    becomes_star: usize,
    allow_empty : bool,
    track_unions : bool,
    track_all : bool,
    f_is_superset : FS,
    f_union : FU,
    f_intersection : FI
) -> (Vec<Line>,HashMap<Line,HashSet<(usize,usize)>>,HashMap<Line, (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>)>) where FS : Fn(&Group,&Group) -> bool + Copy + Sync, FU : Fn(&Group,&Group) -> Group + Copy, FI : Fn(&Group,&Group) -> Group + Copy {
    let mut result = Constraint {
        lines: vec![],
        is_maximized: false,
        degree: l1.degree(),
    };
    let unions = good_unions(l1, l2, f_is_superset, f_union);

    let mut line_to_unions : HashMap<Line,HashSet<_>> = HashMap::new();
    let mut line_to_allinfo : HashMap<Line,_> = HashMap::new();

    for (union, v) in unions {
        let union = Part {
            group: union,
            gtype: GroupType::ONE,
        };
        for (x, y) in v {
            let c1 = &l1_without_one[x];
            let c2 = &l2_without_one[y];
            let lines = intersections((x,y,Operation::Union,union.clone()), c1, c2, allow_empty, f_is_superset, f_intersection); 
            for newline_and_how in lines {
                let mut newline = Line{ parts : newline_and_how.iter().map(|x|x.3.clone()).collect() };
                if newline.has_star() {
                    for parts in newline.parts.iter_mut() {
                        if let GroupType::Many(x) = parts.gtype {
                            if x as usize >= becomes_star {
                                parts.gtype = GroupType::Star;
                            }
                        }
                    }
                }
                let mut normalization_map = vec![];
                let before_normalizing = newline.clone();
                newline.normalize();

                if track_all {
                    for part in &newline.parts {
                        let old_positions : Vec<_> = before_normalizing.parts.iter().enumerate().filter(|(_,p)|p.group == part.group).map(|x|x.0).collect();
                        normalization_map.push(old_positions);
                    }
                }
                if !seen.contains_key(&newline) {
                    seen.insert(newline.clone(),if let Some(line_id) = line_id { line_id.fetch_add(1, Ordering::SeqCst) } else {0});
                    if track_unions {
                        line_to_unions.entry(newline.clone()).or_default().insert((x,y));
                    }
                    if track_all {
                        line_to_allinfo.insert(newline.clone(),(l1.clone(),l2.clone(),before_normalizing, normalization_map,newline_and_how.into_iter().map(|x|(x.0,x.1,x.2)).collect::<Vec<_>>()));
                    }
                    //result.lines.push(newline);
                    result.add_line_and_discard_non_maximal_with_custom_supersets(newline, Some(f_is_superset));
                }
            }
        }
    }
    (result.lines,line_to_unions,line_to_allinfo)
}


fn combine_lines(
    l1: &Line,
    l2: &Line,
    l1_without_one: &[Line],
    l2_without_one: &[Line],
    seen: &CHashMap<Line,usize>,
    becomes_star: usize,
    allow_empty : bool
) -> Vec<Line> {
    let f_is_superset = |g1 : &Group ,g2 : &Group |{ g1.is_superset(g2) };
    let f_union = |g1 : &Group ,g2 : &Group |{ g1.union(g2) };
    let f_intersection = |g1 : &Group ,g2 : &Group |{ g1.intersection(g2) };

    combine_lines_custom(l1, l2, l1_without_one, l2_without_one, seen, None, becomes_star, allow_empty, false, false, f_is_superset, f_union, f_intersection).0
}






#[test]
fn crash_test(){

    //loop {
        let mut eh = EventHandler::null();

    let mut problem = crate::problem::Problem::from_string_active_passive("(0a) (00b) (00c) (00d) (00e) (00f)
(0a) (00b) (00c) (00d) (00e) (01f)
(0a) (00b) (00c) (00d) (01e) (10f)
(0a) (00b) (00c) (00d) (01e) (11f)
(0a) (00b) (00c) (01d) (10e) (00f)
(0a) (00b) (00c) (01d) (10e) (01f)
(0a) (00b) (00c) (01d) (11e) (10f)
(0a) (00b) (00c) (01d) (11e) (11f)
(0a) (00b) (01c) (10d) (00e) (00f)
(0a) (00b) (01c) (10d) (00e) (01f)
(0a) (00b) (01c) (10d) (01e) (10f)
(0a) (00b) (01c) (10d) (01e) (11f)
(0a) (00b) (01c) (11d) (10e) (00f)
(0a) (00b) (01c) (11d) (10e) (01f)
(0a) (00b) (01c) (11d) (11e) (10f)
(0a) (00b) (01c) (11d) (11e) (11f)
(0a) (01b) (10c) (00d) (00e) (00f)
(0a) (01b) (10c) (00d) (00e) (01f)
(0a) (01b) (10c) (00d) (01e) (10f)
(0a) (01b) (10c) (00d) (01e) (11f)
(0a) (01b) (10c) (01d) (10e) (00f)
(0a) (01b) (10c) (01d) (10e) (01f)
(0a) (01b) (10c) (01d) (11e) (10f)
(0a) (01b) (10c) (01d) (11e) (11f)
(0a) (01b) (11c) (10d) (00e) (00f)
(0a) (01b) (11c) (10d) (00e) (01f)
(0a) (01b) (11c) (10d) (01e) (10f)
(0a) (01b) (11c) (10d) (01e) (11f)
(0a) (01b) (11c) (11d) (10e) (00f)
(0a) (01b) (11c) (11d) (10e) (01f)
(0a) (01b) (11c) (11d) (11e) (10f)
(0a) (01b) (11c) (11d) (11e) (11f)
(1a) (10b) (00c) (00d) (00e) (00f)
(1a) (10b) (00c) (00d) (00e) (01f)
(1a) (10b) (00c) (00d) (01e) (10f)
(1a) (10b) (00c) (00d) (01e) (11f)
(1a) (10b) (00c) (01d) (10e) (00f)
(1a) (10b) (00c) (01d) (10e) (01f)
(1a) (10b) (00c) (01d) (11e) (10f)
(1a) (10b) (00c) (01d) (11e) (11f)
(1a) (10b) (01c) (10d) (00e) (00f)
(1a) (10b) (01c) (10d) (00e) (01f)
(1a) (10b) (01c) (10d) (01e) (10f)
(1a) (10b) (01c) (10d) (01e) (11f)
(1a) (10b) (01c) (11d) (10e) (00f)
(1a) (10b) (01c) (11d) (10e) (01f)
(1a) (10b) (01c) (11d) (11e) (10f)
(1a) (10b) (01c) (11d) (11e) (11f)
(1a) (11b) (10c) (00d) (00e) (00f)
(1a) (11b) (10c) (00d) (00e) (01f)
(1a) (11b) (10c) (00d) (01e) (10f)
(1a) (11b) (10c) (00d) (01e) (11f)
(1a) (11b) (10c) (01d) (10e) (00f)
(1a) (11b) (10c) (01d) (10e) (01f)
(1a) (11b) (10c) (01d) (11e) (10f)
(1a) (11b) (10c) (01d) (11e) (11f)
(1a) (11b) (11c) (10d) (00e) (00f)
(1a) (11b) (11c) (10d) (00e) (01f)
(1a) (11b) (11c) (10d) (01e) (10f)
(1a) (11b) (11c) (10d) (01e) (11f)
(1a) (11b) (11c) (11d) (10e) (00f)
(1a) (11b) (11c) (11d) (10e) (01f)
(1a) (11b) (11c) (11d) (11e) (10f)
(1a) (11b) (11c) (11d) (11e) (11f)",
"(0a) (1a)
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
(10e) (11e)
(00f) (00f)
(01f) (01f)
(00f) (10f)
(01f) (11f)
(10f) (11f)").unwrap();
        crate::serial::fix_problem(&mut problem, true, true,&mut eh);
        if problem.diagram_indirect.is_none() {
            problem.compute_partial_diagram(&mut eh);
        }
        let mut new = problem.speedup(&mut eh);
        new.passive.maximize(&mut eh);
        new.compute_diagram(&mut eh);
        new.discard_useless_stuff(true, &mut eh);
        new.sort_active_by_strength();
        new.compute_triviality(&mut eh);
        if new.passive.degree == crate::line::Degree::Finite(2) {
            new.compute_coloring_solvability(&mut eh);
            if let Some(outdegree) = new.orientation_given {
                new.compute_triviality_given_orientation(outdegree, &mut eh);
                new.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
            }
        }
        assert!(new.rename_by_generators().is_ok());
    //}
}

