use crate::group::{Group, GroupType};
use crate::line::Line;
use crate::part::Part;
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
                starred.extend(part.group.0.into_iter());
            }
            let group = Group(starred.into_iter().collect());
            let starred_part = Part {
                group,
                gtype: GroupType::Star,
            };
            self.parts.push(starred_part);
        }

        let mut h = HashMap::new();
        let parts = std::mem::take(&mut self.parts);
        for mut part in parts {
            part.group.0.sort();
            let x = h.get(&part.group).unwrap_or(&GroupType::Many(0));

            use GroupType::*;
            let mut r = match (x, part.gtype) {
                (One, One) => Many(2),
                (One, Many(a)) => Many(a + 1),
                (Many(a), One) => Many(*a + 1),
                (Many(a), Many(b)) => Many(*a + b),
                (Star, _) | (_, Star) => Star,
            };

            if r == GroupType::Many(1) {
                r = GroupType::One;
            }

            h.insert(part.group, r);
        }

        for (group, gtype) in h {
            if gtype != GroupType::Many(0) {
                let part = Part { group, gtype };
                self.parts.push(part);
            }
        }

        self.parts.sort();
    }
}
