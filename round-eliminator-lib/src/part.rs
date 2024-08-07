use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::group::{Group, GroupType, Label};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Part {
    pub gtype: GroupType,
    pub group: Group,
}

impl Part {
    pub fn to_string(&self, mapping: &HashMap<Label, String>) -> String {
        let mut s = String::new();
        for label in self.group.iter() {
            s.push_str(&mapping[label])
        }
        if s.is_empty() {
            s.push_str("∅");
        }
        s.push_str(&self.gtype.to_string());
        s
    }

    pub fn _to_string(&self, mapping: &HashMap<Label, String>) -> String {
        let mut s = String::new();
        let deg = self.gtype.value();
        for _ in 0..deg {
            for label in self.group.iter() {
                s.push_str(&mapping[label]);
            }
            s.push(' ');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        group::{Group, GroupType},
        part::Part,
    };

    #[test]
    fn valid_conversions() {
        let mut h = HashMap::new();
        let p = Part::parse("ABC", &mut h).unwrap();
        assert_eq!(
            Part {
                gtype: GroupType::ONE,
                group: Group::from(vec![0, 1, 2])
            },
            p
        );
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "ABC");

        let mut h = HashMap::new();
        let p = Part::parse("ABC^2", &mut h).unwrap();
        assert_eq!(
            Part {
                gtype: GroupType::Many(2),
                group: Group::from(vec![0, 1, 2])
            },
            p
        );
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "ABC^2");

        let mut h = HashMap::new();
        let p = Part::parse("ABC*", &mut h).unwrap();
        assert_eq!(
            Part {
                gtype: GroupType::Star,
                group: Group::from(vec![0, 1, 2])
            },
            p
        );
        let rh = h.into_iter().map(|(a, b)| (b, a)).collect();
        assert_eq!(p.to_string(&rh), "ABC*");
    }

    #[test]
    #[should_panic]
    fn convert_err_1() {
        let mut h = HashMap::new();
        let _ = Part::parse("(ABC", &mut h).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_2() {
        let _ = Part::parse("(ABC)*.", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_3() {
        let _ = Part::parse("(ABC)^123.", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_4() {
        let _ = Part::parse("AB(AB(C)^123", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_5() {
        let _ = Part::parse("AB)ABC^123", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_6() {
        let _ = Part::parse("AB(A*B)C^123", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_7() {
        let _ = Part::parse("AB(A^B)C^123", &mut HashMap::new()).unwrap();
    }

    #[test]
    #[should_panic]
    fn convert_err_8() {
        let _ = Part::parse("AB()C^123", &mut HashMap::new()).unwrap();
    }
}
