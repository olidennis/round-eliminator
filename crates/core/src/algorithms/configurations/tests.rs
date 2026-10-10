use super::*;
use crate::labels::LabelSet;
use crate::problem::Part;

#[test]
fn large_multiplicities_stay_counted_in_membership_and_enumeration() {
    let part = |labels: Vec<u8>, count| Part::new(LabelSet::new(labels), count).unwrap();
    let constraint = Constraint::new(vec![
        CondensedConfiguration::new(vec![part(vec![0, 1], u32::MAX - 1), part(vec![0], 1)])
            .unwrap(),
    ]);
    let concrete = Configuration::new(vec![(0, 1), (1, u32::MAX - 1)]).unwrap();
    assert!(accepts(&constraint, &concrete));
    let rejected = Constraint::new(vec![
        CondensedConfiguration::new(vec![part(vec![0], u32::MAX - 1), part(vec![1], 1)]).unwrap(),
    ]);
    assert!(!accepts(&rejected, &concrete));

    let singleton = Constraint::new(vec![
        CondensedConfiguration::new(vec![part(vec![0], u32::MAX)]).unwrap(),
    ]);
    assert_eq!(
        concrete_multisets(&singleton, &[u32::MAX]),
        vec![Configuration::new(vec![(0, u32::MAX)]).unwrap(),]
    );
    assert!(accepts(
        &singleton,
        &Configuration::new(vec![(0, u32::MAX)]).unwrap()
    ));
}

#[test]
fn membership_and_enumeration_respect_cancellation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let constraint = Constraint::<u8>::new(vec![]);
    assert!(accepts_with(&constraint, &Configuration::default(), &cancellation).is_err());
    assert!(concrete_multisets_with(&constraint, &[0], &cancellation).is_err());
}

#[test]
fn membership_agrees_with_explicit_expansion() {
    // Exercise overlapping parts, repeated counts, and the augmenting paths
    // needed when a greedy occurrence-to-part assignment gets stuck.
    for first in 1u8..8 {
        for second in 1u8..8 {
            for first_count in 1..=2 {
                for second_count in 1..=2 {
                    let set = |mask| {
                        LabelSet::new((0u8..3).filter(|label| mask & (1 << label) != 0).collect())
                    };
                    let configuration = CondensedConfiguration::new(vec![
                        Part::new(set(first), first_count).unwrap(),
                        Part::new(set(second), second_count).unwrap(),
                    ])
                    .unwrap();
                    let constraint = Constraint::new(vec![configuration]);
                    let degree = first_count + second_count;
                    let mut expanded = BTreeSet::new();
                    for mut code in 0usize..3usize.pow(degree) {
                        let mut tuple = Vec::new();
                        let mut allowed = true;
                        for position in 0..degree {
                            let label = (code % 3) as u8;
                            code /= 3;
                            let mask = if position < first_count {
                                first
                            } else {
                                second
                            };
                            allowed &= mask & (1 << label) != 0;
                            tuple.push(label);
                        }
                        if allowed {
                            tuple.sort_unstable();
                            expanded.insert(tuple);
                        }
                    }
                    assert_eq!(
                        concrete_multisets(&constraint, &[degree]),
                        expanded
                            .iter()
                            .map(|occurrences| Configuration::from_occurrences(
                                occurrences.iter().copied()
                            )
                            .unwrap())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                    );
                    for mut code in 0usize..3usize.pow(degree) {
                        let mut tuple = Vec::new();
                        for _ in 0..degree {
                            tuple.push((code % 3) as u8);
                            code /= 3;
                        }
                        let actual = accepts(
                            &constraint,
                            &Configuration::from_occurrences(tuple.iter().copied()).unwrap(),
                        );
                        tuple.sort_unstable();
                        assert_eq!(actual, expanded.contains(&tuple));
                    }
                    assert!(!accepts(
                        &constraint,
                        &Configuration::new(vec![(0, degree + 1)]).unwrap()
                    ));
                }
            }
        }
    }
}

#[test]
fn zero_degree_and_duplicate_configurations() {
    let empty = CondensedConfiguration::<u8>::new(vec![]).unwrap();
    let constraint = Constraint::new(vec![empty.clone(), empty]);
    assert!(accepts(&constraint, &Configuration::default()));
    assert_eq!(
        concrete_multisets(&constraint, &[0]),
        vec![Configuration::<u8>::default()]
    );
    assert!(concrete_multisets(&constraint, &[1]).is_empty());
    assert!(!accepts(
        &Constraint::<u8>::new(vec![]),
        &Configuration::default()
    ));
}
