use std::{collections::{HashSet, HashMap, BTreeSet}, fmt::Display};

use chashmap::CHashMap;
use itertools::Itertools;

use crate::{problem::Problem, constraint::Constraint, group::{Label, Group}, line::Line, algorithms::diagram::compute_direct_diagram};

use super::{event::EventHandler, maximize::{Operation}, diagram::{diagram_indirect_to_reachability_adj, diagram_to_indirect}};


fn first_diagram(p : &Problem) -> (Vec<(Label,Label)>,HashMap<Label,Label>,Vec<(Label, String)>) {
    let labels = p.labels();
    let diagram_indirect = p.diagram_indirect.as_ref().unwrap();
    let successors =  diagram_indirect_to_reachability_adj(&labels, diagram_indirect);

    let rcs = right_closed_subsets(&labels, &successors);
    
    let mapping_label_rightclosed : HashMap<Label,Vec<Label>> = labels.iter().map(|&l|{
        let mut r = successors[&l].clone();
        r.insert(l);
        (l,r.into_iter().sorted().collect())
    }).collect();

    let mapping_rightclosed_newlabel : HashMap<Vec<Label>,Label> = rcs.iter().enumerate().map(|(i,r)|{
        (r.clone(),i as Label)
    }).collect();

    let mut diagram = vec![];
    
    for s1 in &rcs {
        for s2 in &rcs {
            let set_s1 : HashSet<Label> = HashSet::from_iter(s1.iter().cloned());
            let set_s2 : HashSet<Label> = HashSet::from_iter(s2.iter().cloned());
            let new_s1 = mapping_rightclosed_newlabel[s1];
            let new_s2 = mapping_rightclosed_newlabel[s2];
            if set_s1.is_superset(&set_s2) {
                diagram.push((new_s1,new_s2));
            }
        }
    }

    let oldtext : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();

    let onechar = oldtext.iter().all(|(_,t)|t.len() == 1);

    let new_mapping_label_text : Vec<_> = mapping_rightclosed_newlabel.iter().map(|(r,&l)|{
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

    let mut mapping_label_newlabel = HashMap::new();
    for &l in &labels {
        let newl = mapping_rightclosed_newlabel[&mapping_label_rightclosed[&l]];
        mapping_label_newlabel.insert(l, newl);
    }

    (diagram,mapping_label_newlabel,new_mapping_label_text)
}


impl Problem {
    pub fn fixpoint(&self, eh: &mut EventHandler) -> Self {
        
        let orig_diagram = self.diagram_indirect.as_ref().unwrap();
        let orig_mapping : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let (mut diagram,mut mapping_label_newlabel,mut mapping_newlabel_text) = first_diagram(self);

        let active = self.active.all_choices(true);
        let passive = self.passive.all_choices(true);
        let active = Constraint{ lines: active, is_maximized: false, degree: self.active.degree  };
        let passive = Constraint{ lines: passive, is_maximized: false, degree: self.passive.degree  };

        let mut all_expressions : HashSet<TreeNode<Label>> = HashSet::new();

        let p = loop {
            let active = active.edited(|g| Group(vec![mapping_label_newlabel[&g.0[0]]]));
            let passive = passive.edited(|g| Group(vec![mapping_label_newlabel[&g.0[0]]]));

            let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
            let diagram_indirect = diagram_to_indirect(&newlabels,&diagram);
            let diagram_indirect_rev = diagram_indirect.iter().map(|&(a,b)|(b,a)).collect();

            let mut tracking = CHashMap::new();
            let mut tracking_passive = CHashMap::new();

            let active = procedure(&active, &newlabels, &diagram_indirect, &mapping_newlabel_text, Some(&tracking), eh);
            let passive = procedure(&passive, &newlabels, &diagram_indirect_rev, &mapping_newlabel_text, Some(&tracking_passive), eh);

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
            };

            p.mapping_label_text = mapping_newlabel_text.clone();

            p.compute_triviality(eh);


            println!("zero round");
            
            if !p.trivial_sets.as_ref().unwrap().is_empty() {
                let mapping : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();
                let mut expressions = HashSet::new();
                for line in p.active.lines.iter() {
                    let len = if let Some(rg) = tracking.get(line) {
                        let (_,_,before_norm,_,_) = &*rg;
                        before_norm.parts.len()
                    } else {
                        line.parts.len()
                    };
                    for i in 0..len {
                        let expr = expression_for_line_at(line,i,false, &tracking,&mapping).reduce_rep();
                        expr.get_all_subexpressions(&mut expressions);
                    }
                }
                let mut expressions_passive = HashSet::new();
                for line in passive_before_edit.lines.iter() {
                    let len = if let Some(rg) = tracking.get(line) {
                        let (_,_,before_norm,_,_) = &*rg;
                        before_norm.parts.len()
                    } else {
                        line.parts.len()
                    };
                    for i in 0..len {
                        let expr = expression_for_line_at(line,i,false, &tracking_passive,&mapping).reduce_rep();
                        expr.get_all_subexpressions(&mut expressions_passive);
                    }
                }

                for expr in expressions_passive {
                    expressions.insert(expr.flip());
                }

                for (_,&x) in &mapping_label_newlabel {
                    expressions.insert(TreeNode::Terminal(x));
                }

                for e in &all_expressions {
                    expressions.insert(e.convert(&mapping_label_newlabel));
                }

                println!("The problem is trivial, there are {} subexpressions",expressions.len());

                let map_label_expr : HashMap<_,_> = expressions.iter().cloned().enumerate().map(|(a,b)|(a as Label,b)).collect();
                let map_expr_label : HashMap<_,_> = expressions.iter().cloned().enumerate().map(|(a,b)|(b,a as Label)).collect();
                
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

                let label_to_oldlabel : HashMap<_,_> = mapping_label_newlabel.iter().map(|(&l,&n)|(n,l)).collect();

                diagram = new_diagram;
                mapping_newlabel_text = map_label_expr.iter().map(|(&l,e)|{
                    (l,e.convert(&mapping).to_string()) 
                    /*let mut s = e.result().into_iter().sorted().map(|x|&mapping[&x]).join("");
                    if s == "" {
                        s.push('_');
                    }
                    (l,s)*/
                }).collect();


                mapping_label_newlabel = map_label_expr.iter().filter_map(|(&l,e)| match e {
                    TreeNode::Terminal(x) => { Some((label_to_oldlabel[x],l)) },
                    TreeNode::Expr(_,_,_) => None
                }).collect();

                for (_,e) in &map_label_expr {
                    all_expressions.insert(e.convert(&label_to_oldlabel));
                }

                for (x,y) in orig_diagram {
                    diagram.push((mapping_label_newlabel[x],mapping_label_newlabel[y]));
                }

                let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
                diagram = diagram_to_indirect(&newlabels,&diagram);
                diagram.sort();

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

                let max = mapping_newlabel_text.iter().map(|(l,_)|l).max().unwrap();
                let source = (max+1) as Label;
                let sink = (max+2) as Label;
                for &(l,_) in &mapping_newlabel_text {
                    diagram.push((source,l));
                    diagram.push((l,sink));
                }
                mapping_newlabel_text.push((source,"S".to_owned()));
                mapping_newlabel_text.push((sink,"T".to_owned()));


                let newlabels : Vec<Label> = mapping_newlabel_text.iter().map(|&(l,_)|l).collect();
                let (merges,direct) = compute_direct_diagram(&newlabels, &diagram);
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
                /*
                let newmapping : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();
                for (a,b) in &direct {
                    println!("{} {}",newmapping[a].replace(" ",""),newmapping[b].replace(" ",""));
                }
                println!("{:?}",mapping_newlabel_text);
                println!("{:?}",diagram);
                println!("{:?}",mapping_label_newlabel);*/

            } else {
                break p;
            }
        };



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


fn procedure(constraint : &Constraint, labels : &[Label], diagram_indirect : &Vec<(Label, Label)>, mapping : &Vec<(Label, String)>, mut tracking : Option<&CHashMap<Line, (Line, Line, Line, Vec<Vec<usize>>, Vec<(usize, usize, Operation)>)>>, eh: &mut EventHandler) -> Constraint {
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
            //assert!(common.len() == 1);
            unions.insert((l1,l2),common.into_iter().next().unwrap());

            let mut common : HashSet<Label> = predecessors[&l1].intersection(&predecessors[&l2]).cloned().collect();
            for l in common.clone().into_iter() {
                for r in predecessors[&l].iter().filter(|&&x|x != l) {
                    common.remove(r);
                }
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

    newconstraint
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