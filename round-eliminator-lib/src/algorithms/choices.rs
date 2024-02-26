use permutator::copy::CartesianProductIterator;
use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    constraint::Constraint,
    group::{Group, GroupType, Label},
    line::Line,
    part::Part,
};

impl Constraint {
    pub fn sets_of_all_choices(&self) -> HashSet<Group> {
        let mut result = HashSet::new();

        for line in &self.lines {
            line.sets_of_all_choices(&mut result);
        }
        result
    }

    pub fn minimal_sets_of_all_choices(&self) -> Vec<HashSet<Label>> {
        minimal_sets(self.sets_of_all_choices())
    }

    //panics if there are stars
    pub fn all_choices(&self, normalize: bool) -> Vec<Line> {
        let mut result = vec![];
        let mut seen = HashSet::new();
        for line in &self.lines {
            let mut domain = vec![];
            for part in &line.parts {
                for _ in 0..part.gtype.value() {
                    domain.push(&part.group[..]);
                }
            }
            let all = CartesianProductIterator::new(&domain);
            for choice in all {
                let mut sorted = choice.clone();
                sorted.sort_unstable();
                if !seen.contains(&sorted) {
                    seen.insert(sorted);
                    let mut new = Line {
                        parts: choice
                            .into_iter()
                            .map(|g| Part {
                                group: Group(vec![g]),
                                gtype: GroupType::ONE,
                            })
                            .collect(),
                    };
                    if normalize {
                        new.normalize();
                    }
                    result.push(new);
                }
            }
        }
        result
    }
}

impl Line {

    pub fn line_set(&self) -> Group {
        let groups = self.parts.iter().map(|part| &part.group);
        if groups.clone().all(|group| group.len() == 1) {
            let labels = groups.map(|group| group[0]).unique();
            let set = Self::labels_to_set(labels);
            set
        } else {
            panic!("this function only works when all groups have size 1");
        }

    }

    pub fn labels_to_set<T: Iterator<Item = Label>>(labels: T) -> Group {
        let mut group: Vec<_> = labels.into_iter().collect();
        group.sort_unstable();
        Group(group)
    }

    pub fn sets_of_all_choices(&self, result: &mut HashSet<Group>) {
        let groups = self.parts.iter().map(|part| &part.group);
        if groups.clone().all(|group| group.len() == 1) {
            let labels = groups.map(|group| group[0]).unique();
            let set = Self::labels_to_set(labels);
            result.insert(set);
        } else {
            let domain: Vec<_> = groups.map(|group| &group[..]).collect();
            for labels in CartesianProductIterator::new(&domain) {
                let set = Self::labels_to_set(labels.into_iter().unique());
                result.insert(set);
            }
        }
    }

    pub fn minimal_sets_of_all_choices(&self) -> Vec<HashSet<Label>> {
        let mut all_sets = HashSet::new();
        self.sets_of_all_choices(&mut all_sets);
        minimal_sets(all_sets)
    }
}

pub fn minimal_sets(all_sets: HashSet<Group>) -> Vec<HashSet<Label>> {
    let mut result: Vec<HashSet<Label>> = vec![];
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
