//! Joint synthesis of one or more original-edge-predicate MIS stages. The
//! mapping constraints are guarded by the selected edge predicates; decoded
//! graphs are rebuilt and verified by the ordinary certificate path.
use super::*;

pub(super) struct Symbolic {
    pub input: Input,
    pub pairs: Vec<[Label; 2]>,
    pub guards: BTreeMap<[usize; 2], Vec<(usize, bool)>>,
    pub activation: mapping::Activations,
}

pub(super) fn mis_graphs(
    p: &Problem,
    q: &Problem,
    stages: usize,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<Vec<Subgraph>>, String> {
    let Symbolic {
        input,
        pairs,
        guards,
        activation,
    } = symbolic(q, stages, budget, eh)?;
    let target = Input::new(p, budget, eh)?;
    let Some((_, chosen)) = mapping::find_guarded(
        &input,
        &target,
        &guards,
        stages * pairs.len(),
        Some(&activation),
        budget,
        eh,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(
        (0..stages)
            .map(|stage| {
                Subgraph::Pairs(
                    pairs
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &p)| chosen[stage * pairs.len() + i].then_some(p))
                        .collect(),
                )
            })
            .collect(),
    ))
}

pub(super) fn symbolic(
    q: &Problem,
    stages: usize,
    budget: &Budget,
    eh: &EventHandler,
) -> Result<Symbolic, String> {
    if !(1..=3).contains(&stages) {
        return Err("Unsupported synthesized MIS stage count".into());
    }
    let raw = Input::new(q, budget, eh)?;
    let pairs: Vec<_> = raw
        .edges
        .iter()
        .map(|&[a, b]| pair(raw.base[a], raw.base[b]))
        .collect();
    let indices: BTreeMap<_, _> = pairs.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let mut input = raw;
    let mut activation = mapping::Activations::default();
    for i in 0..stages * pairs.len() {
        activation
            .conditions
            .push(mapping::Condition::Parameter(i, false));
        activation
            .conditions
            .push(mapping::Condition::Parameter(i, true));
    }
    activation.roots = vec![None; input.nodes.len()];
    let mut guards = BTreeMap::<[usize; 2], Vec<(usize, bool)>>::new();
    for stage in 0..stages {
        budget.check(eh)?;
        let mut next = input.step(&Step::Mis(Subgraph::All), stage + 1, budget, eh)?;
        let mut next_guards = BTreeMap::new();
        let mut possible_parents = vec![vec![]; input.names.len()];
        for &[a, b] in &input.edges {
            let parameter = stage * pairs.len() + indices[&pair(input.base[a], input.base[b])];
            let mut requirements: Vec<_> = guards
                .get(&[a, b])
                .into_iter()
                .flatten()
                .map(|&(p, positive)| 2 * p + usize::from(positive))
                .collect();
            requirements.push(2 * parameter + 1);
            let condition = activation.conditions.len();
            activation
                .conditions
                .push(mapping::Condition::All(requirements));
            possible_parents[a].push(condition);
            possible_parents[b].push(condition);
            for (x, y, condition) in [
                (0, 0, Some(false)),
                (0, 1, None),
                (1, 0, None),
                (1, 1, None),
                (0, 2, Some(true)),
                (2, 0, Some(true)),
            ] {
                let e = annotations::edge(3 * a + x, 3 * b + y);
                let mut conditions = guards.get(&[a, b]).cloned().unwrap_or_default();
                if let Some(positive) = condition {
                    conditions.push((parameter, positive));
                }
                next.edges.insert(e);
                if !conditions.is_empty() {
                    next_guards.insert(e, conditions);
                }
                if next.edges.len() > 500_000 {
                    return Err(LIMIT.into());
                }
            }
        }
        let parent_conditions: Vec<_> = possible_parents
            .into_iter()
            .map(|mut possibilities| {
                possibilities.sort();
                possibilities.dedup();
                let condition = activation.conditions.len();
                activation
                    .conditions
                    .push(mapping::Condition::Any(possibilities));
                condition
            })
            .collect();
        let old_roots: BTreeMap<_, _> = input
            .nodes
            .iter()
            .cloned()
            .zip(activation.roots.iter().copied())
            .collect();
        let mut roots = vec![];
        for row in &next.nodes {
            let old: Vec<_> = row.iter().map(|&s| s / 3).collect();
            let mut requirements: Vec<_> = old_roots[&old].into_iter().collect();
            if let Some(&parent) = row.iter().find(|&&s| s % 3 == 2) {
                requirements.push(parent_conditions[parent / 3]);
            }
            let root = match requirements.len() {
                0 => None,
                1 => Some(requirements[0]),
                _ => {
                    let root = activation.conditions.len();
                    activation
                        .conditions
                        .push(mapping::Condition::All(requirements));
                    Some(root)
                }
            };
            roots.push(root);
        }
        activation.roots = roots;
        input = next;
        guards = next_guards;
    }
    Ok(Symbolic {
        input,
        pairs,
        guards,
        activation,
    })
}
