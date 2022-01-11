use serde::{Deserialize, Serialize};

use crate::{algorithms::event::EventHandler, group::Label, line::Degree, problem::Problem};

fn fix_problem(new: &mut Problem, sort_by_strength: bool, eh: &mut EventHandler) {
    if new.passive.degree == Degree::Finite(2) {
        new.diagram_indirect = None;
        new.compute_diagram(eh);
        new.discard_useless_stuff(true, eh);
        if sort_by_strength {
            new.sort_active_by_strength();
        }
        new.compute_triviality(eh);
        new.compute_coloring_solvability(eh);
        if let Some(outdegree) = new.orientation_given {
            new.compute_triviality_given_orientation(outdegree, eh);
            new.compute_coloring_solvability_given_orientation(outdegree, eh);
        }
    } else {
        new.discard_useless_stuff(false, eh);
        if sort_by_strength {
            new.sort_active_by_strength();
        }
    }
}

pub fn request_json<F>(req: &str, f: F)
where
    F: Fn(String),
{
    let req: Request = serde_json::from_str(req).unwrap();
    let handler = |resp: Response| {
        let s = serde_json::to_string(&resp).unwrap();
        f(s);
    };

    let mut eh = EventHandler::with(|x: (String, usize, usize)| {
        let resp = Response::Event(x.0, x.1, x.2);
        handler(resp);
    });

    match req {
        Request::Ping => {
            handler(Response::Pong);
            return;
        }
        Request::NewProblem(active, passive) => {
            match Problem::from_string_active_passive(active, passive) {
                Ok(mut new) => {
                    fix_problem(&mut new, true, &mut eh);
                    handler(Response::P(new))
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::Speedup(mut problem) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.speedup(&mut eh);
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::InverseSpeedup(problem) => {
            if problem.active.degree == Degree::Star {
                handler(Response::E(
                    "Cannot perform inverse speedup if the active side contains a star.".into(),
                ));
            } else {
                let mut new = problem.inverse_speedup();
                fix_problem(&mut new, false, &mut eh);
                handler(Response::P(new));
            }
        }
        Request::SpeedupMaximize(mut problem) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.speedup(&mut eh);
            new.passive.maximize(&mut eh);
            new.compute_diagram(&mut eh);
            new.discard_useless_stuff(true, &mut eh);
            new.sort_active_by_strength();
            new.compute_triviality(&mut eh);
            if new.passive.degree == Degree::Finite(2) {
                new.compute_coloring_solvability(&mut eh);
                if let Some(outdegree) = new.orientation_given {
                    new.compute_triviality_given_orientation(outdegree, &mut eh);
                    new.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
                }
            }
            handler(Response::P(new));
        }
        Request::SpeedupMaximizeRenamegen(mut problem) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.speedup(&mut eh);
            new.passive.maximize(&mut eh);
            new.compute_diagram(&mut eh);
            new.discard_useless_stuff(true, &mut eh);
            new.sort_active_by_strength();
            new.compute_triviality(&mut eh);
            if new.passive.degree == Degree::Finite(2) {
                new.compute_coloring_solvability(&mut eh);
                if let Some(outdegree) = new.orientation_given {
                    new.compute_triviality_given_orientation(outdegree, &mut eh);
                    new.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
                }
            }
            match new.rename_by_generators() {
                Ok(()) => {
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::SimplifyMerge(problem, a, b) => {
            let mut new = problem.relax_merge(a, b);
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::SimplifyMergeGroup(problem, labels, to) => {
            let mut new = problem;
            for label in labels {
                new = new.relax_merge(label, to);
            }
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::SimplifyAddarrow(problem, a, b) => {
            let mut new = problem.relax_addarrow(a, b);
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::HardenRemove(mut problem, label, keep_predecessors) => {
            if keep_predecessors && problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.harden_remove(label, keep_predecessors);
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::HardenKeep(mut problem, labels, keep_predecessors) => {
            if keep_predecessors && problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.harden_keep(&labels.into_iter().collect(), keep_predecessors);
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::MergeEquivalentLabels(problem) => {
            let mut new = problem.merge_equivalent_labels();
            fix_problem(&mut new, true, &mut eh);
            handler(Response::P(new));
        }
        Request::Maximize(mut problem) => {
            problem.diagram_indirect = None;
            problem.passive.maximize(&mut eh);
            problem.compute_diagram(&mut eh);
            problem.discard_useless_stuff(true, &mut eh);
            problem.sort_active_by_strength();
            problem.compute_triviality(&mut eh);
            if problem.passive.degree == Degree::Finite(2) {
                problem.compute_coloring_solvability(&mut eh);
                if let Some(outdegree) = problem.orientation_given {
                    problem.compute_triviality_given_orientation(outdegree, &mut eh);
                    problem.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
                }
            }
            handler(Response::P(problem));
        }
        Request::RenameGenerators(mut problem) => match problem.rename_by_generators() {
            Ok(()) => {
                handler(Response::P(problem));
            }
            Err(s) => handler(Response::E(s.into())),
        },
        Request::Rename(mut problem, renaming) => match problem.rename(&renaming) {
            Ok(()) => handler(Response::P(problem)),
            Err(s) => handler(Response::E(s.into())),
        },
        Request::Orientation(mut problem, outdegree) => {
            problem.orientation_given = Some(outdegree);
            problem.orientation_coloring_sets = None;
            problem.orientation_trivial_sets = None;
            if problem.passive.degree == Degree::Finite(2) {
                problem.compute_triviality_given_orientation(outdegree, &mut eh);
                problem.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
            }
            handler(Response::P(problem));
        } //_ => { unimplemented!() }
    }

    handler(Response::Done);
}

#[derive(Deserialize, Serialize)]
pub enum Request {
    NewProblem(String, String),
    SimplifyMerge(Problem, Label, Label),
    SimplifyMergeGroup(Problem, Vec<Label>, Label),
    SimplifyAddarrow(Problem, Label, Label),
    HardenRemove(Problem, Label, bool),
    HardenKeep(Problem, Vec<Label>, bool),
    Speedup(Problem),
    InverseSpeedup(Problem),
    SpeedupMaximize(Problem),
    SpeedupMaximizeRenamegen(Problem),
    Maximize(Problem),
    MergeEquivalentLabels(Problem),
    RenameGenerators(Problem),
    Rename(Problem, Vec<(Label, String)>),
    Orientation(Problem, usize),
    Ping,
}

#[allow(clippy::large_enum_variant)]
#[derive(Deserialize, Serialize)]
pub enum Response {
    Done,
    Pong,
    Event(String, usize, usize),
    P(Problem),
    E(String),
}
