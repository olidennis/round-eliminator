use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    group::{Group, GroupType, Label},
    line::{Degree, Line},
    part::Part,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Constraint {
    pub lines: Vec<Line>,
    pub is_maximized: bool,
    pub degree: Degree,
}

impl Constraint {

    pub fn includes_with_custom_supersets<T>(&self, newline : &Line, is_superset: Option<T>) -> bool where
    T: Fn(&Group, &Group) -> bool + Copy {
        self.lines.iter().any(|oldline| oldline.includes_with_custom_supersets(newline, is_superset))
    }

    pub fn includers_with_custom_supersets<T>(&self, newline : &Line, is_superset: Option<T>) -> Vec<Line> where
    T: Fn(&Group, &Group) -> bool + Copy {
        self.lines.iter().filter(|oldline| oldline.includes_with_custom_supersets(newline, is_superset)).cloned().collect()
    }

    pub fn add_line_and_discard_non_maximal(&mut self, newline: Line) {
        self.add_line_and_discard_non_maximal_with_custom_supersets(
            newline,
            None::<fn(&'_ _, &'_ _) -> _>,
        );
    }

    pub fn discard_non_maximal_lines(&mut self) {
        self.discard_non_maximal_lines_with_custom_supersets(None::<fn(&'_ _, &'_ _) -> _>)
    }


    pub fn is_included_with_custom_supersets<T>(&self, newline : &Line,is_superset: Option<T>,
    ) -> bool where T: Fn(&Group, &Group) -> bool + Copy + Sync {
        // a line is likely to be included in the lines added recently, so rev() is useful
        !self.lines.iter().rev().all(|oldline| !oldline.includes_with_custom_supersets(&newline, is_superset))
    }

    #[inline(never)]
    pub fn add_line_and_discard_non_maximal_with_custom_supersets<T>(
        &mut self,
        newline: Line,
        is_superset: Option<T>,
    ) -> bool where
        T: Fn(&Group, &Group) -> bool + Copy + Sync,
    {
        if self.is_included_with_custom_supersets(&newline, is_superset) {
            return false;
        }
        self.is_maximized = false;
        self.lines.retain(|oldline| !newline.includes_with_custom_supersets(oldline, is_superset));
        self.lines.push(newline);
        true
    }

    pub fn discard_non_maximal_lines_with_custom_supersets<T>(&mut self, is_superset: Option<T>)
    where
        T: Fn(&Group, &Group) -> bool + Copy + Sync,
    {
        self.is_maximized = false;
        let lines = std::mem::take(&mut self.lines);
        for line in lines {
            self.add_line_and_discard_non_maximal_with_custom_supersets(line, is_superset);
        }
    }

    pub fn parse<S: AsRef<str>>(
        text: S,
        mapping: &mut HashMap<String, Label>,
    ) -> Result<Constraint, &'static str> {
        let text = text.as_ref();
        let lines: Vec<_> = text
            .lines()
            .map(|l| Line::parse(l, mapping))
            .collect::<Result<_, _>>()?;
        if lines.is_empty() {
            return Err("Empty constraint");
        }
        let degree = lines[0].degree();
        if lines.iter().any(|line| line.degree() != degree) {
            return Err("Lines have different degrees");
        }
        let mut constraint = Constraint {
            lines,
            is_maximized: false,
            degree,
        };
        constraint.discard_non_maximal_lines();
        Ok(constraint)
    }

    pub fn includes(&self, other: &Line) -> bool {
        if !self.is_maximized && self.degree != Degree::Finite(2) {
            panic!("this should not happen");
        }
        if self.is_maximized || self.degree != Degree::Finite(2) {
            self.lines.iter().any(|line| line.includes(other))
        } else {
            self.includes_slow(other)
        }
    }

    pub fn includes_single_line(&self, other: &Line) -> bool {
        for parts in &other.parts {
            assert!(parts.group.len() == 1);
        }
        self.lines.iter().any(|line| line.includes(other))
    }

    pub fn includes_slow(&self, other: &Line) -> bool {
        assert!(self.degree == Degree::Finite(2));
        let a = &other.parts[0].group;
        let b = if other.parts.len() == 1 {
            &other.parts[0].group
        } else {
            &other.parts[1].group
        };
        let lines = self.lines.iter().flat_map(|line| {
            let cur_a = &line.parts[0].group;
            let cur_b = if line.parts.len() == 1 {
                &line.parts[0].group
            } else {
                &line.parts[1].group
            };
            std::iter::once((cur_a, cur_b)).chain(std::iter::once((cur_b, cur_a)))
        });

        Constraint::includes_slow_helper(a, b, lines)
    }

    pub fn includes_slow_helper<'a>(
        g1: &Group,
        g2: &Group,
        mut lines: impl Iterator<Item = (&'a Group, &'a Group)> + 'a + Clone,
    ) -> bool {
        if g1.is_empty() || g2.is_empty() {
            return true;
        }

        while let Some((o1, o2)) = lines.next() {
            let int1 = o1.intersection(g1);
            let int2 = o2.intersection(g2);
            if !int1.is_empty() && !int2.is_empty() {
                let diff1 = g1.difference(o1);
                let diff2 = g2.difference(o2);
                return Constraint::includes_slow_helper(&int1, &diff2, lines.clone())
                    && Constraint::includes_slow_helper(&diff1, &int2, lines.clone())
                    && Constraint::includes_slow_helper(&diff1, &diff2, lines);
            }
        }

        false
    }

    pub fn is_diagram_predecessor_partial(&self, l1: Label, l2: Label) -> bool {
        // this function just checks if *every time* a label appears, then also the other appears
        self.groups()
            .all(|group| !group.contains(&l1) || group.contains(&l2))
    }

    pub fn is_diagram_predecessor(&self, l1: Label, l2: Label) -> /*Option<(Line,Line)>*/ bool {
        // this is commented out so that one may still try to see if a label is a predecessor of another label
        // if the result is true, then it is always correct
        // if the result is false, it may just be because of a non-maximized right side
        //if !self.is_maximized {
        //    panic!("Maximization has not been performed");
        //}
        for line in &self.lines {
            for (i, part) in line.parts.iter().enumerate() {
                if part.group.contains(&l1) {
                    let mut test = line.clone();
                    match test.parts[i].gtype {
                        GroupType::ONE => {
                            test.parts[i].group = Group::from(vec![l2]);
                        }
                        GroupType::Many(x) => {
                            test.parts[i].gtype = GroupType::Many(x - 1);
                            let part = Part {
                                group: Group::from(vec![l2]),
                                gtype: GroupType::ONE,
                            };
                            test.parts.push(part);
                        }
                        GroupType::Star => {
                            let part = Part {
                                group: Group::from(vec![l2]),
                                gtype: GroupType::ONE,
                            };
                            test.parts.push(part);
                        }
                    }
                    if !self.includes(&test) {
                        /*for mut candidate in test.all_choices(false) {
                            if !self.includes(&candidate) {
                                let mut orig = candidate.clone();
                                for part in candidate.parts.iter_mut() {
                                    if *part == (Part {
                                        group: Group::from(vec![l2]),
                                        gtype: GroupType::ONE,
                                    }) {
                                        *part = (Part {
                                            group: Group::from(vec![l1]),
                                            gtype: GroupType::ONE,
                                        });
                                        break;
                                    } 
                                }
                                candidate.normalize();
                                orig.normalize();
                                assert!(candidate != orig);
                                return Some((candidate,orig));
                            }
                        }*/
                        //unreachable!("a candidate should have been found");
                        return false;
                    }
                }
            }
        }
        //None
        true
    }

    pub fn labels_appearing(&self) -> HashSet<Label> {
        let mut h = HashSet::new();
        for group in self.groups() {
            for &label in group.iter() {
                h.insert(label);
            }
        }
        h
    }

    pub fn finite_degree(&self) -> usize {
        if let Degree::Finite(d) = self.degree {
            return d;
        }
        panic!("the degree is not finite");
    }
}

#[cfg(test)]
mod tests {

    use crate::{group::Group, problem::Problem};
    use std::collections::HashSet;

    #[test]
    fn sets_of_all_choices() {
        let p = Problem::from_string("A B^2 C*\nD E E*\n\nA BCDE").unwrap();
        assert_eq!(
            p.active.sets_of_all_choices(),
            HashSet::from([Group::from(vec![0, 1, 2]), Group::from(vec![3, 4])])
        );

        let p = Problem::from_string("A AB AB\nCD EF EF\n\nA BCDEF").unwrap();
        assert_eq!(
            p.active.sets_of_all_choices(),
            HashSet::from([
                Group::from(vec![0]),
                Group::from(vec![0, 1]),
                Group::from(vec![2, 4]),
                Group::from(vec![2, 5]),
                Group::from(vec![3, 4]),
                Group::from(vec![3, 5])
            ])
        );

        let p = Problem::from_string("A AB AB\nCD CEF CEF\n\nA BCDEF").unwrap();
        assert_eq!(
            p.active.minimal_sets_of_all_choices(),
            vec!(
                HashSet::from([0]),
                HashSet::from([2]),
                HashSet::from([3, 4]),
                HashSet::from([3, 5])
            )
        );
    }
}
