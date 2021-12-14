use std::collections::{HashSet, HashMap};

use itertools::Itertools;
use streaming_iterator::StreamingIterator;

use crate::{
    constraint::Constraint,
    group::{Group, GroupType},
    line::Line,
    part::Part, algorithms::multisets_pairing::Pairings,
};

use super::event::EventHandler;

impl Constraint {
    // TEMPORARY, SLOW, JUST FOR DEGREE 2, FOR TESTING
    pub fn maximize(&mut self, eh : &mut EventHandler) {
        if self.is_maximized || self.lines.is_empty() {
            return;
        }

        if self.lines[0].degree_without_star() == 2 && !self.lines[0].has_star() {
            let mut c = self.clone();
            loop {
                eh.notify("maximize", 1, 1);
                
                let mut newc = c.clone();

                for l1 in &c.lines {
                    for l2 in &c.lines {
                        let g11 = l1.parts[0].group.0.clone();
                        let g12 = if l1.parts.len() == 1 {
                            l1.parts[0].group.0.clone()
                        } else {
                            l1.parts[1].group.0.clone()
                        };

                        let g21 = l2.parts[0].group.0.clone();
                        let g22 = if l2.parts.len() == 1 {
                            l2.parts[0].group.0.clone()
                        } else {
                            l2.parts[1].group.0.clone()
                        };

                        let g11: HashSet<usize> = HashSet::from_iter(g11.into_iter());
                        let g12: HashSet<usize> = HashSet::from_iter(g12.into_iter());
                        let g21: HashSet<usize> = HashSet::from_iter(g21.into_iter());
                        let g22: HashSet<usize> = HashSet::from_iter(g22.into_iter());

                        let i_11_21 = g11.intersection(&g21);
                        let i_11_22 = g11.intersection(&g22);
                        let i_12_21 = g12.intersection(&g21);
                        let i_12_22 = g12.intersection(&g22);

                        let u_11_21 = g11.union(&g21);
                        let u_11_22 = g11.union(&g22);
                        let u_12_21 = g12.union(&g21);
                        let u_12_22 = g12.union(&g22);

                        let i_11_21: Vec<_> = i_11_21.into_iter().cloned().sorted().collect();
                        let i_11_22: Vec<_> = i_11_22.into_iter().cloned().sorted().collect();
                        let i_12_21: Vec<_> = i_12_21.into_iter().cloned().sorted().collect();
                        let i_12_22: Vec<_> = i_12_22.into_iter().cloned().sorted().collect();

                        let u_11_21: Vec<_> = u_11_21.into_iter().cloned().sorted().collect();
                        let u_11_22: Vec<_> = u_11_22.into_iter().cloned().sorted().collect();
                        let u_12_21: Vec<_> = u_12_21.into_iter().cloned().sorted().collect();
                        let u_12_22: Vec<_> = u_12_22.into_iter().cloned().sorted().collect();

                        let gtype = GroupType::One;

                        if !i_11_21.is_empty() {
                            let mut l1 = Line {
                                parts: vec![
                                    Part {
                                        gtype,
                                        group: Group(i_11_21),
                                    },
                                    Part {
                                        gtype,
                                        group: Group(u_12_22),
                                    },
                                ],
                            };
                            l1.parts.sort();
                            l1.normalize();
                            newc.add_line_and_discard_non_maximal(l1);
                        }

                        if !i_11_22.is_empty() {
                            let mut l2 = Line {
                                parts: vec![
                                    Part {
                                        gtype,
                                        group: Group(i_11_22),
                                    },
                                    Part {
                                        gtype,
                                        group: Group(u_12_21),
                                    },
                                ],
                            };
                            l2.parts.sort();
                            l2.normalize();
                            newc.add_line_and_discard_non_maximal(l2);
                        }

                        if !i_12_21.is_empty() {
                            let mut l3 = Line {
                                parts: vec![
                                    Part {
                                        gtype,
                                        group: Group(i_12_21),
                                    },
                                    Part {
                                        gtype,
                                        group: Group(u_11_22),
                                    },
                                ],
                            };
                            l3.parts.sort();
                            l3.normalize();
                            newc.add_line_and_discard_non_maximal(l3);
                        }

                        if !i_12_22.is_empty() {
                            let mut l4 = Line {
                                parts: vec![
                                    Part {
                                        gtype,
                                        group: Group(i_12_22),
                                    },
                                    Part {
                                        gtype,
                                        group: Group(u_11_21),
                                    },
                                ],
                            };
                            l4.parts.sort();
                            l4.normalize();
                            newc.add_line_and_discard_non_maximal(l4);
                        }
                    }
                }

                if c == newc {
                    break;
                }
                c = newc;
            }
            *self = c;
            self.is_maximized = true;
        } else {
            self.is_maximized = true;
            log::warn!("This is not implemented.")
        }
    }
}



fn intersections(union : &Part, c1 : &Line, c2 : &Line) -> Vec<Line> {
    let t1 = c1.degree_without_star();
    let t2 = c2.degree_without_star();
    let d = if c1.has_star() { t1 + t2 } else { t1 };
    let star1 = d - t1;
    let star2 = d - t2;

    let line_to_counts = |line : &Line,starvalue| -> Vec<usize> {
        line.parts.iter().map(|part| {
            match part.gtype {
                GroupType::One => 1,
                GroupType::Many(x) => x,
                GroupType::Star => starvalue
            }
        }).collect()
    };

    let star_intersection = if let (Some(s1),Some(s2)) = (c1.get_star(),c2.get_star()) {
        let group = s1.group.intersection(&s2.group);
        if group.is_empty() {
            return vec![];
        }
        Some(Part{gtype : GroupType::Star, group })
    } else {
        None
    };

    let mut result = vec![];


    let v1 = line_to_counts(c1,star1);
    let v2 = line_to_counts(c1,star2);

    let mut pairings = Pairings::new(v1,v2);

    'outer: while let Some(pairing) = pairings.next() {
        let mut parts = vec![];
        parts.push(union.clone());
        for (i,pa) in c1.parts.iter().enumerate() {
            for (j,pb) in c2.parts.iter().enumerate() {
                let value = GroupType::Many(pairing[i][j]);
                let intersection = pa.group.intersection(&pb.group);
                if intersection.0.is_empty() {
                    continue 'outer;
                }
                parts.push(Part{ gtype : value, group : intersection });
            }
        }
        if let Some(star_intersection) = &star_intersection {
            parts.push(star_intersection.clone());
        }
        let mut line = Line{parts};
        line.normalize();
        result.push(line);
    }

    result
}

fn good_unions<BigNum>(l1 : &Line, l2 : &Line) -> HashMap<Vec<usize>,Vec<(usize,usize)>> {

    let s1 : Vec<_> = l1.parts.iter().map(|part|part.group.as_set()).collect();
    let s2 : Vec<_> = l2.parts.iter().map(|part|part.group.as_set()).collect();

    let mut unions = HashMap::new();
    
    for (i,x) in s1.iter().enumerate() {
        for (j,y) in s2.iter().enumerate() {

            if x.is_subset(y) || y.is_superset(x) {
                continue;
            }

            let union : Vec<_> = x.union(y).cloned().sorted().collect();
            let same_union : &mut Vec<(usize,usize)> = unions.entry(union).or_default();

            same_union.retain(|&(xc,yc)| !(s1[xc].is_superset(x) && s2[yc].is_superset(y)) );
            let len = same_union.len();
            if same_union.len() != len || same_union.iter().all(|&(xc,yc)| !(x.is_superset(&s1[xc]) && y.is_superset(&s2[yc])) ) {
                same_union.push((i,j));
            }

        }
    }

    unions
}