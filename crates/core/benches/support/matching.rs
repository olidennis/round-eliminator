//! LLM: Reproducible fixtures and alternatives for configuration membership.
//! LLM: The occurrence matcher is the implementation used before counted configurations.

use petgraph::{
    Graph,
    algo::{dinics, ford_fulkerson},
};
use round_eliminator_3::{
    algorithms::configurations::accepts,
    computation::Cancellation,
    model::{CondensedConfiguration, Configuration, Constraint, LabelSet, Part},
};

/// LLM: A named benchmark input together with the fixture data and expected behavior needed to
/// LLM: compare implementations reproducibly.
pub struct Case {
    pub name: &'static str,
    pub constraint: Constraint<u32>,
    pub concrete: Configuration<u32>,
    pub occurrences: Vec<u32>,
    pub expected: bool,
}

/// LLM: Construct a named matching fixture with offered counts, target capacities, and
/// LLM: compatibility data. Used to feed the same instance to each benchmark matcher.
fn case(
    name: &'static str,
    parts: Vec<(Vec<u32>, u32)>,
    counts: Vec<(u32, u32)>,
    expected: bool,
) -> Case {
    // LLM: Prepare `concrete` for the following computation; it stores the concrete for this
    // LLM: stage.
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

/// LLM: List the representative matching workloads shared by the benchmark implementations.
/// LLM: Used to keep library and custom-matcher comparisons on equivalent inputs.
pub fn cases() -> Vec<Case> {
    // LLM: Prepare `cases` for the following computation; it stores the cases for this stage.
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
    // LLM: Process the relevant label choices while preserving their table identities.
    for (name, size) in [
        ("16 distinct, dense", 16),
        ("64 distinct, dense", 64),
        ("256 distinct, dense", 256),
    ] {
        // LLM: Retain the newly constructed choices in the result for this stage.
        cases.push(case(
            name,
            (0..size).map(|_| ((0..size).collect(), 1)).collect(),
            (0..size).map(|label| (label, 1)).collect(),
            true,
        ));
    }
    // LLM: Prepare `retry` for the following computation; it stores the retry for this stage.
    let mut retry = case(
        "degree 10, union retry",
        vec![(vec![0, 1], 5), (vec![2, 3], 5)],
        vec![(0, 4), (1, 2), (2, 2), (3, 2)],
        true,
    );
    let mut configurations: Vec<_> = retry.constraint.configurations().cloned().collect();
    // LLM: Retain the newly constructed choices in the result for this stage.
    configurations.push(
        CondensedConfiguration::new(vec![
            Part::new(LabelSet::new(vec![0, 1, 2, 3]), 10).unwrap(),
        ])
        .unwrap(),
    );
    retry.constraint = Constraint::new(configurations);
    cases.push(retry);
    // LLM: Update the current working state before continuing this operation.
    cases
}

pub const ALGORITHMS: [&str; 4] = ["original", "selected", "dinic", "edmonds-karp"];

/// LLM: Evaluate the prepared benchmark or operation fixture and return its result data. Used
/// LLM: by this module's harness rather than the application protocol.
pub fn run(case: &Case, algorithm: usize) -> bool {
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    match algorithm {
        0 => original(case),
        1 => accepts(&case.constraint, &case.concrete),
        2 => flow(case, true),
        3 => flow(case, false),
        _ => unreachable!(),
    }
}

/// LLM: Build and solve the fixture's counted flow network using the selected library
/// LLM: algorithm. Used to compare Dinic and Edmonds–Karp against the original matcher.
fn flow(case: &Case, use_dinic: bool) -> bool {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = Cancellation::default();
    for configuration in case
        .constraint
        .configurations()
        .filter(|configuration| configuration.degree() == case.concrete.degree())
    {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check().unwrap();
        let mut network = Graph::<(), u32>::new();
        let source = network.add_node(());
        let sink = network.add_node(());
        // LLM: Prepare `destinations` for the following computation; it stores the
        // LLM: destinations for this stage.
        let destinations: Vec<_> = configuration
            .parts()
            .iter()
            .map(|part| {
                let node = network.add_node(());
                network.add_edge(node, sink, part.multiplicity());
                node
            })
            .collect();
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for &(label, count) in case.concrete.multiplicities() {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check().unwrap();
            let node = network.add_node(());
            network.add_edge(source, node, count);
            for (part, &destination) in configuration.parts().iter().zip(&destinations) {
                if part.labels().contains(label) {
                    network.add_edge(node, destination, count.min(part.multiplicity()));
                }
            }
        }
        // LLM: Compute exact maximum flow and compare it with the required total occurrence
        // LLM: count.
        let total = if use_dinic {
            dinics(&network, source, sink).0
        } else {
            ford_fulkerson(&network, source, sink).0
        };
        cancellation.check().unwrap();
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if total == case.concrete.degree() {
            return true;
        }
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    false
}

/// LLM: Run the original occurrence-based matcher for a benchmark fixture. Used as the
/// LLM: historical comparison rather than the production matching implementation.
fn original(case: &Case) -> bool {
    // LLM: Share the stop flag with the work performed in this stage.
    let cancellation = Cancellation::default();
    for configuration in case
        .constraint
        .configurations()
        .filter(|configuration| configuration.degree() as usize == case.occurrences.len())
    {
        // LLM: Prepare `occupants` for the following computation; it stores the occupants for
        // LLM: this stage.
        let mut occupants = vec![Vec::new(); configuration.parts().len()];
        if (0..case.occurrences.len()).all(|occurrence| {
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
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
    // LLM: Return the result of this completed search or conversion to its caller.
    false
}

/// LLM: Search an augmenting assignment path in the original benchmark matcher. Used only to
/// LLM: compare the previous custom matcher with flow-library alternatives.
fn augment(
    occurrence: usize,
    occurrences: &[u32],
    parts: &[Part<u32>],
    occupants: &mut [Vec<usize>],
    visited: &mut [bool],
    cancellation: &Cancellation,
) -> bool {
    // LLM: Observe cooperative interruption before continuing this unit of work.
    cancellation.check().unwrap();
    for (part, choices) in parts.iter().enumerate() {
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if visited[part] || !choices.labels().contains(occurrences[occurrence]) {
            continue;
        }
        visited[part] = true;
        if occupants[part].len() < choices.multiplicity() as usize {
            occupants[part].push(occurrence);
            return true;
        }
        // LLM: Process each condensed part without expanding its repeated occurrences.
        for slot in 0..occupants[part].len() {
            // LLM: Prepare `displaced` for the following computation; it stores the displaced
            // LLM: for this stage.
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
    // LLM: Return the result of this completed search or conversion to its caller.
    false
}
