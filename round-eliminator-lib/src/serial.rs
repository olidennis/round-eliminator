use std::{collections::HashMap, time::Duration};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{algorithms::{event::EventHandler, fixpoint::FixpointType}, group::Label, line::Degree, problem::Problem};

pub fn fix_problem(new: &mut Problem, sort_by_strength: bool, compute_triviality_and_coloring : bool, eh: &mut EventHandler) {
    if new.passive.degree == Degree::Finite(2) {
        new.diagram_indirect = None;
        new.compute_diagram(eh);
        new.discard_useless_stuff(true, eh);
        if sort_by_strength {
            new.sort_active_by_strength();
        }
        if compute_triviality_and_coloring {
            new.compute_triviality(eh);
            new.compute_coloring_solvability(eh);
            if let Some(outdegree) = new.orientation_given {
                new.orientation_trivial_sets = None;
                new.compute_triviality_given_orientation(outdegree, eh);
                //new.compute_coloring_solvability_given_orientation(outdegree, eh);
            }
        }
    } else {
        new.discard_useless_stuff(false, eh);
        if sort_by_strength {
            new.sort_active_by_strength();
        }
    }
    //new.randomized_success_prob(eh);
    new.compute_passive_gen();
}

pub fn maximize_rename_gen(new : &mut Problem, eh : &mut EventHandler) -> Result<(), &'static str> {
    new.passive.maximize(eh);
    new.compute_diagram(eh);
    new.discard_useless_stuff(true, eh);
    new.sort_active_by_strength();
    new.compute_triviality(eh);
    if new.passive.degree == Degree::Finite(2) {
        new.compute_coloring_solvability(eh);
        if let Some(outdegree) = new.orientation_given {
            new.compute_triviality_given_orientation(outdegree, eh);
            //new.compute_coloring_solvability_given_orientation(outdegree, eh);
        }
    }
    new.compute_passive_gen();
    new.rename_by_generators()
}

#[cfg(not(target_arch = "wasm32"))]
pub trait SyncOnlyNonWasm : Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T> SyncOnlyNonWasm for T where T : Sync {}
#[cfg(target_arch = "wasm32")]
pub trait SyncOnlyNonWasm {}
#[cfg(target_arch = "wasm32")]
impl<T> SyncOnlyNonWasm for T{}

#[cfg(not(target_arch = "wasm32"))]
pub trait SendOnlyNonWasm : Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T> SendOnlyNonWasm for T where T : Send {}
#[cfg(target_arch = "wasm32")]
pub trait SendOnlyNonWasm {}
#[cfg(target_arch = "wasm32")]
impl<T> SendOnlyNonWasm for T{}


pub fn request_json<F>(req: &str, f: F)
where
    F: Fn(String, bool) + SyncOnlyNonWasm,
{
    let req: Request = serde_json::from_str(req).unwrap();
    let handler = |resp: Response| {
        let s = serde_json::to_string(&resp).unwrap();
        f(s, true);
    };

    let mut eh = EventHandler::with(move |x: (String, usize, usize)| {
        let resp = Response::Event(x.0, x.1, x.2);
        handler(resp);
    });

    let handler_ignore = |resp: Response| {
        let s = serde_json::to_string(&resp).unwrap();
        f(s, false);
    };

    let mut eh_ignore = EventHandler::with(|x: (String, usize, usize)| {
        let resp = Response::Event(x.0, x.1, x.2);
        handler_ignore(resp);
    });

    match req {
        Request::Ping => {
            handler(Response::Pong);
            return;
        }
        Request::NewProblem(active, passive) => {
            match Problem::from_string_active_passive(active, passive) {
                Ok((mut new, missing_labels)) => {
                    if missing_labels {
                        handler(Response::W("Some labels appear on only one side!".into()));
                    }
                    fix_problem(&mut new, true, true,&mut eh);
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
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::FixpointBasic(mut problem, partial, triviality_only, sublabels) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            match problem.fixpoint_generic(if partial {Some(sublabels)} else {None},FixpointType::Basic,triviality_only,&mut eh) {
                Ok((mut new,_,_)) => {
                    fix_problem(&mut new, true, true, &mut eh);
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::FixpointLoop(mut problem, partial, triviality_only, sublabels) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            match problem.fixpoint_generic(if partial {Some(sublabels)} else {None},FixpointType::Loop,triviality_only,&mut eh) {
                Ok((mut new,_,_)) => {
                    fix_problem(&mut new, true, true, &mut eh);
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::FixpointCustom(mut problem, diagram, partial, triviality_only, sublabels) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            match problem.fixpoint_generic(if partial {Some(sublabels)} else {None}, FixpointType::Custom(diagram),triviality_only,&mut eh) {
                Ok((mut new,_,_)) => {
                    fix_problem(&mut new, true, true, &mut eh);
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::FixpointDup(mut problem, dups, partial, triviality_only, sublabels, track) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            match problem.fixpoint_generic(if partial {Some(sublabels)} else {None},FixpointType::Dup(dups, track),triviality_only,&mut eh) {
                Ok((mut new,_,_)) => {
                    fix_problem(&mut new, true, true, &mut eh);
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::InverseSpeedup(problem) => {
            if problem.active.degree == Degree::Star {
                handler(Response::E(
                    "Cannot perform inverse speedup if the active side contains a star.".into(),
                ));
            } else {
                let mut new = problem.inverse_speedup();
                if new.active.degree != Degree::Finite(1) {
                    new.trivial_sets = Some(vec![]);
                }
                fix_problem(&mut new, false, false, &mut eh);
                handler(Response::P(new));
            }
        }
        Request::AllDifferentLabels(problem) => {
            if problem.active.degree == Degree::Star {
                handler(Response::E(
                    "Cannot perform this operation if the active side contains a star.".into(),
                ));
            } else {
                let mut new = problem.inverse_speedup();
                let active = new.active;
                let passive = new.passive;
                new.active = passive;
                new.passive = active;
                fix_problem(&mut new, false, false, &mut eh);
                handler(Response::P(new));
            }
        }
        Request::DeltaEdgeColoring(problem) => {
            if problem.active.degree == Degree::Star {
                handler(Response::E(
                    "Cannot perform this operation if the active side contains a star.".into(),
                ));
            } else {
                let mut new = problem.duplicate_labels_delta_edge_coloring();
                fix_problem(&mut new, false, false, &mut eh);
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
                    //new.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
                }
            }
            new.compute_passive_gen();
            handler(Response::P(new));
        }
        Request::SpeedupMaximizeRenamegen(mut problem) => {
            if problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.speedup(&mut eh);
            match maximize_rename_gen(&mut new, &mut eh) {
                Ok(()) => {
                    handler(Response::P(new));
                }
                Err(s) => handler(Response::E(s.into())),
            }
        }
        Request::SimplifyMerge(problem, a, b) => {
            let mut new = problem.relax_merge(a, b);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::SimplifyMergeGroup(problem, labels, to) => {
            let mut new = problem.relax_merge_group(&labels,to);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::SimplifyAddarrow(problem, a, b) => {
            let mut new = problem.relax_addarrow(a, b);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::HardenRemove(mut problem, label, keep_predecessors) => {
            if keep_predecessors && problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.harden_remove(label, keep_predecessors);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::HardenKeep(mut problem, labels, keep_predecessors) => {
            if keep_predecessors && problem.diagram_indirect.is_none() {
                problem.compute_partial_diagram(&mut eh);
            }
            let mut new = problem.harden_keep(&labels.into_iter().collect(), keep_predecessors);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        }
        Request::MergeEquivalentLabels(problem) => {
            let mut new = problem.merge_equivalent_labels();
            fix_problem(&mut new, true, true, &mut eh);
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
                    //problem.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
                }
            }
            problem.compute_passive_gen();
            handler(Response::P(problem));
        }
        Request::FullDiagram(mut problem) => {
            problem.compute_diagram_without_storing_maximized_passive(&mut eh);
            problem.compute_passive_gen();
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
            if problem.passive.degree == Degree::Finite(2) || problem.active.degree == Degree::Finite(2) {
                problem.compute_triviality_given_orientation(outdegree, &mut eh);
                //problem.compute_coloring_solvability_given_orientation(outdegree, &mut eh);
            }
            handler(Response::P(problem));
        },
        Request::AutoUb(problem, b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, coloring_given, coloring, coloring_given_passive, coloring_passive) => {
            eh.notify("autoub",0,0);
            problem.autoautoub( b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, if coloring_given {Some(coloring)} else {None}, if coloring_given_passive {Some(coloring_passive)} else {None}, |len,is_trivial,mut sequence|{
                //for p in sequence.iter_mut() {
                //    fix_problem(&mut p.1, true, true, &mut eh);
                //}
                handler(Response::AutoUb(len,sequence));
                eh.notify("autoub",0,0);
            }, &mut eh_ignore);
        },
        Request::AutoLb(problem, b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, coloring_given, coloring, coloring_given_passive, coloring_passive) => {
            eh.notify("autolb",0,0);
            problem.autoautolb( b_max_labels, max_labels, b_branching, branching, b_max_steps, max_steps, if coloring_given {Some(coloring)} else {None}, if coloring_given_passive {Some(coloring_passive)} else {None}, |len,mut sequence|{
                for (_,p) in sequence.iter_mut() {
                    p.compute_passive_gen();
                }
                handler(Response::AutoLb(len,sequence));
                eh.notify("autolb",0,0);
            }, &mut eh_ignore);
        },
        Request::ColoringSolvability(mut problem) => {
            problem.compute_coloring_solvability(&mut eh);
            problem.compute_passive_gen();
            handler(Response::P(problem));
        }
        Request::Marks(mut problem) => {
            if problem.passive.degree  != Degree::Finite(2) {
                handler(Response::E(
                    "It is required that the passive degree is 2.".into(),
                ));
            }else{
                problem.apply_marks_technique(&mut eh);
                handler(Response::P(problem));
            }
        }
        Request::DefaultDiagram(mut problem, partial, _triviality_only, labels, larger) => {
            problem.compute_default_fixpoint_diagram(if partial {Some(labels)} else {None}, larger, &mut eh);
            handler(Response::P(problem));
        }
        Request::SimplifySD(problem, sd, recompute_full_diagram) => {
            if let Some(mut new) =  problem.merge_subdiagram(&sd, recompute_full_diagram, &mut eh) {
                fix_problem(&mut new, true, true, &mut eh);
                handler(Response::P(new));
            } else {
                handler(Response::E("There is some problem with the given pattern".into()));
            }            
        }
        Request::CriticalHarden(problem, b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, keep_predecessors, b_maximize_rename) => {
            let mut new = problem.critical_harden(zerosteps, if b_coloring{ Some(coloring) } else {None}, if b_coloring_passive{ Some(coloring_passive) } else {None}, keep_predecessors, &mut eh);
            if !b_maximize_rename {
                fix_problem(&mut new, true, true, &mut eh);
                handler(Response::P(new));
            } else {
                match maximize_rename_gen(&mut new, &mut eh) {
                    Ok(()) => {
                        handler(Response::P(new));
                    }
                    Err(s) => handler(Response::E(s.into())),
                }
            }
        },
        Request::CriticalRelax(problem, b_coloring, coloring, b_coloring_passive, coloring_passive, zerosteps, b_maximize_rename) => {
            let mut new = problem.critical_relax(zerosteps, if b_coloring{ Some(coloring) } else {None},if b_coloring_passive{ Some(coloring_passive) } else {None}, &mut eh);
            if !b_maximize_rename {
                fix_problem(&mut new, true, true, &mut eh);
                handler(Response::P(new));
            } else {
                new = new.speedup(&mut eh);
                match maximize_rename_gen(&mut new, &mut eh) {
                    Ok(()) => {
                        handler(Response::P(new));
                    }
                    Err(s) => handler(Response::E(s.into())),
                }
            }
        }
        Request::Demisifiable(mut p,old) => {
            let mapping : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();
            p.compute_demisifiable(|set|{
                let set = set.iter().map(|l|&mapping[l]).join("");
                handler(Response::W(format!("Found set: {}",set)))
            },old,&mut eh);
            handler(Response::P(p));
        }
        Request::AddActivePredecessors(mut p, flip) => {
            p.add_active_predecessors();
            if flip {
                let active = p.active.clone();
                let passive = p.passive.clone();
                p.passive = active;
                p.active = passive;
                p.diagram_indirect = None;
                fix_problem(&mut p, true, true, &mut eh);
            }
            handler(Response::P(p));
        }
        Request::RemoveTrivialLines(p) => {
            let mut newp = p.remove_trivial_lines();
            fix_problem(&mut newp, true, true, &mut eh);
            handler(Response::P(newp));
        }
        Request::CheckZeroWithInput(mut problem, active, passive, sat, subset, reverse) => {
            let input = Problem::from_string_active_passive(active,passive);
            match input {
                Ok((mut input,missing_labels)) => {
                    if missing_labels {
                        handler(Response::W("Some labels appear on only one side!".into()));
                    }
                    if input.active.degree != problem.active.degree || input.passive.degree != problem.passive.degree {
                        handler(Response::E("Problems have different degrees".into()));
                    } else {
                        if reverse {
                            let t = problem;
                            problem = input;
                            input = t;
                        }
                        if !subset {
                            problem.compute_triviality_with_input(input, sat);
                            handler(Response::P(problem));
                        } else {
                            let mut best = input.labels().len()+1;
                            let mut best_arrows = 0;
                            let f = |mut p : Problem|{
                                let l = p.labels().len();
                                if l < best {
                                    best = l;
                                    p.diagram_indirect = None;
                                    p.compute_diagram(&mut eh);
                                    best_arrows = p.diagram_indirect.as_ref().unwrap().len();
                                    handler(Response::P(p));
                                } else if l == best {
                                    p.diagram_indirect = None;
                                    p.compute_diagram(&mut eh);
                                    let arrows = p.diagram_indirect.as_ref().unwrap().len();
                                    if arrows > best_arrows {
                                        best = l;
                                        best_arrows = arrows;
                                        handler(Response::P(p));
                                    }
                                }
                            };
                            if problem.compute_subinput_that_gives_nontriviality(input,sat, f).is_none() {
                                handler(Response::E("Could not find a suitable subinput".into()));
                            }
                        }
                    }
                }
                Err(s) => handler(Response::E(s.into())),
            }
        },
        Request::Dual(problem, active, passive) => {
            let fp = Problem::from_string_active_passive(active,passive);
            match fp {
                Ok((mut fp,missing_labels)) => {
                    if missing_labels {
                        handler(Response::W("Some labels appear on only one side!".into()));
                    }
                    if fp.active.degree != problem.active.degree || fp.passive.degree != problem.passive.degree {
                        handler(Response::E("Problems have different degrees".into()));
                    } else {
                        println!("maximizing passive fp");
                        fp.passive.maximize(&mut eh);
                        println!("computing diagram fp");
                        fp.compute_diagram(&mut eh);
                        println!("computing dual");
                        match problem.dual_problem(&fp, &mut eh) {
                            Ok((mut dual,_,_)) => {
                                fix_problem(&mut dual, true, false, &mut eh);
                                //let mut dual = dual.merge_subdiagram("",true,&mut eh).unwrap();
                                dual.compute_triviality(&mut eh);
                                handler(Response::P(dual));
                            }
                            Err(s) => handler(Response::E(s.into())),
                        }
                    }
                }
                Err(s) => handler(Response::E(s.into())),
            }
        },
        Request::DoubleDual(problem, active, passive) => {
            let fp = Problem::from_string_active_passive(active,passive);
            match fp {
                Ok((mut fp,missing_labels)) => {
                    if missing_labels {
                        handler(Response::W("Some labels appear on only one side!".into()));
                    }
                    if fp.active.degree != problem.active.degree || fp.passive.degree != problem.passive.degree {
                        handler(Response::E("Problems have different degrees".into()));
                    } else {
                        println!("maximizing passive fp");
                        fp.passive.maximize(&mut eh);
                        println!("computing diagram fp");
                        fp.compute_diagram(&mut eh);
                        println!("computing dual");
                        match problem.doubledual_problem(&fp, &mut eh) {
                            Ok(mut dual) => {
                                fix_problem(&mut dual, true, true, &mut eh);
                                handler(Response::P(dual));
                            }
                            Err(s) => handler(Response::E(s.into())),
                        }
                    }
                }
                Err(s) => handler(Response::E(s.into())),
            }
        },
        Request::DoubleDual2(problem, active, passive,diagram,input_active,input_passive) => {
            match problem.doubledual_diagram(&active,&passive,&diagram,&input_active,&input_passive, &mut eh) {
                Ok(diagram) => {
                    match problem.fixpoint_generic(None, FixpointType::Custom(diagram),false,&mut eh) {
                        Ok((mut new,_,_)) => {
                            fix_problem(&mut new, true, true, &mut eh);
                            handler(Response::P(new));
                        }
                        Err(s) => handler(Response::E(s.into())),
                    }
                }
                Err(s) => handler(Response::E(s.into()))
            }
        },
        Request::SmallestDual(mut problem, active, passive) => {
            let fp = Problem::from_string_active_passive(active,passive);
            match fp {
                Ok((mut fp,missing_labels)) => {
                    if missing_labels {
                        handler(Response::W("Some labels appear on only one side!".into()));
                    }
                    if fp.active.degree != problem.active.degree || fp.passive.degree != problem.passive.degree {
                        handler(Response::E("Problems have different degrees".into()));
                    } else {
                        fp.passive.maximize(&mut eh);
                        fp.compute_diagram(&mut eh);
                        println!("starting to compute dual");
                        match problem.dual_problem(&fp, &mut eh) {
                            Ok((input,_,_)) => {
                                println!("dual computed");
                                let input = input.merge_subdiagram("", false, &mut eh).unwrap();
                                let input = input.merge_subdiagram("", true, &mut eh).unwrap();
                                println!("merged equivalent");
                                let mut best = input.labels().len()+1;
                                let mut best_arrows = 0;
                                let f = |mut p : Problem|{
                                    let l = p.labels().len();
                                    if l < best {
                                        best = l;
                                        p.diagram_indirect = None;
                                        p.compute_diagram(&mut eh);
                                        best_arrows = p.diagram_indirect.as_ref().unwrap().len();
                                        handler(Response::P(p));
                                    } else if l == best {
                                        p.diagram_indirect = None;
                                        p.compute_diagram(&mut eh);
                                        let arrows = p.diagram_indirect.as_ref().unwrap().len();
                                        if arrows > best_arrows {
                                            best = l;
                                            best_arrows = arrows;
                                            handler(Response::P(p));
                                        }
                                    }
                                };
                                if fp.compute_subinput_that_gives_nontriviality(input,true, f).is_none() {
                                    handler(Response::E("Always trivial.".into()));
                                }
                            }
                            Err(s) => handler(Response::E(s.into())),
                        }
                    }
                }
                Err(s) => handler(Response::E(s.into())),
            }
        },
        Request::LogstarDup(problem, labels) => {
            let (mut new,_) = problem.logstar_dup(&labels);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        },
        Request::LogstarSee(problem, labels) => {
            let mut new = problem.logstar_see(&labels);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        },
        Request::LogstarMIS(problem, labels) => {
            let mut new = problem.logstar_mis(&labels);
            fix_problem(&mut new, true, true, &mut eh);
            handler(Response::P(new));
        },
        Request::AutoLogstar(mut problem, max_labels, max_depth, active, passive, max_active, max_passive, onlybool) => {
            //eh.notify("logstarautoub",0,0);
            if let Some((len,sequence)) = problem.autologstar(max_labels, max_depth, active, passive, max_active, max_passive, onlybool, &mut eh) {
                if !onlybool {
                    handler(Response::Logstar(len,sequence));
                } else {
                    handler(Response::E("Upper Bound Found!".into()));
                }
            }
        },
        Request::FixpointAddarrow(mut problem) => {
            let mut best = 0;

            problem.fixpoint_addarrow(|arrows : Vec<(Label,Label)>,x : usize, is_trivial : bool|{
                let mapping : HashMap<_,_> = problem.mapping_label_text.iter().cloned().collect();
                let arrows_str = arrows.iter().map(|(l1,l2)|format!("{} -> {}",mapping[&l1],mapping[&l2])).join(", ");
                if !is_trivial {
                    let msg = format!("Adding [{}] gives a fixed point",arrows_str);
                    handler(Response::E(msg.into()));
                } else if x >= best {
                    best = x;
                    let msg = format!("Adding [{}] gives {} active lines",arrows_str,x);
                    handler(Response::E(msg.into()));
                }
            });
        },
    }

    handler(Response::Done);
}

#[derive(Deserialize, Serialize)]
pub enum Request {
    NewProblem(String, String),
    SimplifyMerge(Problem, Label, Label),
    SimplifyMergeGroup(Problem, Vec<Label>, Label),
    SimplifyAddarrow(Problem, Label, Label),
    SimplifySD(Problem,String,bool),
    HardenRemove(Problem, Label, bool),
    HardenKeep(Problem, Vec<Label>, bool),
    Speedup(Problem),
    FixpointBasic(Problem, bool, bool, Vec<Label>),
    FixpointLoop(Problem, bool, bool, Vec<Label>),
    FixpointCustom(Problem,String, bool, bool, Vec<Label>),
    FixpointDup(Problem,Vec<Vec<Label>>, bool, bool, Vec<Label>, bool),
    FixpointAddarrow(Problem),
    InverseSpeedup(Problem),
    AllDifferentLabels(Problem),
    DeltaEdgeColoring(Problem),
    SpeedupMaximize(Problem),
    SpeedupMaximizeRenamegen(Problem),
    FullDiagram(Problem),
    Maximize(Problem),
    MergeEquivalentLabels(Problem),
    RenameGenerators(Problem),
    Rename(Problem, Vec<(Label, String)>),
    Orientation(Problem, usize),
    DefaultDiagram(Problem, bool, bool, Vec<Label>, bool),
    AutoUb(Problem, bool, usize, bool, usize, bool, usize, bool, usize, bool, usize),
    AutoLb(Problem, bool, usize, bool, usize, bool, usize, bool, usize, bool, usize),
    ColoringSolvability(Problem),
    Marks(Problem),
    CriticalHarden(Problem,bool, usize, bool, usize, usize, bool, bool),
    CriticalRelax(Problem,bool, usize, bool, usize, usize, bool),
    Demisifiable(Problem,bool),
    AddActivePredecessors(Problem,bool),
    RemoveTrivialLines(Problem),
    CheckZeroWithInput(Problem,String,String,bool,bool,bool),
    Dual(Problem,String,String),
    DoubleDual(Problem,String,String),
    DoubleDual2(Problem,String,String,String,String,String),
    SmallestDual(Problem,String,String),
    LogstarDup(Problem, Vec<Label>),
    LogstarSee(Problem, Vec<Label>),
    LogstarMIS(Problem, Vec<Label>),
    AutoLogstar(Problem, usize, usize, String, String, usize, usize, bool),
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
    W(String),
    AutoUb(usize,Vec<(AutoOperation,Problem)>),
    AutoLb(usize,Vec<(AutoOperation,Problem)>),
    Logstar(usize,Vec<(AutoOperation,Problem)>)
}

#[derive(Serialize,Deserialize,Clone)]
pub enum AutoOperation{
    Initial,
    Harden(Vec<Label>),
    Merge(Vec<(Label,Label)>,Problem),
    LogstarDup(Vec<Label>,Problem),
    LogstarSee(Vec<Label>,Problem),
    LogstarMIS(Vec<Label>,Problem),
    Speedup
}