use crate::constraint::Constraint;
use crate::group::{Group, GroupType, Label};
use crate::line::Line;
use crate::part::Part;
use crate::problem::Problem;
use std::collections::{HashMap, HashSet};

impl Line {
    pub fn normalize(&mut self) {
        let mut with_star = vec![];
        self.parts.retain(|x| {
            if x.gtype == GroupType::Star {
                with_star.push(x.clone());
                false
            } else {
                true
            }
        });
        if !with_star.is_empty() {
            let mut starred = HashSet::new();
            for part in with_star {
                starred.extend(part.group.iter().cloned());
            }
            let group = Group::from(starred.into_iter().collect());
            let starred_part = Part {
                group,
                gtype: GroupType::Star,
            };
            self.parts.push(starred_part);
        }

        let mut parts = std::mem::take(&mut self.parts);
        for part in parts.iter_mut() {
            part.group.ensure_sorted();
        }

        parts.sort_unstable_by(|part1, part2| part1.group.cmp(&part2.group));
        let mut lastgroup = Group::from(vec![]);
        let mut lastcount = GroupType::Many(0);
        let mut rparts = Vec::with_capacity(parts.len());

        for part in parts {
            if part.group != lastgroup {
                if lastcount != GroupType::Many(0) {
                    rparts.push(Part {
                        group: lastgroup,
                        gtype: lastcount,
                    });
                }
                lastgroup = part.group;
                lastcount = part.gtype;
            } else {
                use GroupType::*;
                lastcount = match (lastcount, part.gtype) {
                    (Many(a), Many(b)) => Many(a + b),
                    (Star, _) | (_, Star) => Star,
                };
            }
        }
        if lastcount != GroupType::Many(0) {
            rparts.push(Part {
                group: lastgroup,
                gtype: lastcount,
            });
        }

        self.parts = rparts;

        self.parts.sort_unstable();
        self.parts.shrink_to_fit();
        for part in self.parts.iter_mut() {
            part.group.shrink_to_fit();
        }
    }

    pub fn sort_by_strength(&mut self, reachability: &HashMap<Label, HashSet<Label>>) {
        self.parts.sort_by(|a, b| {
            if a.group.len() != 1 || b.group.len() != 1 {
                a.cmp(b)
            } else {
                let la = a.group.first();
                let lb = b.group.first();
                match reachability[&la].len().cmp(&reachability[&lb].len()) {
                    std::cmp::Ordering::Less => std::cmp::Ordering::Greater,
                    std::cmp::Ordering::Greater => std::cmp::Ordering::Less,
                    std::cmp::Ordering::Equal => a.cmp(b),
                }
                /*match (
                    reachability[&la].contains(&lb),
                    reachability[&lb].contains(&la),
                ) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.cmp(b),
                }*/
            }
        })
    }
}

impl Constraint {
    pub fn sort_lines_by_strength(&mut self, reachability: &HashMap<Label, HashSet<Label>>) {
        for line in self.lines.iter_mut() {
            line.sort_by_strength(reachability);
        }
    }
}

impl Problem {
    pub fn sort_active_by_strength(&mut self) {
        if self.diagram_indirect.is_none() {
            panic!("diagram required for sort active by strength, but it has not been computed");
        }
        let reachability = self.diagram_indirect_to_reachability_adj();
        self.active.sort_lines_by_strength(&reachability);
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn sort_by_strength() {
        let mut p = Problem::from_string("B B A\n\nB AB").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        p.sort_active_by_strength();
        assert_eq!(format!("{}", p), "A B^2\n\nB BA\n");
    }
}
