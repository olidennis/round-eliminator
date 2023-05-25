use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use streaming_iterator::StreamingIterator;

use crate::{
    algorithms::multisets_pairing::Pairings,
    constraint::Constraint,
    group::{Group, GroupType, Label},
    line::Line,
    part::Part,
};

use super::event::EventHandler;

impl Constraint {
    pub fn maximize(&mut self, eh: &mut EventHandler) {
        if self.is_maximized || self.lines.is_empty() {
            self.is_maximized = true;
            return;
        }

        let becomes_star = 100;

        let mut seen = HashSet::new();
        let mut seen_pairs = HashSet::new();

        let lines = std::mem::take(&mut self.lines);
        let empty = self.clone();
        for mut line in lines {
            line.normalize();
            seen.insert(line.clone());
            self.add_line_and_discard_non_maximal(line);
        }

        loop {
            let mut newconstraint = self.clone();
            let lines = &self.lines;

            let without_one = without_one(lines);

            for i in 0..lines.len() {
                let mut candidates2 = empty.clone();

                for j in 0..=i {
                    let len = lines.len();
                    eh.notify("combining line pairs", i * len + j, len * len);

                    let pair = (lines[i].clone(), lines[j].clone());
                    if seen_pairs.contains(&pair)
                        || seen_pairs.contains(&(pair.1.clone(), pair.0.clone()))
                    {
                        continue;
                    }
                    seen_pairs.insert(pair);

                    let candidates = combine_lines(
                        &lines[i],
                        &lines[j],
                        &without_one[i],
                        &without_one[j],
                        &mut seen,
                        becomes_star,
                        false
                    );
                    for newline in candidates {
                        candidates2.add_line_and_discard_non_maximal(newline);
                    }
                }

                for newline in candidates2.lines {
                    newconstraint.add_line_and_discard_non_maximal(newline);
                }
            }

            if &newconstraint == self {
                break;
            }
            *self = newconstraint;
        }

        self.is_maximized = true;
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

#[allow(clippy::needless_range_loop)]
fn intersections<FS,FI>(union: (usize,usize,Operation,Part), c1: &Line, c2: &Line, allow_empty : bool, f_is_superset : FS, f_intersection : FI) -> (Vec<Vec<(usize,usize,Operation,Part)>>) where FS : Fn(&Group,&Group) -> bool, FI : Fn(&Group,&Group) -> Group {
    let t1 = c1.degree_without_star();
    let t2 = c2.degree_without_star();
    let d = if c1.has_star() { t1 + t2 } else { t1 };
    let star1 = d - t1;
    let star2 = d - t2;

    let line_to_counts = |line: &Line, starvalue| -> Vec<usize> {
        line.parts
            .iter()
            .map(|part| match part.gtype {
                //GroupType::ONE => 1,
                GroupType::Many(x) => x,
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
                    let value = GroupType::Many(pairing[i][j]);
                    let intersection = f_intersection(&pa.group,&pb.group); 
                    if !allow_empty && intersection.0.is_empty() { 
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

pub fn combine_lines_custom<FS,FU,FI>(
    l1: &Line,
    l2: &Line,
    l1_without_one: &[Line],
    l2_without_one: &[Line],
    seen: &mut HashSet<Line>,
    becomes_star: usize,
    allow_empty : bool,
    track_unions : bool,
    track_all : bool,
    f_is_superset : FS,
    f_union : FU,
    f_intersection : FI
) -> (Vec<Line>,HashMap<Line,HashSet<(usize,usize)>>,HashMap<Line, (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>)>) where FS : Fn(&Group,&Group) -> bool + Copy, FU : Fn(&Group,&Group) -> Group + Copy, FI : Fn(&Group,&Group) -> Group + Copy {
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
                            if x >= becomes_star {
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
                if !seen.contains(&newline) {
                    seen.insert(newline.clone());
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
    seen: &mut HashSet<Line>,
    becomes_star: usize,
    allow_empty : bool
) -> Vec<Line> {
    let f_is_superset = |g1 : &Group ,g2 : &Group |{ g1.is_superset(g2) };
    let f_union = |g1 : &Group ,g2 : &Group |{ g1.union(g2) };
    let f_intersection = |g1 : &Group ,g2 : &Group |{ g1.intersection(g2) };

    combine_lines_custom(l1, l2, l1_without_one, l2_without_one, seen, becomes_star, allow_empty, false, false, f_is_superset, f_union, f_intersection).0
}