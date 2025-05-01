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
                    domain.push(part.group.as_vec());
                }
            }
            let domain: Vec<_> = domain.iter().map(|v|&v[..]).collect();
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
                                group: Group::from(vec![g]),
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
            let labels = groups.map(|group| group.first()).unique();
            let set = Self::labels_to_set(labels);
            set
        } else {
            panic!("this function only works when all groups have size 1");
        }

    }

    pub fn labels_to_set<T: Iterator<Item = Label>>(labels: T) -> Group {
        let mut group: Vec<_> = labels.into_iter().collect();
        group.sort_unstable();
        Group::from(group)
    }

    pub fn sets_of_all_choices(&self, result: &mut HashSet<Group>) {
        let groups = self.parts.iter().map(|part| &part.group);
        if groups.clone().all(|group| group.len() == 1) {
            let labels = groups.map(|group| group.first()).unique();
            let set = Self::labels_to_set(labels);
            result.insert(set);
        } else {
            let domain: Vec<_> = groups.map(|group| group.as_vec()).collect();
            let domain: Vec<_> = domain.iter().map(|v|&v[..]).collect();

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
        let set = set.as_set();
        let len = result.len();
        result.retain(|x| !x.is_superset(&set));
        if result.len() != len || result.iter().all(|r| !set.is_superset(r)) {
            result.push(set);
        }
    }
    result
}


pub fn left_labels<T>(labels : &Vec<Label>, is_right: T) -> Vec<Label> where T: Fn(Label, Label) -> bool + Copy + Sync {
    let mut result: Vec<Label> = vec![];
    for &label in labels {
        let len = result.len();
        result.retain(|&x| !is_right(x,label));
        if result.len() != len || result.iter().all(|&r| !is_right(label,r)) {
            result.push(label);
        }
    }
    result
}
