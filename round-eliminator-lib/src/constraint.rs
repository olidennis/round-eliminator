use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use permutator::copy::CartesianProductIterator;
use serde::{Deserialize, Serialize};

use crate::{
    group::{Group, GroupType},
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
    pub fn add_line_and_discard_non_maximal(&mut self, newline: Line) {
        self.add_line_and_discard_non_maximal_with_custom_supersets(
            newline,
            None::<fn(&'_ _, &'_ _) -> _>,
        )
    }

    pub fn discard_non_maximal_lines(&mut self) {
        self.discard_non_maximal_lines_with_custom_supersets(None::<fn(&'_ _, &'_ _) -> _>)
    }

    pub fn add_line_and_discard_non_maximal_with_custom_supersets<T>(
        &mut self,
        newline: Line,
        is_superset: Option<T>,
    ) where
        T: Fn(&HashSet<usize>, &HashSet<usize>) -> bool + Copy,
    {
        self.is_maximized = false;
        let lines = &mut self.lines;
        // computing and checking len is not needed, but it allows to skip the second check if something already changed,
        // assuming that there are only maximal lines at the moment
        let len = lines.len();
        lines.retain(|oldline| !newline.includes_with_custom_supersets(oldline, is_superset));
        if lines.len() != len
            || lines
                .iter()
                .all(|oldline| !oldline.includes_with_custom_supersets(&newline, is_superset))
        {
            lines.push(newline);
        }
    }

    pub fn discard_non_maximal_lines_with_custom_supersets<T>(&mut self, is_superset: Option<T>)
    where
        T: Fn(&HashSet<usize>, &HashSet<usize>) -> bool + Copy,
    {
        self.is_maximized = false;
        let lines = std::mem::take(&mut self.lines);
        for line in lines {
            self.add_line_and_discard_non_maximal_with_custom_supersets(line, is_superset);
        }
    }

    pub fn parse<S: AsRef<str>>(
        text: S,
        mapping: &mut HashMap<String, usize>,
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

    fn sets_of_all_choices(&self) -> HashSet<Group> {
        let mut result = HashSet::new();

        fn labels_to_set<T: Iterator<Item = usize>>(labels: T) -> Group {
            let mut group: Vec<_> = labels.into_iter().collect();
            group.sort();
            Group(group)
        }

        for line in &self.lines {
            let groups = line.parts.iter().map(|part| &part.group);
            if groups.clone().all(|group| group.len() == 1) {
                let labels = groups.map(|group| group[0]).unique();
                let set = labels_to_set(labels);
                result.insert(set);
            } else {
                let domain: Vec<_> = groups.map(|group| &group[..]).collect();
                for labels in CartesianProductIterator::new(&domain) {
                    let set = labels_to_set(labels.into_iter().unique());
                    result.insert(set);
                }
            }
        }
        result
    }

    pub fn minimal_sets_of_all_choices(&self) -> Vec<HashSet<usize>> {
        let all_sets = self.sets_of_all_choices();
        let mut result: Vec<HashSet<usize>> = vec![];
        for set in all_sets.into_iter().sorted() {
            let set = HashSet::from_iter(set.0.into_iter());
            let len = result.len();
            result.retain(|x| !x.is_superset(&set));
            if result.len() != len || result.iter().all(|r| !set.is_superset(r)) {
                result.push(set);
            }
        }
        result
    }

    pub fn includes(&self, other: &Line) -> bool {
        self.lines.iter().any(|line| line.includes(other))
    }

    pub fn is_diagram_predecessor(&self, l1: usize, l2: usize) -> bool {
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
                            test.parts[i].group.0 = vec![l2];
                        }
                        GroupType::Many(x) => {
                            test.parts[i].gtype = GroupType::Many(x - 1);
                            let part = Part {
                                group: Group(vec![l2]),
                                gtype: GroupType::ONE,
                            };
                            test.parts.push(part);
                        }
                        GroupType::Star => {
                            let part = Part {
                                group: Group(vec![l2]),
                                gtype: GroupType::ONE,
                            };
                            test.parts.push(part);
                        }
                    }
                    if !self.includes(&test) {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn labels_appearing(&self) -> HashSet<usize> {
        let mut h = HashSet::new();
        for group in self.groups() {
            for &label in &group.0 {
                h.insert(label);
            }
        }
        h
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
            HashSet::from([Group(vec![0, 1, 2]), Group(vec![3, 4])])
        );

        let p = Problem::from_string("A AB AB\nCD EF EF\n\nA BCDEF").unwrap();
        assert_eq!(
            p.active.sets_of_all_choices(),
            HashSet::from([
                Group(vec![0]),
                Group(vec![0, 1]),
                Group(vec![2, 4]),
                Group(vec![2, 5]),
                Group(vec![3, 4]),
                Group(vec![3, 5])
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
