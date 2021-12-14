use crate::problem::Problem;

impl Problem {
    pub fn merge_equivalent_labels(&self) -> Problem {
        let mut p = self.clone();
        let merge_groups = &self.diagram_direct.as_ref().expect("in order to merge equivalent labels, the diagram is required, but it has not been computed").0;
        for (dest, group) in merge_groups {
            for from in group {
                p = p.relax_merge(*from, *dest);
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    fn relax_merge() {
        let mut p =
            Problem::from_string("A ABC ABC\nD EFG DEFG\n\nAB AB\nC ABC\nDEFG DEFG").unwrap();
        p.compute_diagram(&mut EventHandler::null());
        p.passive.lines.sort();
        let p = p.merge_equivalent_labels();
        assert_eq!(format!("{}", p), "A^3\nD^3\n\nA^2\nD^2\n");
    }
}
