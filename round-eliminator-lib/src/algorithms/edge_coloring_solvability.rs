use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use crate::{
    group::{Group, GroupType, Label},
    line::{Degree, Line},
    part::Part,
    problem::{EdgeColoringSolvability, Problem},
};

use super::{coloring_sat::SatEngine, event::EventHandler};

fn line_from_sets(sets: &[&Vec<Label>]) -> Line {
    Line {
        parts: sets
            .iter()
            .map(|set| Part {
                gtype: GroupType::ONE,
                group: Group::from((*set).clone()),
            })
            .collect(),
    }
}

fn line_from_labels(labels: &[Label]) -> Line {
    Line {
        parts: labels
            .iter()
            .map(|&label| Part {
                gtype: GroupType::ONE,
                group: Group::from(vec![label]),
            })
            .collect(),
    }
}

fn active_allows(problem: &Problem, sets: &[&Vec<Label>]) -> bool {
    let line = line_from_sets(sets);
    // An active node can pick one label from each incident color's set.
    problem.active.is_included_with_custom_supersets(
        &line,
        Some(|allowed: &Group, offered: &Group| !allowed.intersection(offered).is_empty()),
    )
}

pub(super) fn passive_allows(problem: &Problem, sets: &[&Vec<Label>]) -> bool {
    // Every combination of labels that neighboring active nodes might choose
    // must be accepted by the passive constraint.
    if problem.passive.is_maximized || problem.passive.degree == Degree::Finite(2) {
        return problem.passive.includes(&line_from_sets(sets));
    }
    fn check_choices(
        problem: &Problem,
        sets: &[&Vec<Label>],
        position: usize,
        chosen: &mut Vec<Label>,
    ) -> bool {
        if position == sets.len() {
            let line = Line {
                parts: chosen
                    .iter()
                    .map(|&label| Part {
                        gtype: GroupType::ONE,
                        group: Group::from(vec![label]),
                    })
                    .collect(),
            };
            return problem.passive.includes_single_line(&line);
        }
        for &label in sets[position].iter() {
            chosen.push(label);
            let allowed = check_choices(problem, sets, position + 1, chosen);
            chosen.pop();
            if !allowed {
                return false;
            }
        }
        true
    }
    check_choices(problem, sets, 0, &mut Vec::new())
}

/// Each profile is one permutation of an active line, with one allowed-label
/// group per incident input color. A witness may choose any label in its group.
fn active_profiles(problem: &Problem, labels: &[Label], degree: usize) -> Vec<Vec<Vec<usize>>> {
    let indices: HashMap<_, _> = labels.iter().enumerate().map(|(i, &l)| (l, i)).collect();
    let mut profiles = HashSet::new();
    for line in &problem.active.lines {
        let mut groups = Vec::with_capacity(degree);
        for part in &line.parts {
            let GroupType::Many(copies) = part.gtype else {
                unreachable!()
            };
            let group: Vec<_> = part
                .group
                .iter()
                .filter_map(|label| indices.get(label).copied())
                .collect();
            for _ in 0..copies {
                groups.push(group.clone());
            }
        }
        for profile in groups.into_iter().permutations(degree) {
            if profile.iter().all(|group| !group.is_empty()) {
                profiles.insert(profile);
            }
        }
    }
    profiles.into_iter().collect()
}

fn add_active_requirement(
    sat: &mut SatEngine,
    color_vars: &[Vec<i32>],
    colors: &[usize],
    profiles: &[Vec<Vec<usize>>],
) -> Result<(), &'static str> {
    let mut witnesses = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let witness = sat.variable();
        witnesses.push(witness);
        for (position, group) in profile.iter().enumerate() {
            sat.clause(
                std::iter::once(-witness)
                    .chain(group.iter().map(|&i| color_vars[colors[position]][i])),
            )?;
        }
    }
    sat.clause(witnesses)
}

fn add_passive_requirements(
    problem: &Problem,
    sat: &mut SatEngine,
    color_vars: &[Vec<i32>],
    labels: &[Label],
    passive_degree: usize,
) -> Result<(), &'static str> {
    // At a passive node with degree greater than two, incident input colors
    // are distinct. Forbid each invalid tuple of output labels at each such
    // input palette. This is the universal quantifier in CNF form.
    for colors in (0..color_vars.len()).combinations(passive_degree) {
        for choice in std::iter::repeat(0..labels.len())
            .take(passive_degree)
            .multi_cartesian_product()
        {
            let outputs: Vec<_> = choice.iter().map(|&i| labels[i]).collect();
            if !problem
                .passive
                .includes_single_line(&line_from_labels(&outputs))
            {
                sat.clause(
                    colors
                        .iter()
                        .zip(choice.iter())
                        .map(|(&c, &i)| -color_vars[c][i]),
                )?;
            }
        }
    }
    Ok(())
}

fn solve_palette(
    problem: &Problem,
    labels: &[Label],
    active_degree: usize,
    passive_degree: usize,
    colors: usize,
    eh: &mut EventHandler,
) -> Result<Option<Vec<Vec<Label>>>, &'static str> {
    let shared_edge_color = passive_degree == 2;
    let mut sat = SatEngine::new();
    let color_vars: Vec<Vec<_>> = (0..colors)
        .map(|_| (0..labels.len()).map(|_| sat.variable()).collect())
        .collect();
    for row in &color_vars {
        sat.clause(row.iter().copied())?;
    }

    let mut compatible = Vec::new();
    if shared_edge_color {
        compatible = labels
            .iter()
            .map(|&a| {
                labels
                    .iter()
                    .map(|&b| {
                        problem
                            .passive
                            .includes_single_line(&line_from_labels(&[a, b]))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for row in &color_vars {
            for i in 0..labels.len() {
                for j in i..labels.len() {
                    if !compatible[i][j] {
                        sat.clause([-row[i], -row[j]])?;
                    }
                }
            }
        }
    } else {
        add_passive_requirements(problem, &mut sat, &color_vars, labels, passive_degree)?;
    }

    let profiles = active_profiles(problem, labels, active_degree);
    let palettes: Vec<_> = (0..colors).combinations(active_degree).collect();
    let mut added = HashSet::new();
    loop {
        if eh.is_cancelled() {
            return Err("Edge coloring solvability cancelled");
        }
        eh.notify("edge coloring SAT search", added.len(), palettes.len());
        let Some(model) = sat.solve()? else {
            return Ok(None);
        };
        let mut output_sets: Vec<Vec<_>> = color_vars
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .filter_map(|(i, &lit)| model[(lit - 1) as usize].then_some(labels[i]))
                    .collect()
            })
            .collect();
        if shared_edge_color {
            // Any clique may be enlarged without hurting passive validity.
            // Check the strongest extension of the SAT model before adding an
            // active constraint; this avoids enumerating maximal cliques.
            for set in &mut output_sets {
                for (i, &label) in labels.iter().enumerate() {
                    if !set.contains(&label)
                        && set.iter().all(|&old| {
                            let j = labels.binary_search(&old).unwrap();
                            compatible[i][j]
                        })
                        && compatible[i][i]
                    {
                        set.push(label);
                    }
                }
                set.sort_unstable();
            }
        }
        let failing = palettes.iter().find(|palette| {
            let sets: Vec<_> = palette.iter().map(|&c| &output_sets[c]).collect();
            !active_allows(problem, &sets)
        });
        match failing {
            None => return Ok(Some(output_sets)),
            Some(palette) => {
                if !added.insert(palette.clone()) {
                    return Err("SAT active-constraint encoding disagrees with validation");
                }
                add_active_requirement(&mut sat, &color_vars, palette, &profiles)?;
            }
        }
    }
}

impl Problem {
    /// Find the largest palette that permits a deterministic zero-round solution.
    ///
    /// With passive degree two, an input color belongs to the original edge and
    /// is shared by its two incidences. For other passive degrees, input colors
    /// belong to the edges of the bipartite active/passive graph and are proper
    /// at both ends.
    ///
    /// For each input color, collect every output label the active algorithm
    /// might use on an incidence of that color. Every active palette must
    /// admit one valid choice from its color sets, and every passive palette
    /// must accept all choices from its color sets. These conditions are also
    /// sufficient: choose one valid active configuration for each local input
    /// palette. SAT searches directly over membership of labels in each
    /// color's set; it does not enumerate the possible sets.
    pub fn compute_edge_coloring_solvability(
        &mut self,
        eh: &mut EventHandler,
    ) -> Result<(), &'static str> {
        if self.edge_coloring_solvability.is_some() {
            return Err("Edge coloring solvability has already been computed");
        }
        let (active_degree, passive_degree) = match (self.active.degree, self.passive.degree) {
            (Degree::Finite(a), Degree::Finite(p)) if a > 0 && p > 0 => (a, p),
            _ => {
                return Err(
                    "Edge coloring solvability requires finite, positive degrees on both sides",
                )
            }
        };
        let shared_edge_color = passive_degree == 2;
        let minimum_colors = if shared_edge_color {
            active_degree
        } else {
            active_degree.max(passive_degree)
        };
        eh.notify("edge coloring solvability", 0, 0);

        // If one behavior works at every port, repeat it for any palette.
        // It suffices to inspect minimal label sets from active choices:
        // shrinking a valid passive set cannot invalidate the forall check.
        for set in self.active.minimal_sets_of_all_choices() {
            let set: Vec<_> = set.into_iter().sorted().collect();
            if passive_allows(self, &vec![&set; passive_degree]) {
                self.edge_coloring_solvability = Some(EdgeColoringSolvability {
                    maximum: 0,
                    minimum: minimum_colors,
                    unbounded: true,
                    color_sets: vec![set; minimum_colors],
                });
                return Ok(());
            }
        }

        let labels: Vec<_> = self
            .active
            .labels_appearing()
            .into_iter()
            .sorted()
            .collect();
        let mut best = Vec::new();
        for colors in minimum_colors.. {
            eh.notify("edge coloring solvability", colors, 0);
            match solve_palette(self, &labels, active_degree, passive_degree, colors, eh)? {
                Some(sets) => best = sets,
                None => break,
            }
        }
        self.edge_coloring_solvability = Some(EdgeColoringSolvability {
            maximum: best.len(),
            minimum: minimum_colors,
            unbounded: false,
            color_sets: best,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{algorithms::event::EventHandler, problem::Problem};
    use itertools::Itertools;

    #[test]
    fn shared_color_on_graph_edges() {
        // A proper two-edge coloring lets every node select the same color
        // as its A edge. Three input colors would require two equal choices.
        let mut p = Problem::from_string("A B\n\nA A\nB B").unwrap();
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        let result = p.edge_coloring_solvability.unwrap();
        assert_eq!(result.maximum, 2);
        assert!(!result.unbounded);
        assert_eq!(result.color_sets.len(), 2);
    }

    #[test]
    fn palette_can_be_larger_than_the_active_degree() {
        let mut p = Problem::from_string("A B\nA C\nB C\n\nA A\nB B\nC C").unwrap();
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        assert_eq!(p.edge_coloring_solvability.unwrap().maximum, 3);
    }

    #[test]
    fn bipartite_incidence_colors_are_distinct_at_passive_nodes() {
        // The passive node sees three distinct colors and can require one A
        // incidence and two B incidences.
        let mut p = Problem::from_string("A\nB\n\nA B B").unwrap();
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        let result = p.edge_coloring_solvability.unwrap();
        assert_eq!(result.maximum, 3);
        assert!(!result.unbounded);
    }

    #[test]
    fn repeated_color_behavior_can_be_unbounded() {
        let mut p = Problem::from_string("A A\n\nA A A").unwrap();
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        assert!(p.edge_coloring_solvability.unwrap().unbounded);
    }

    #[test]
    fn no_feasible_graph_edge_palette() {
        let mut p = Problem::from_string("A B\n\nA B").unwrap();
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        let result = p.edge_coloring_solvability.unwrap();
        assert_eq!(result.maximum, 0);
        assert!(result.color_sets.is_empty());
    }

    #[test]
    fn seven_edge_coloring_on_degree_four_bipartite_graph() {
        let labels = ["A", "B", "C", "D", "E", "F", "G"];
        let distinct_lines = labels
            .iter()
            .combinations(4)
            .map(|tuple| tuple.into_iter().join(" "))
            .join("\n");
        let mut p = Problem::from_string(format!("{distinct_lines}\n\n{distinct_lines}")).unwrap();
        crate::serial::fix_problem(&mut p, true, true, &mut EventHandler::null());
        p.compute_edge_coloring_solvability(&mut EventHandler::null())
            .unwrap();
        let result = p.edge_coloring_solvability.unwrap();
        assert_eq!(result.minimum, 4);
        assert_eq!(result.maximum, 7);
        assert_eq!(result.color_sets.len(), 7);
    }

    #[test]
    #[ignore = "exact generic SAT search on 98 labels is intentionally expensive"]
    fn graph_seven_edge_coloring_after_two_speedups() {
        let labels = ["A", "B", "C", "D", "E", "F", "G"];
        let active = labels
            .iter()
            .combinations(4)
            .map(|tuple| tuple.into_iter().join(" "))
            .join("\n");
        let passive = labels
            .iter()
            .map(|label| format!("{label} {label}"))
            .join("\n");
        let mut p = Problem::from_string(format!("{active}\n\n{passive}")).unwrap();
        let mut eh = EventHandler::null();
        for step in 1..=2 {
            p = p.speedup(&mut eh);
            assert_eq!(p.labels().len(), if step == 1 { 7 } else { 98 });
        }
        p.compute_edge_coloring_solvability(&mut eh).unwrap();
        let result = p.edge_coloring_solvability.unwrap();
        assert!(result.maximum >= 8);
        assert_eq!(result.maximum, result.color_sets.len());
    }
}
