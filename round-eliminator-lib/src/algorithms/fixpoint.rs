use std::collections::{HashSet, HashMap};

use itertools::Itertools;

use crate::{problem::Problem, constraint::Constraint, group::{Label, Group}, line::Line};

use super::{event::EventHandler, maximize::{combine_lines_custom, without_one}, diagram::diagram_indirect_to_reachability_adj};




impl Problem {
    pub fn fixpoint(&self, eh: &mut EventHandler) -> Self {
        let labels = self.labels();
        let diagram_indirect = self.diagram_indirect.as_ref().unwrap();
        let successors =  diagram_indirect_to_reachability_adj(&labels, diagram_indirect);
        eh.notify("new labels", 1, 1);
        let rcs = right_closed_subsets(&labels, &successors);
        
        let mapping_label_rightclosed : HashMap<Label,Vec<Label>> = labels.iter().map(|&l|{
            let mut r = successors[&l].clone();
            r.insert(l);
            (l,r.into_iter().sorted().collect())
        }).collect();

        let mapping_rightclosed_newlabel : HashMap<Vec<Label>,Label> = rcs.iter().enumerate().map(|(i,r)|{
            (r.clone(),i as Label)
        }).collect();

        let newlabels : Vec<Label> = mapping_rightclosed_newlabel.iter().map(|(_,&l)|l).collect();

        eh.notify("enumerating configurations", 1, 1);

        let active = self.active.all_choices(true);
        let passive = self.passive.all_choices(true);
        let active = Constraint{ lines: active, is_maximized: false, degree: self.active.degree  };
        let passive = Constraint{ lines: passive, is_maximized: false, degree: self.passive.degree  };
        let active = active.edited(|g| Group(vec![mapping_rightclosed_newlabel[&mapping_label_rightclosed[&g.0[0]]]]));
        let passive = passive.edited(|g| Group(vec![mapping_rightclosed_newlabel[&mapping_label_rightclosed[&g.0[0]]]]));

        let mut diagram_indirect = vec![];
        let mut diagram_indirect_rev = vec![];
        
        for s1 in &rcs {
            for s2 in &rcs {
                let set_s1 : HashSet<Label> = HashSet::from_iter(s1.iter().cloned());
                let set_s2 : HashSet<Label> = HashSet::from_iter(s2.iter().cloned());
                let new_s1 = mapping_rightclosed_newlabel[s1];
                let new_s2 = mapping_rightclosed_newlabel[s2];
                if set_s1.is_superset(&set_s2) {
                    diagram_indirect.push((new_s1,new_s2));
                }
                if set_s1.is_superset(&set_s2) {
                    diagram_indirect_rev.push((new_s2,new_s1));
                }
            }
        }

        let active = procedure(&active, &newlabels, &diagram_indirect, eh);
        let passive = procedure(&passive, &newlabels, &diagram_indirect_rev, eh);

        let passive_successors = diagram_indirect_to_reachability_adj(&newlabels,&diagram_indirect);
        let passive = passive.edited(|g| Group(passive_successors[&g.0[0]].iter().cloned().sorted().collect()));

        let mut p = Problem {
            active,
            passive,
            mapping_label_text: vec![],
            mapping_label_oldlabels: None,
            mapping_oldlabel_labels: None,
            mapping_oldlabel_text: None,
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_indirect_old: None,
            diagram_direct: None,
            orientation_coloring_sets: None,
            orientation_trivial_sets: None,
            orientation_given: None,
        };

        let oldtext : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();

        let onechar = oldtext.iter().all(|(_,t)|t.len() == 1);

        p.mapping_label_text = mapping_rightclosed_newlabel.iter().map(|(r,&l)|{
            if onechar {
                let mut text = r.iter().map(|ol|&oldtext[ol]).sorted().join("");
                if text == "" {
                    text = "∅".into();
                }
                if text.chars().count() > 1 {
                    text = format!("({})",text);
                }
                (l,text)
            } else {
                let mut text = format!("({})",
                    r.iter().map(|ol|
                        oldtext[ol].chars().filter(|&c|c!='('&&c!=')').collect::<String>()
                    ).sorted().join("_")
                );
                if text == "()" {
                    text = "∅".into();
                }
                (l,text)
            }
        }).collect();
        p
    }


}

fn rcs_helper(labels : &[Label], right: &HashMap<Label,HashSet<Label>>, result: &mut Vec<HashSet<Label>>, added: HashSet<Label>) {
    for &x in labels {
        let mut toadd = right[&x].clone();
        toadd.insert(x);
        if !added.contains(&x) && (added.is_empty() || !toadd.is_superset(&added)) {
            let mut new = added.clone();
            new.extend(toadd.into_iter());
            result.push(new.clone());
            rcs_helper(&labels[1..], right, result, new);
        }
    }
}

pub fn right_closed_subsets(labels : &[Label], successors : &HashMap<Label, HashSet<Label>>) -> Vec<Vec<Label>> {
    let mut result = vec![HashSet::new()];
    rcs_helper(labels, successors, &mut result, HashSet::new());
    result.into_iter().map(|set|set.into_iter().sorted().collect::<Vec<Label>>()).unique().sorted().collect()
}


fn procedure(constraint : &Constraint, labels : &[Label], diagram_indirect : &Vec<(Label, Label)>, eh: &mut EventHandler) -> Constraint {
    let becomes_star = 100;

    let successors = diagram_indirect_to_reachability_adj(&labels,&diagram_indirect);
    let predecessors = diagram_indirect_to_reachability_adj(&labels,&diagram_indirect.iter().cloned().map(|(a,b)|(b,a)).collect());

    let mut unions = HashMap::<(Label,Label),Label>::new();
    let mut intersections = HashMap::<(Label,Label),Label>::new();

    for &l1 in labels {
        for &l2 in labels {
            let mut common : HashSet<Label> = successors[&l1].intersection(&successors[&l2]).cloned().collect();
            for l in common.clone().into_iter() {
                for r in successors[&l].iter().filter(|&&x|x != l) {
                    common.remove(r);
                }
            }
            unions.insert((l1,l2),common.into_iter().next().unwrap());

            let mut common : HashSet<Label> = predecessors[&l1].intersection(&predecessors[&l2]).cloned().collect();
            for l in common.clone().into_iter() {
                for r in predecessors[&l].iter().filter(|&&x|x != l) {
                    common.remove(r);
                }
            }
            intersections.insert((l1,l2),common.into_iter().next().unwrap());
        }
    }

    let f_is_superset = |g1 : &Group,g2 : &Group|{
        successors[&g2[0]].contains(&g1[0])
    };

    let f_union = |g1 : &Group,g2 : &Group|{ 
        Group(vec![unions[&(g1[0],g2[0])]])
    };

    let f_intersection = |g1 : &Group,g2 : &Group|{ 
        Group(vec![intersections[&(g1[0],g2[0])]])
    };

    let mut seen = HashSet::new();
    let mut seen_pairs = HashSet::new();

    let mut newconstraint = Constraint{ lines: vec![], is_maximized: false, degree: constraint.degree  };
    for line in &constraint.lines {
        let mut line = line.clone();
        line.normalize();
        seen.insert(line.clone());
        newconstraint.add_line_and_discard_non_maximal_with_custom_supersets(line.clone(), Some(f_is_superset));
    }

    let mut constraint = newconstraint;
    let empty = Constraint{ lines: vec![], is_maximized: false, degree: constraint.degree  };

    loop {
        let mut newconstraint = constraint.clone();
        let lines = &constraint.lines;

        let without_one = without_one(lines);

        for i in 0..lines.len() {
            let mut candidates2 = empty.clone();

            for j in 0..=i {
                let len = lines.len();
                eh.notify("combining line pairs", i * len + j, len * len);

                let pair = (lines[i].clone(), lines[j].clone());
                if seen_pairs.contains(&pair)
                    || seen_pairs.contains(&(pair.1.clone(), pair.0.clone()))
                {
                    continue;
                }
                seen_pairs.insert(pair);

                let candidates = combine_lines_custom(
                    &lines[i],
                    &lines[j],
                    &without_one[i],
                    &without_one[j],
                    &mut seen,
                    becomes_star,
                    true,
                    f_is_superset, f_union, f_intersection
                ).0;
                for newline in candidates {
                    candidates2.add_line_and_discard_non_maximal_with_custom_supersets(newline,Some(f_is_superset));
                }
            }

            for newline in candidates2.lines {
                newconstraint.add_line_and_discard_non_maximal_with_custom_supersets(newline,Some(f_is_superset));
            }
        }

        if newconstraint == constraint {
            break;
        }
        constraint = newconstraint;
    }

    constraint
}

