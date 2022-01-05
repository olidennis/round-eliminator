use std::collections::HashMap;

use crate::group::{Group, GroupType, Label};
use crate::part::Part;

impl Part {
    pub fn parse(part: &str, mapping: &mut HashMap<String, Label>) -> Result<Part, &'static str> {
        #[derive(Copy, Clone, Eq, PartialEq)]
        enum State {
            Out,
            In,
        }
        use State::*;

        let mut state = State::Out;
        let mut chars = part.chars();
        let mut current_label_str = String::new();
        let mut group = vec![];
        let mut label_for_str = |s| {
            let next_label = mapping.len() as Label;
            *mapping.entry(s).or_insert(next_label)
        };
        let mut gtype = GroupType::ONE;

        while let Some(c) = chars.by_ref().next() {
            match (state, c) {
                (Out, '(') => {
                    current_label_str.push('(');
                    state = In;
                }
                (Out, ')') => return Err("')' not allowed in a label"),
                (In, '(') => return Err("'(' not allowed in a label"),
                (In, '^') => return Err("'^' not allowed in a label"),
                (In, '*') => return Err("'*' not allowed in a label"),
                (In, ')') => {
                    if current_label_str.len() == 1 {
                        return Err("Empty label not allowed");
                    }
                    current_label_str.push(')');
                    let label = label_for_str(current_label_str);
                    current_label_str = String::new();
                    group.push(label);
                    state = Out;
                }
                (Out, '*') => {
                    gtype = GroupType::Star;
                    break;
                }
                (Out, '^') => {
                    let s: String = chars.by_ref().collect();
                    let n: usize = s.parse().map_err(|_| "Invalid number")?;
                    gtype = GroupType::Many(n);
                }
                (Out, c) => {
                    let label = label_for_str(String::from(c));
                    group.push(label);
                }
                (In, c) => {
                    current_label_str.push(c);
                }
            }
        }
        if chars.next().is_some() {
            return Err("Something after the star");
        }
        if state == In {
            return Err("Missing ')'");
        }

        group.sort_unstable();
        Ok(Part {
            group: Group(group),
            gtype,
        })
    }
}
