use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    constraint::Constraint,
    group::{Group, GroupType},
    line::Line,
    part::Part,
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
