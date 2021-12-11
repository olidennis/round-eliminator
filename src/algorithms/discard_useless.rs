use std::collections::HashSet;

use crate::problem::Problem;



impl Problem {


    pub fn discard_useless_passive_labels(&mut self) {
        todo!();
    }

    /*
    pub fn remove_label(&mut self, label : usize){
        self.mapping_label_text.retain(|(l,_)|{
            *l != label
        });
        if let Some(x) = self.mapping_label_oldlabels.as_mut() {
            x.retain(|(l,_)|{
                *l != label
            })
        }
    }*/

    pub fn discard_useless_stuff(&mut self) {
        self.passive.discard_non_maximal_lines();
        self.remove_weak_active_lines();
    }

    pub fn remove_weak_active_lines(&mut self) {

        let reachable = self.diagram_indirect_to_reachability_adj();

        // part 1: make groups smaller if possible
        for line in self.active.lines.iter_mut() {
            for part in line.parts.iter_mut() {
                let group = part.group.as_set();

                let mut newgroup = vec![];
                'outer: for &label in &group {
                    for &other in &group {
                        if label != other && reachable[&label].contains(&other) {
                            continue 'outer;
                        }
                    }
                    newgroup.push(label);
                }
                newgroup.sort();
                part.group.0 = newgroup;
            }
        }


        // part 2: remove lines by inclusion
        self.active.discard_non_maximal_lines_with_custom_supersets(Some(|h1 : &HashSet<usize>, h2 : &HashSet<usize>|{
            // h2 is superset of h1 if all elements of h1 have a successor in h2
            h2.iter().all(|x|{
                h1.iter().any(|y|{
                    reachable[x].contains(y)
                })
            })
        }));

        // remove from passive side the labels that do not appear anymore on the active side
        self.discard_useless_passive_labels();
    }
}


#[cfg(test)]
mod tests {

    use crate::problem::Problem;

    #[test]
    fn useless1() {
        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.compute_diagram();
        p.remove_weak_active_lines();
        assert_eq!(format!("{}", p), "A B^2\n\nAB B\n");

        let mut p = Problem::from_string("M M M\nP UP UP\n\nM UP\nU U").unwrap();
        p.compute_diagram();
        p.remove_weak_active_lines();
        assert_eq!(format!("{}", p), "M^3\nP U^2\n\nM PU\nMU U\n");
    }

    #[test]
    fn useless2() {
        let mut p = Problem::from_string("A A A\nA A B\n A B B\n\nB AB").unwrap();
        p.compute_diagram();
        p.remove_weak_active_lines();
        assert_eq!(format!("{}", p), "A B^2\n\nAB B\n");

        let mut p = Problem::from_string("M M M\nP U P\nP U U\nP P P\n\nM UP\nU U").unwrap();
        p.compute_diagram();
        p.remove_weak_active_lines();
        assert_eq!(format!("{}", p), "M^3\nP U^2\n\nM PU\nMU U\n");
    }
}
