use std::collections::HashSet;

use itertools::Itertools;

use crate::problem::Problem;

impl Problem {
    pub fn rename(&mut self, v: &[(usize, &str)]) -> Result<(), &'static str> {
        let given_labels: HashSet<usize> = v.iter().map(|(l, _)| *l).unique().collect();
        let labels: HashSet<usize> = self
            .mapping_label_text
            .iter()
            .map(|(l, _)| *l)
            .unique()
            .collect();

        if labels != given_labels || given_labels.len() != v.len() {
            return Err("There is something wrong with the given renaming");
        }

        if v.iter().map(|(_, s)| s).unique().count() != labels.len() {
            return Err("Labels are not unique");
        }

        let mut renaming = vec![];

        for (l, s) in v {
            if s.chars().any(|c| "()*^ ".contains(c)) {
                return Err("Some label contains characters that are not allowed");
            }
            if s.len() == 1 {
                renaming.push((*l, s.to_string()));
            } else {
                renaming.push((*l, format!("({})", s)));
            }
        }

        self.mapping_label_text = renaming;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::problem::Problem;

    #[test]
    fn renaming() {
        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.rename(&[(0, "B"), (1, "A")]).unwrap();
        assert_eq!(format!("{}", p), "B BA^2\n\nBA A\n");

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        p.rename(&[(1, "ABC"), (0, "TEST")]).unwrap();
        assert_eq!(
            format!("{}", p),
            "(TEST) (TEST)(ABC)^2\n\n(TEST)(ABC) (ABC)\n"
        );

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST"), (0, "ABC")]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST")]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "TEST"), (1, "TEST")]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A"), (1, "B*")]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A"), (1, "(B)")]).is_err());

        let mut p = Problem::from_string("A AB AB\n\nB AB").unwrap();
        assert!(p.rename(&[(0, "A"), (1, "B ")]).is_err());
    }
}
