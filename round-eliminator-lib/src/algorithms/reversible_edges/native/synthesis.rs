//! Joint synthesis of one or more original-edge-predicate MIS stages. The
//! mapping constraints are guarded by the selected edge predicates; decoded
//! graphs are rebuilt and verified by the ordinary certificate path.
use super::*;

pub(super) fn mis_graphs(
    p: &Problem,
    q: &Problem,
    stages: usize,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<Vec<Subgraph>>, String> {
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
    let mut guards = BTreeMap::<[usize; 2], Vec<(usize, bool)>>::new();
    for stage in 0..stages {
        budget.check(eh)?;
        let mut next = input.step(&Step::Mis(Subgraph::All), stage + 1, budget, eh)?;
        let mut next_guards = BTreeMap::new();
        for &[a, b] in &input.edges {
            let parameter = stage * pairs.len() + indices[&pair(input.base[a], input.base[b])];
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
        input = next;
        guards = next_guards;
    }
    let target = Input::new(p, budget, eh)?;
    let Some((_, chosen)) =
        mapping::find_guarded(&input, &target, &guards, stages * pairs.len(), budget, eh)?
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
