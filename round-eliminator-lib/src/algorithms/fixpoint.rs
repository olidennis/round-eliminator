use std::{collections::{HashSet, HashMap, BTreeSet}, fmt::Display};

use chashmap::CHashMap;
use itertools::Itertools;

use crate::{problem::{Problem, DiagramDirect}, constraint::Constraint, group::{Label, Group}, line::Line, algorithms::diagram::compute_direct_diagram};
use serde::{Deserialize, Serialize};
use super::{event::EventHandler, maximize::{Operation}, diagram::{diagram_indirect_to_reachability_adj, diagram_to_indirect}};


#[derive(Clone,Debug,Serialize,Deserialize,Eq,PartialEq)]
pub struct FixpointDiagram {
    additional_orig : Vec<Label>,
    orig_labels : Vec<Label>,
    rightclosed : Vec<Vec<Label>>,
    diagram : Vec<(Label,Label)>,
    mapping_oldlabel_text : Vec<(Label, String)>,
    mapping_rightclosed_newlabel : Vec<(Vec<Label>,Label)>,
    mapping_label_newlabel : Vec<(Label,Label)>,
    mapping_newlabel_text : Vec<(Label, String)>,
    text : String,
    diagram_direct : DiagramDirect
}

impl FixpointDiagram {
    fn new(p : &Problem) -> FixpointDiagram {
        let labels = p.labels();
        let diagram_indirect = p.diagram_indirect.as_ref().unwrap();
        let successors =  diagram_indirect_to_reachability_adj(&labels, diagram_indirect);

        let rcs = right_closed_subsets(&labels, &successors);

        let mut fd = FixpointDiagram {
            orig_labels : labels,
            rightclosed : rcs,
            diagram : vec![],
            additional_orig : vec![],
            mapping_newlabel_text : vec![],
            mapping_oldlabel_text : p.mapping_label_text.iter().cloned().collect(),
            mapping_label_newlabel : vec![],
            mapping_rightclosed_newlabel : vec![],
            text : "".into(),
            diagram_direct : (vec![],vec![])
        };
        fd.compute_mappings();
        fd.assign_text();
        fd.compute_diagram();
        fd.compute_text();

        fd
    }

    fn compute_mappings(&mut self){

        self.mapping_rightclosed_newlabel = self.rightclosed.iter().enumerate().map(|(i,r)|{
            (r.clone(),i as Label)
        }).collect();


        let mapping_label_rightclosed : HashMap<Label,Vec<Label>> = self.orig_labels.iter().map(|&l|{
            let mut containing : Vec<_> = self.rightclosed.iter().filter(|set|set.contains(&l)).collect();
            containing.sort_by_key(|x|x.len());
            (l,containing[0].clone())
        }).collect();

        let mapping_rightclosed_newlabel : HashMap<_,_> = self.mapping_rightclosed_newlabel.iter().cloned().collect();
        let mut mapping_label_newlabel = HashMap::new();
        for &l in &self.orig_labels {
            let newl = mapping_rightclosed_newlabel[&mapping_label_rightclosed[&l]];
            mapping_label_newlabel.insert(l, newl);
        }
        self.mapping_label_newlabel = mapping_label_newlabel.into_iter().collect();

    }

    fn compute_diagram(&mut self){
        let mut diagram = vec![];
        let mapping_rightclosed_newlabel : HashMap<_,_> = self.mapping_rightclosed_newlabel.iter().cloned().collect();

        let rcs = &self.rightclosed;
        
        for s1 in rcs {
            for s2 in rcs {
                let set_s1 : HashSet<Label> = HashSet::from_iter(s1.iter().cloned());
                let set_s2 : HashSet<Label> = HashSet::from_iter(s2.iter().cloned());
                let new_s1 = mapping_rightclosed_newlabel[s1];
                let new_s2 = mapping_rightclosed_newlabel[s2];
                if set_s1.is_superset(&set_s2) {
                    diagram.push((new_s1,new_s2));
                }
            }
        }

        let newlabels : Vec<Label> = mapping_rightclosed_newlabel.values().cloned().collect();
        let diagram = diagram_to_indirect(&newlabels,&diagram);
        self.diagram_direct = compute_direct_diagram(&newlabels, &diagram);
        self.diagram = diagram;
    }

    fn assign_text(&mut self) {
        let oldtext : HashMap<_,_> = self.mapping_oldlabel_text.iter().cloned()
            .chain(self.additional_orig.iter().enumerate().map(|(i,&l)|(l,format!("(dup{})",i))))
            .collect();
        let onechar = oldtext.iter().all(|(_,t)|t.len() == 1);
        self.mapping_newlabel_text = self.mapping_rightclosed_newlabel.iter().map(|(r,l)|{
            if onechar {
                let mut text = r.iter().map(|ol|&oldtext[ol]).sorted().join("");
                if text == "" {
                    text = "∅".into();
                }
                if text.chars().count() > 1 {
                    text = format!("({})",text);
                }
                (*l,text)
            } else {
                let mut text = format!("({})",
                    r.iter().map(|ol|
                        oldtext[ol].chars().filter(|&c|c!='('&&c!=')').collect::<String>()
                    ).sorted().join("_")
                );
                if text == "()" {
                    text = "∅".into();
                }
                (*l,text)
            }
        }).collect();
    }

    fn duplicate_labels(&mut self, groups : &Vec<Vec<Label>>){
        let newlabel_to_rightclosed : HashMap<_,_> = self.mapping_rightclosed_newlabel.iter().map(|(r,n)|(*n,r.clone())).collect();
        let mut sets : Vec<_> = self.mapping_rightclosed_newlabel.iter().map(|(r,_)|HashSet::from_iter(r.iter().cloned())).collect();
        let mut next_fresh = self.mapping_oldlabel_text.iter().map(|(o,t)|*o).max().unwrap() + 1;
        let mut additional_orig = vec![];

        for group in groups {
            let group : Vec<HashSet<Label>> = group.iter().map(|l|HashSet::from_iter(newlabel_to_rightclosed[l].iter().cloned())).collect();

            let mut labels_to_add = vec![];
            for set in sets.iter_mut() {
                if group.iter().any(|todup|{
                    let hset : HashSet<Label> = HashSet::from_iter(set.iter().cloned());
                    let hdup : HashSet<Label> = HashSet::from_iter(todup.iter().cloned());
                    let added : HashSet<Label> = HashSet::from_iter(additional_orig.iter().cloned());
                    let mut diff = hset.difference(&hdup);
                    hset.is_superset(&hdup) && diff.all(|d|added.contains(d))
                }) {
                    let mut set = set.clone();
                    set.insert(next_fresh);
                    labels_to_add.push(set);
                } else {
                    if group.iter().any(|set_to_dup|set_to_dup.is_subset(set)) {
                        set.insert(next_fresh);
                    }
                }
            }
            sets.extend(labels_to_add.into_iter());

            additional_orig.push(next_fresh);
            next_fresh += 1;
        }


        /* 
        let all_dup : HashSet<Label> = groups.iter().flat_map(|group|group.iter().cloned()).collect();
        for label in all_dup {
            let mut origs = vec![];
            for (i,group) in groups.iter().enumerate() {
                if group.contains(&label) {
                    origs.push(additional_orig[i]);
                }
            }
            for toadd in origs.iter().cloned().powerset() {
                let set = newlabel_to_rightclosed[&label].iter().cloned().chain(toadd.into_iter()).collect();
                sets.push(set);
            }
        }*/

        let mut result = HashSet::new();
        for set in sets {
            let mut set : Vec<_> = set.into_iter().collect();
            set.sort();
            result.insert(set);
        }

        self.rightclosed = result.into_iter().collect();
        self.additional_orig = additional_orig;
        self.compute_mappings();
        self.assign_text();
        self.compute_diagram();
        self.compute_text();
    }

    pub fn compute_text(&mut self) {
        let mapping_oldlabel_text : HashMap<_,_> = self.mapping_oldlabel_text.iter().cloned().collect();
        let mapping_label_newlabel = &self.mapping_label_newlabel;
        let mapping_newlabel_text = &self.mapping_newlabel_text;
        let diagram = &self.diagram;
        let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
        let mapping_newlabel_text : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();

        let diagram = diagram_to_indirect(&newlabels,&diagram);
        let (_,diagram) = compute_direct_diagram(&newlabels, &diagram);
        let mut hdiagram = HashMap::new();
        for &(a,b) in diagram.iter() {
            hdiagram.entry(a).or_insert(vec![]).push(b);
        }
        let diagram : Vec<_> = hdiagram.into_iter().sorted_by_key(|(l,_)|&mapping_newlabel_text[l]).collect();

        let diagram = diagram.iter().map(|(a,b)|{
            format!("{} -> {}",mapping_newlabel_text[a],b.iter().map(|b|&mapping_newlabel_text[b]).sorted().join(" "))
        }).join("\n");
        let mapping = mapping_label_newlabel.iter().map(|(l,n)|format!("{} = {}",mapping_oldlabel_text[l],mapping_newlabel_text[n].clone())).join("\n");
        self.text = format!("# mapping from original labels to diagram labels\n{}\n# diagram edges\n{}\n",mapping,diagram);
    }
}

type Tracking = (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>);

impl Problem {

    pub fn compute_default_fixpoint_diagram(&mut self) {
        self.fixpoint_diagram = Some(FixpointDiagram::new(self));
    }

    pub fn fixpoint(&self, eh: &mut EventHandler) -> Result<Self, &'static str> {
        self.fixpoint_dup(None, eh)
    }

    pub fn fixpoint_dup(&self, dup : Option<Vec<Vec<Label>>>, eh: &mut EventHandler) -> Result<Self, &'static str> {
        let mut fd = if let Some(fd) = self.fixpoint_diagram.clone() {
            fd
        } else {
            FixpointDiagram::new(self)
        };
        if let Some(dup) = dup {
            fd.duplicate_labels(&dup);
        }
        let mapping_label_newlabel = fd.mapping_label_newlabel;
        let mapping_newlabel_text = fd.mapping_newlabel_text;
        let diagram = fd.diagram;
        //println!("{:?}\n{:?}\n{:?}",mapping_label_newlabel,mapping_newlabel_text,diagram);

        Ok(self.fixpoint_onestep(&mapping_label_newlabel, &mapping_newlabel_text, &diagram, None, None, eh)?.0)
    }


    pub fn fixpoint_custom(&self, text_diag : String, eh: &mut EventHandler) -> Result<Self, &'static str> {
        let text_mapping = text_diag.lines().filter(|line|!line.starts_with("#") && line.contains("=")).join("\n");
        let text_diagram = text_diag.lines().filter(|line|!line.starts_with("#") && (line.contains("->") || line.contains("<-"))).join("\n");

        let mapping_newlabel_text : Vec<_> = text_diagram.split_whitespace().flat_map(|w|w.split("<-")).flat_map(|w|w.split("->")).filter(|&s|s != "->" && s != "<-" && s != "").unique().enumerate().map(|(l,s)|(l as Label,s.to_owned())).collect();
        let mapping_text_newlabel : HashMap<_,_> = mapping_newlabel_text.iter().cloned().map(|(a,b)|(b,a)).collect();
        let mapping_oldtext_newtext : HashMap<_,_> = text_mapping.lines().map(|line|{
            let mut line = line.split("=");
            let a = line.next().unwrap().trim();
            let b = line.next().unwrap().trim();
            (a.to_owned(),b.to_owned())
        }).collect();

        let mapping_label_newlabel : Vec<_> = self.mapping_label_text.iter().map(|(l,s)|{
            if mapping_oldtext_newtext.contains_key(s) {
                (*l,mapping_text_newlabel[&mapping_oldtext_newtext[s]])
            } else {
                (*l,mapping_text_newlabel[s])
            }
            
        }).collect();

        let diagram : Vec<_> = text_diagram.split("\n").flat_map(|line|{
            let mut v = vec![];
            if line.contains("->") {
                let mut line = line.split("->");
                let a = line.next().unwrap();
                let b = line.next().unwrap();
                for a in a.split_whitespace() {
                    for b in b.split_whitespace() {
                        v.push((mapping_text_newlabel[a],mapping_text_newlabel[b]));
                    }
                }
            } else if line.contains("<-") {
                let mut line = line.split("<-");
                let b = line.next().unwrap();
                let a = line.next().unwrap();
                for a in a.split_whitespace() {
                    for b in b.split_whitespace() {
                        v.push((mapping_text_newlabel[a],mapping_text_newlabel[b]));
                    }
                }
            } 
            v.into_iter()
        }).collect();
        Ok(self.fixpoint_onestep(&mapping_label_newlabel, &mapping_newlabel_text, &diagram, None, None, eh)?.0)
    }

    pub fn fixpoint_onestep(&self, mapping_label_newlabel : &Vec<(Label, Label)>, mapping_newlabel_text : &Vec<(Label, String)>, diagram : &Vec<(Label,Label)>, tracking : Option<&CHashMap<Line,Tracking>>, tracking_passive : Option<&CHashMap<Line,Tracking>>, eh: &mut EventHandler) -> Result<(Self,Constraint), &'static str> {
        let active = self.active.all_choices(true);
        let passive = self.passive.all_choices(true);
        let active = Constraint{ lines: active, is_maximized: false, degree: self.active.degree  };
        let passive = Constraint{ lines: passive, is_maximized: false, degree: self.passive.degree  };
        let mapping_label_newlabel : HashMap<_,_> = mapping_label_newlabel.iter().cloned().collect();
        let active = active.edited(|g| Group(vec![mapping_label_newlabel[&g.0[0]]]));
        let passive = passive.edited(|g| Group(vec![mapping_label_newlabel[&g.0[0]]]));
        let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
        let diagram_indirect = diagram_to_indirect(&newlabels,&diagram);
        let diagram_indirect_rev = diagram_indirect.iter().map(|&(a,b)|(b,a)).collect();
        let active = procedure(&active, &newlabels, &diagram_indirect, &mapping_newlabel_text, tracking, eh)?;
        let passive = procedure(&passive, &newlabels, &diagram_indirect_rev, &mapping_newlabel_text, tracking_passive, eh)?;
        let passive_successors = diagram_indirect_to_reachability_adj(&newlabels,&diagram_indirect);
        let passive_before_edit = passive.clone();
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
            fixpoint_diagram : None
        };
        p.mapping_label_text = mapping_newlabel_text.clone();
        Ok((p,passive_before_edit))
    }


    pub fn fixpoint_loop(&self, eh: &mut EventHandler) -> Result<Self,&'static str> {
        let fd = if let Some(fd) = self.fixpoint_diagram.clone() {
            fd
        } else {
            FixpointDiagram::new(self)
        };        
        let orig_diagram = self.diagram_indirect.as_ref().unwrap();
        let mut diagram = fd.diagram;
        let mapping_label_newlabel = fd.mapping_label_newlabel;
        let mut mapping_newlabel_text = fd.mapping_newlabel_text;
        let mut mapping_label_newlabel : HashMap<_, _> = mapping_label_newlabel.iter().cloned().collect();

        let mut all_expressions : HashSet<TreeNode<Label>> = HashSet::new();

        let p = loop {

            let tracking = CHashMap::new();
            let tracking_passive = CHashMap::new();

            // run the fixpoint procedure, keep track of how each line has been obtained
            let (mut p,passive_before_edit) = self.fixpoint_onestep(&mapping_label_newlabel.iter().map(|(&a,&b)|(a,b)).collect(),&mapping_newlabel_text,&diagram,Some(&tracking),Some(&tracking_passive),eh)?;
            p.compute_triviality(eh);

            // if the problem is trivial, we need to repeat with a different diagram
            if !p.trivial_sets.as_ref().unwrap().is_empty() {
                let mapping : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();

                // we extract all subexpressions for all lines obtained, both active and passive
                let mut expressions = HashSet::new();
                for (lines,tracking,flip) in [(p.active.lines,tracking,false),(passive_before_edit.lines,tracking_passive,true)] {
                    let mut exprs = HashSet::new();
                    for line in lines {
                        let len = if let Some(rg) = tracking.get(&line) {
                            let (_,_,before_norm,_,_) = &*rg;
                            before_norm.parts.len()
                        } else {
                            line.parts.len()
                        };
                        for i in 0..len {
                            let expr = expression_for_line_at(&line,i,false, &tracking,&mapping).reduce_rep();
                            expr.get_all_subexpressions(&mut exprs);
                        }
                    }
                    for expr in exprs {
                        expressions.insert(if flip { expr.flip() } else {expr});
                    }
                }

                // if something goes wrong, the original labels may not appear in the result, so we add them
                for (_,&x) in &mapping_label_newlabel {
                    expressions.insert(TreeNode::Terminal(x));
                }

                // we also add all expressions obtained in previous attempts
                for e in &all_expressions {
                    expressions.insert(e.convert(&mapping_label_newlabel));
                }

                println!("The problem is trivial, there are {} subexpressions",expressions.len());

                let map_label_expr : HashMap<_,_> = expressions.iter().cloned().enumerate().map(|(a,b)|(a as Label,b)).collect();
                let map_expr_label : HashMap<_,_> = expressions.iter().cloned().enumerate().map(|(a,b)|(b,a as Label)).collect();
                
                // the first edges of the diagram are just given by the structure of the expressions
                let mut new_diagram = vec![];
                for (&l,e) in &map_label_expr {
                    if let TreeNode::Expr(a,b,op) = e {
                        if op == &Operation::Union {
                            new_diagram.push((map_expr_label[a],l));
                            new_diagram.push((map_expr_label[b],l));
                        }
                        if op == &Operation::Intersection {
                            new_diagram.push((l,map_expr_label[a]));
                            new_diagram.push((l,map_expr_label[b]));
                        }
                    }
                }

                // we now just compute some mappings
                let label_to_oldlabel : HashMap<_,_> = mapping_label_newlabel.iter().map(|(&l,&n)|(n,l)).collect();

                diagram = new_diagram;
                mapping_newlabel_text = map_label_expr.iter().map(|(&l,e)|{
                    (l,e.convert(&mapping).to_string()) 
                }).collect();


                mapping_label_newlabel = map_label_expr.iter().filter_map(|(&l,e)| match e {
                    TreeNode::Terminal(x) => { Some((label_to_oldlabel[x],l)) },
                    TreeNode::Expr(_,_,_) => None
                }).collect();

                // the current expressions are added to the ones that we use in the next try
                for (_,e) in &map_label_expr {
                    all_expressions.insert(e.convert(&label_to_oldlabel));
                }

                // we also add to the current diagram the edges of the original diagram
                for (x,y) in orig_diagram {
                    diagram.push((mapping_label_newlabel[x],mapping_label_newlabel[y]));
                }

                let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
                diagram = diagram_to_indirect(&newlabels,&diagram);
                diagram.sort();

                // we fix the diagram: a node that is the union of (a,b) must point to all common successors of a and b 
                loop{
                    let before_edit = diagram.clone();
                    let diagram_rev : Vec<_> = diagram.iter().map(|&(a,b)|(b,a)).collect();
                    let successors = diagram_indirect_to_reachability_adj(&newlabels,&diagram);
                    let predecessors = diagram_indirect_to_reachability_adj(&newlabels,&diagram_rev);
                    for (&l,e) in &map_label_expr {
                        if let TreeNode::Expr(a,b,op) = e {
                            let a = map_expr_label[a];
                            let b = map_expr_label[b];
                            if op == &Operation::Union {
                                let commons : Vec<_> = successors[&a].intersection(&successors[&b]).collect();
                                for &common in commons {
                                    diagram.push((l,common));
                                }
                            }
                            if op == &Operation::Intersection {
                                let commons : Vec<_> = predecessors[&a].intersection(&predecessors[&b]).collect();
                                for &common in commons {
                                    diagram.push((common,l));
                                }
                            }
                        }
                    }
                    diagram = diagram_to_indirect(&newlabels,&diagram);
                    diagram.sort();
                    if before_edit == diagram {
                        break;
                    }
                }

                // we add a source and a sink to make sure that every pair of labels has some common successor and predecessor
                let max = mapping_newlabel_text.iter().map(|(l,_)|l).max().unwrap();
                let source = (max+1) as Label;
                let sink = (max+2) as Label;
                for &(l,_) in &mapping_newlabel_text {
                    diagram.push((source,l));
                    diagram.push((l,sink));
                }
                mapping_newlabel_text.push((source,"(Source)".to_owned()));
                mapping_newlabel_text.push((sink,"(Sink)".to_owned()));

                // we merge equivalent labels
                let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
                let (merges,_) = compute_direct_diagram(&newlabels, &diagram);
                for (l,g) in merges {
                    for l2 in g {
                        if l2 != l {
                            diagram.retain(|&(a,b)|a != l2 && b != l2);
                            mapping_newlabel_text.retain(|&(a,_)|a != l2);
                            for (k,v) in mapping_label_newlabel.iter().map(|(&k,&v)|(k,v)).collect::<Vec<_>>().into_iter() {
                                if v == l2 {
                                    mapping_label_newlabel.insert(k,l);
                                }
                            }
                        }
                    }
                }

            } else {
                break p;
            }
        };

        Ok(p)
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


fn procedure(constraint : &Constraint, labels : &[Label], diagram_indirect : &Vec<(Label, Label)>, mapping : &Vec<(Label, String)>, tracking : Option<&CHashMap<Line,Tracking>>, eh: &mut EventHandler) -> Result<Constraint, &'static str> {
    let becomes_star = 100;


    let mapping : HashMap<_,_> = mapping.iter().cloned().collect();

    let successors = diagram_indirect_to_reachability_adj(&labels,&diagram_indirect);
    let predecessors = diagram_indirect_to_reachability_adj(&labels,&diagram_indirect.iter().cloned().map(|(a,b)|(b,a)).collect());

    let mut unions = HashMap::<(Label,Label),Label>::new();
    let mut intersections = HashMap::<(Label,Label),Label>::new();

    for &l1 in labels {
        for &l2 in labels {
            let mut common : HashSet<Label> = successors[&l1].intersection(&successors[&l2]).cloned().collect();
            let len = common.len();
            for l in common.clone().into_iter() {
                for r in successors[&l].iter().filter(|&&x|x != l) {
                    common.remove(r);
                }
            }
            if common.len() != 1 {
                return Err("The diagram does not satisfy the requirements");
            }
            //assert!(common.len() == 1);
            unions.insert((l1,l2),common.into_iter().next().unwrap());

            let mut common : HashSet<Label> = predecessors[&l1].intersection(&predecessors[&l2]).cloned().collect();
            for l in common.clone().into_iter() {
                for r in predecessors[&l].iter().filter(|&&x|x != l) {
                    common.remove(r);
                }
            }
            if common.len() != 1 {
                return Err("The diagram does not satisfy the requirements");
            }
            //assert!(common.len() == 1);
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

    let mut newconstraint = constraint.clone();
    newconstraint.is_maximized = false;


    newconstraint.maximize_custom(eh,true,false,tracking,f_is_superset, f_union, f_intersection);
    /*println!("obtained constraint");
    for line in &newconstraint.lines {
        println!("{}",line.to_string(&mapping));
    }*/

    Ok(newconstraint)
}


#[derive(Ord,PartialOrd,Eq,PartialEq,Hash,Clone)]
enum TreeNode<T> where T : Ord + PartialOrd + Eq + PartialEq + std::hash::Hash + Clone{
    Terminal(T),
    Expr(Box<TreeNode<T>>,Box<TreeNode<T>>,Operation)
}


fn expression_for_line_at(line : &Line, pos : usize, norm_pos : bool, how : &CHashMap<Line, (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>)>, mapping : &HashMap<Label,String>) -> TreeNode<Label> {
    if let Some(rg) = how.get(line) {
        let (l1,l2, _, norm_map, parts) = &*rg;
        let (p1,p2,op) = parts[if norm_pos {norm_map[pos][0]} else {pos}];
        let part1 = expression_for_line_at(l1, p1, true, how, mapping);
        let part2 = expression_for_line_at(l2, p2, true, how, mapping);
        let mut v = vec![part1,part2];
        v.sort();
        let part2 = v.pop().unwrap();
        let part1 = v.pop().unwrap();
        TreeNode::Expr(Box::new(part1),Box::new(part2),op)
    } else {
        TreeNode::Terminal(line.parts[pos].group[0])
    }

}

impl<T> std::fmt::Display for TreeNode<T> where T : Ord + PartialOrd + Eq + PartialEq + std::hash::Hash + Clone + std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = match self {
            TreeNode::Terminal(x) => { x.to_string() },
            TreeNode::Expr(a,b,op) => {
                let part1 = a.to_string();
                let part2 = b.to_string();
                let op = if *op == Operation::Union { "∩" } else { "∪" };
                format!("({} {} {})",part1 , op , part2)
            }
        };
        write!(f, "{}",r)
    }
}

impl<T> TreeNode<T> where T : Ord + PartialOrd + Eq + PartialEq + std::hash::Hash + Clone + Display {

    fn convert<U>(&self, map : &HashMap<T,U>) -> TreeNode<U> where U : Ord + PartialOrd + Eq + PartialEq + std::hash::Hash + Clone {
        match self {
            TreeNode::Terminal(x) => { TreeNode::Terminal(map[&x].clone()) },
            TreeNode::Expr(a,b,op) => {
                let part1 = a.convert(map);
                let part2 = b.convert(map);
                TreeNode::Expr(Box::new(part1),Box::new(part2),*op)
            }
        }
    }

    fn reduce_rep(&self) -> Self {
        let mut t = self.clone();
        loop {
            let nt = t.reduce();
            if nt == t {
                break;
            }
            t = nt;
        }
        t
    }

    fn reduce(&self) -> Self {
        match self {
            TreeNode::Terminal(_) => { self.clone() },
            TreeNode::Expr(a,b,op) => {
                if a == b {
                    return a.reduce();
                }
                if let TreeNode::Expr(a1,a2,aop) = &**a {
                    if aop == op && (a1 == b || a2 == b) {
                        return a.reduce();
                    }
                }
                if let TreeNode::Expr(b1,b2,bop) = &**b {
                    if bop == op && (b1 == a || b2 == a) {
                        return b.reduce();
                    }
                }
                return TreeNode::Expr(Box::new(a.reduce()),Box::new(b.reduce()),*op);
            }
        }
    }

    fn get_all_subexpressions(&self, r : &mut HashSet<Self>) {
        if !r.contains(self) {
            r.insert(self.clone());
            match self {
                TreeNode::Terminal(_) => {},
                TreeNode::Expr(a,b,_) => {
                    a.get_all_subexpressions(r);
                    b.get_all_subexpressions(r);
                }
            }
        }
    }

    /*fn mirror_expr(&self, map : &HashMap<T,T>) -> Self {
        match self {
            TreeNode::Terminal(x) => { TreeNode::Terminal(map[&x].clone()) },
            TreeNode::Expr(a,b,op) => {
                let op = if *op == Operation::Intersection { Operation::Union } else { Operation::Intersection };
                return TreeNode::Expr(Box::new(a.mirror_expr(map)),Box::new(b.mirror_expr(map)),op);
            }
        }
    }*/
    fn flip(&self) -> Self {
        match self {
            TreeNode::Terminal(x) => { TreeNode::Terminal(x.clone()) },
            TreeNode::Expr(a,b,op) => {
                let op = if *op == Operation::Intersection { Operation::Union } else { Operation::Intersection };
                return TreeNode::Expr(Box::new(a.flip()),Box::new(b.flip()),op);
            }
        }
    }

    fn result(&self) -> BTreeSet<T> {
        match self {
            TreeNode::Terminal(x) => { BTreeSet::from([x.clone()]) },
            TreeNode::Expr(a,b,op) => {
                let a = a.result();
                let b = b.result();
                if *op == Operation::Intersection {
                    a.union(&b).cloned().collect()
                } else {
                    a.intersection(&b).cloned().collect()
                }
            }
        }
    }
}

/* 
fn add_diagram_edges(&mut self){
    for (l,e) in &self.map_label_expression {
        self.successors.entry(*l).or_default().insert(*l);
        self.predecessors.entry(*l).or_default().insert(*l);
        if let TreeNode::Expr(a,b,op) = e {
            if *op == Operation::Intersection {
                self.successors.entry(self.map_expression_label[a]).or_default().insert(*l);
                self.successors.entry(self.map_expression_label[b]).or_default().insert(*l);
            }
            if *op == Operation::Union {
                self.successors.entry(*l).or_default().insert(self.map_expression_label[a]);
                self.successors.entry(*l).or_default().insert(self.map_expression_label[b]);
            }
        }
        
    }
}*/