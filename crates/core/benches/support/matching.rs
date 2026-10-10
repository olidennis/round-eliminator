//! Reproducible fixtures and alternatives for configuration membership.
//! The occurrence matcher is the implementation used before counted configurations.

use petgraph::{
    Graph,
    algo::{dinics, ford_fulkerson},
};
use round_eliminator_3::{
    algorithms::configurations::accepts,
    computation::Cancellation,
    labels::LabelSet,
    problem::{CondensedConfiguration, Configuration, Constraint, Part},
};

pub struct Case {
    pub name: &'static str,
    pub constraint: Constraint<u32>,
    pub concrete: Configuration<u32>,
    pub occurrences: Vec<u32>,
    pub expected: bool,
}

fn case(
    name: &'static str,
    parts: Vec<(Vec<u32>, u32)>,
    counts: Vec<(u32, u32)>,
    expected: bool,
) -> Case {
    let concrete = Configuration::new(counts).unwrap();
    Case {
        name,
        constraint: Constraint::new(vec![
            CondensedConfiguration::new(
                parts
                    .into_iter()
                    .map(|(labels, count)| Part::new(LabelSet::new(labels), count).unwrap())
                    .collect(),
            )
            .unwrap(),
        ]),
        occurrences: concrete.occurrences().collect(),
        concrete,
        expected,
    }
}

pub fn cases() -> Vec<Case> {
    let mut cases = vec![
        case(
            "degree 2, reassignment",
            vec![(vec![0, 1], 1), (vec![0], 1)],
            vec![(0, 1), (1, 1)],
            true,
        ),
        case(
            "degree 3, overlapping",
            vec![(vec![0, 1], 1), (vec![0, 2], 1), (vec![1, 2], 1)],
            vec![(0, 1), (1, 1), (2, 1)],
            true,
        ),
        case(
            "degree 10, overlapping",
            vec![(vec![0, 1], 5), (vec![0, 2], 5)],
            vec![(0, 4), (1, 3), (2, 3)],
            true,
        ),
        case(
            "degree 10, rejected",
            vec![(vec![0, 1], 5), (vec![2, 3], 5)],
            vec![(0, 4), (1, 2), (2, 2), (3, 2)],
            false,
        ),
        case(
            "degree 50, overlapping",
            vec![(vec![0, 1], 25), (vec![0, 2], 25)],
            vec![(0, 20), (1, 15), (2, 15)],
            true,
        ),
        case(
            "degree 1000, overlapping",
            vec![(vec![0, 1], 500), (vec![0, 2], 500)],
            vec![(0, 400), (1, 300), (2, 300)],
            true,
        ),
        case(
            "degree 10000, overlapping",
            vec![(vec![0, 1], 5000), (vec![0, 2], 5000)],
            vec![(0, 4000), (1, 3000), (2, 3000)],
            true,
        ),
        case(
            "degree 1000, broad part",
            vec![(vec![0, 1, 2, 3], 1000)],
            vec![(0, 250), (1, 250), (2, 250), (3, 250)],
            true,
        ),
    ];
    for (name, size) in [
        ("16 distinct, dense", 16),
        ("64 distinct, dense", 64),
        ("256 distinct, dense", 256),
    ] {
        cases.push(case(
            name,
            (0..size).map(|_| ((0..size).collect(), 1)).collect(),
            (0..size).map(|label| (label, 1)).collect(),
            true,
        ));
    }
    let mut retry = case(
        "degree 10, union retry",
        vec![(vec![0, 1], 5), (vec![2, 3], 5)],
        vec![(0, 4), (1, 2), (2, 2), (3, 2)],
        true,
    );
    let mut configurations: Vec<_> = retry.constraint.configurations().cloned().collect();
    configurations.push(
        CondensedConfiguration::new(vec![
            Part::new(LabelSet::new(vec![0, 1, 2, 3]), 10).unwrap(),
        ])
        .unwrap(),
    );
    retry.constraint = Constraint::new(configurations);
    cases.push(retry);
    cases
}

pub const ALGORITHMS: [&str; 4] = ["original", "selected", "dinic", "edmonds-karp"];

pub fn run(case: &Case, algorithm: usize) -> bool {
    match algorithm {
        0 => original(case),
        1 => accepts(&case.constraint, &case.concrete),
        2 => flow(case, true),
        3 => flow(case, false),
        _ => unreachable!(),
    }
}

fn flow(case: &Case, use_dinic: bool) -> bool {
    let cancellation = Cancellation::default();
    for configuration in case
        .constraint
        .configurations()
        .filter(|configuration| configuration.degree() == case.concrete.degree())
    {
        cancellation.check().unwrap();
        let mut network = Graph::<(), u32>::new();
        let source = network.add_node(());
        let sink = network.add_node(());
        let destinations: Vec<_> = configuration
            .parts()
            .iter()
            .map(|part| {
                let node = network.add_node(());
                network.add_edge(node, sink, part.multiplicity());
                node
            })
            .collect();
        for &(label, count) in case.concrete.multiplicities() {
            cancellation.check().unwrap();
            let node = network.add_node(());
            network.add_edge(source, node, count);
            for (part, &destination) in configuration.parts().iter().zip(&destinations) {
                if part.labels().contains(label) {
                    network.add_edge(node, destination, count.min(part.multiplicity()));
                }
            }
        }
        let total = if use_dinic {
            dinics(&network, source, sink).0
        } else {
            ford_fulkerson(&network, source, sink).0
        };
        cancellation.check().unwrap();
        if total == case.concrete.degree() {
            return true;
        }
    }
    false
}

fn original(case: &Case) -> bool {
    let cancellation = Cancellation::default();
    for configuration in case
        .constraint
        .configurations()
        .filter(|configuration| configuration.degree() as usize == case.occurrences.len())
    {
        let mut occupants = vec![Vec::new(); configuration.parts().len()];
        if (0..case.occurrences.len()).all(|occurrence| {
            augment(
                occurrence,
                &case.occurrences,
                configuration.parts(),
                &mut occupants,
                &mut vec![false; configuration.parts().len()],
                &cancellation,
            )
        }) {
            return true;
        }
    }
    false
}

fn augment(
    occurrence: usize,
    occurrences: &[u32],
    parts: &[Part<u32>],
    occupants: &mut [Vec<usize>],
    visited: &mut [bool],
    cancellation: &Cancellation,
) -> bool {
    cancellation.check().unwrap();
    for (part, choices) in parts.iter().enumerate() {
        if visited[part] || !choices.labels().contains(occurrences[occurrence]) {
            continue;
        }
        visited[part] = true;
        if occupants[part].len() < choices.multiplicity() as usize {
            occupants[part].push(occurrence);
            return true;
        }
        for slot in 0..occupants[part].len() {
            let displaced = occupants[part][slot];
            if augment(
                displaced,
                occurrences,
                parts,
                occupants,
                visited,
                cancellation,
            ) {
                occupants[part][slot] = occurrence;
                return true;
            }
        }
    }
    false
}
