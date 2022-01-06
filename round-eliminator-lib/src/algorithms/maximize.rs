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

            let mut without_one = vec![];
            for line in lines {
                let mut current = vec![];
                for (i, part) in line.parts.iter().enumerate() {
                    let before = &line.parts[0..i];
                    let mid = match part.gtype {
                        GroupType::Many(0) => panic!("group of size 0, should not happen"),
                        GroupType::ONE => None,
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

#[allow(clippy::needless_range_loop)]
fn intersections(union: &Part, c1: &Line, c2: &Line) -> Vec<Line> {
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
        let group = s1.group.intersection(&s2.group);
        if group.is_empty() {
            return vec![];
        }
        Some(Part {
            gtype: GroupType::Star,
            group,
        })
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
                                let u1 = c1.parts[i1].group.intersection(&c2.parts[j1].group);
                                let u2 = c1.parts[i2].group.intersection(&c2.parts[j2].group);
                                let u3 = c1.parts[i1].group.intersection(&c2.parts[j2].group);
                                let u4 = c1.parts[i2].group.intersection(&c2.parts[j1].group);

                                if (u4.is_superset(&u1)
                                    && u3.is_superset(&u2)
                                    && (u1 != u4 || u2 != u3))
                                    || (u3.is_superset(&u1)
                                        && u4.is_superset(&u2)
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
                    let intersection = pa.group.intersection(&pb.group);
                    if intersection.0.is_empty() {
                        continue 'outer;
                    }
                    parts.push(Part {
                        gtype: value,
                        group: intersection,
                    });
                }
            }
        }
        if let Some(star_intersection) = &star_intersection {
            parts.push(star_intersection.clone());
        }
        let line = Line { parts };
        //line.normalize();
        result.push(line);
    }

    result
}

fn good_unions(l1: &Line, l2: &Line) -> HashMap<Vec<Label>, Vec<(usize, usize)>> {
    let s1: Vec<_> = l1.parts.iter().map(|part| part.group.as_set()).collect();
    let s2: Vec<_> = l2.parts.iter().map(|part| part.group.as_set()).collect();

    let mut unions = HashMap::new();

    for (i, x) in s1.iter().enumerate() {
        for (j, y) in s2.iter().enumerate() {
            if x.is_superset(y) || y.is_superset(x) {
                continue;
            }

            let union: Vec<_> = x.union(y).cloned().sorted().collect();
            let same_union: &mut Vec<(usize, usize)> = unions.entry(union).or_default();

            let len = same_union.len();
            same_union.retain(|&(xc, yc)| !(s1[xc].is_superset(x) && s2[yc].is_superset(y)));
            if same_union.len() != len
                || same_union
                    .iter()
                    .all(|&(xc, yc)| !(x.is_superset(&s1[xc]) && y.is_superset(&s2[yc])))
            {
                same_union.push((i, j));
            }
        }
    }

    unions
}

fn combine_lines(
    l1: &Line,
    l2: &Line,
    l1_without_one: &[Line],
    l2_without_one: &[Line],
    seen: &mut HashSet<Line>,
    becomes_star: usize,
) -> Vec<Line> {

    let mut result = Constraint {
        lines: vec![],
        is_maximized: false,
        degree: l1.degree(),
    };
    let unions = good_unions(l1, l2);

    for (union, v) in unions {
        let union = Part {
            group: Group(union),
            gtype: GroupType::ONE,
        };
        for (x, y) in v {
            let c1 = &l1_without_one[x];
            let c2 = &l2_without_one[y];
            let lines = intersections(&union, c1, c2);
            for mut newline in lines {
                if newline.has_star() {
                    for parts in newline.parts.iter_mut() {
                        if let GroupType::Many(x) = parts.gtype {
                            if x >= becomes_star {
                                parts.gtype = GroupType::Star;
                            }
                        }
                    }
                }
                newline.normalize();
                if !seen.contains(&newline) {
                    seen.insert(newline.clone());
                    result.add_line_and_discard_non_maximal(newline);
                }
            }
        }
    }
    result.lines
}
