use crate::group::{GroupType, Label};
use crate::part::Part;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Line {
    pub parts: Vec<Part>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Degree {
    Finite(usize),
    Star,
}

impl Line {
    pub fn parse(line: &str, mapping: &mut HashMap<String, Label>) -> Result<Line, &'static str> {
        let parts = line
            .split_whitespace()
            .map(|part| Part::parse(part, mapping))
            .collect::<Result<_, _>>()?;
        let mut line = Line { parts };
        line.normalize();
        Ok(line)
    }

    pub fn to_string(&self, mapping: &HashMap<Label, String>) -> String {
        self.parts.iter().map(|p| p.to_string(mapping)).join(" ")
    }

    pub fn degree_without_star(&self) -> usize {
        let mut s = 0;
        for part in &self.parts {
            use GroupType::*;
            match part.gtype {
                //One => {
                //    s += 1;
                //}
                Many(n) => {
                    s += n;
                }
                Star => {}
            }
        }
        s
    }

    pub fn has_star(&self) -> bool {
        self.get_star().is_some()
    }

    pub fn get_star(&self) -> Option<&Part> {
        self.parts.iter().find(|x| x.gtype == GroupType::Star)
    }

    pub fn degree(&self) -> Degree {
        let mut s = 0;
        for part in &self.parts {
            use GroupType::*;
            match part.gtype {
                //One => {
                //    s += 1;
                //}
                Many(n) => {
                    s += n;
                }
                Star => {
                    return Degree::Star;
                }
            }
        }
        Degree::Finite(s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::line::Line;

    #[test]
    fn valid_conversions() {
        let mut h = HashMap::new();
        let p = Line::parse("ABC", &mut h).unwrap();
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "ABC");

        let mut h = HashMap::new();
        let p = Line::parse("ABC^0", &mut h).unwrap();
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "");

        let mut h = HashMap::new();
        let p = Line::parse("AB ABC^1 AB ABC^0 ABC^5", &mut h).unwrap();
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "AB^2 ABC^6");

        let mut h = HashMap::new();
        let p = Line::parse("AB AB* AB^3 AB", &mut h).unwrap();
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "AB*");

        let mut h = HashMap::new();
        let p = Line::parse("AB ABC^1 BA BCA^0 BAC^5", &mut h).unwrap();
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "AB^2 ABC^6");
    }

    #[test]
    fn star() {
        let p = Line::parse("AB AB* AB^3 AB", &mut HashMap::new()).unwrap();
        assert_eq!(p.has_star(), true);

        let p = Line::parse("AB AB AB^3 AB", &mut HashMap::new()).unwrap();
        assert_eq!(p.has_star(), false);
    }

    #[test]
    fn degree() {
        let p = Line::parse("AB AB* AB^3 ABC", &mut HashMap::new()).unwrap();
        assert_eq!(p.degree_without_star(), 1);

        let p = Line::parse("AB AB^2 AB^3 ABC", &mut HashMap::new()).unwrap();
        assert_eq!(p.degree_without_star(), 7);
    }

    #[test]
    #[should_panic]
    fn convert_err() {
        let _ = Line::parse("AB (ABC)*. ABC", &mut HashMap::new()).unwrap();
    }

    #[test]
    fn line_inclusion() {
        let l1 = Line::parse("AB^3 AB^2 ABC", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("AB^2 AB^3 ABC", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), true);
        assert_eq!(l2.includes(&l1), true);

        let l1 = Line::parse("ABC^10 AB^5", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("AB^8 ABC^7", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), true);
        assert_eq!(l2.includes(&l1), false);

        let l1 = Line::parse("AB^5 ABC*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("AB^8 ABC*", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), true);
        assert_eq!(l2.includes(&l1), false);

        let l1 = Line::parse("AB^5 CD^3 ABC*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("AB^8 ABCD*", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), false);
        assert_eq!(l2.includes(&l1), false);

        let l1 = Line::parse("ABCDE CDE^3 AB^5 ABC*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("ABCDE ABCD* CDE^4", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), false);
        assert_eq!(l2.includes(&l1), false);

        let l1 = Line::parse("ABCDE CDE^3 AB^5 ABC*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("ABCDE ABCD* CDE^3", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), false);
        assert_eq!(l2.includes(&l1), true);

        let l1 = Line::parse("AB*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("ABC*", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), false);
        assert_eq!(l2.includes(&l1), true);

        let l1 = Line::parse("ABC AB*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("ABC BC*", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), false);
        assert_eq!(l2.includes(&l1), false);

        let l1 = Line::parse("AB*", &mut HashMap::new()).unwrap();
        let l2 = Line::parse("AB*", &mut HashMap::new()).unwrap();
        assert_eq!(l1.includes(&l2), true);
    }
}
