use std::{collections::{HashMap, HashSet}, fmt::Display, ops::Deref};
use std::hash::Hash;

use dashmap::DashMap as CHashMap;
use itertools::Itertools;
use petgraph::{algo::toposort, Graph};

use crate::{algorithms::{diagram::{compute_direct_diagram, diagram_indirect_to_reachability_adj, diagram_to_indirect}, event::EventHandler, fixpoint::{expression_for_line_at, TreeNode}}, group::Label, problem::Problem};




#[derive(Clone,Hash,Eq,PartialEq,PartialOrd, Ord,Debug)]
enum Expr<T : Hash + Clone + Eq + PartialEq + PartialOrd + Ord> {
    Base(T,bool),
    Left(Box<Expr<T>>,Box<Expr<T>>),
    Right(Box<Expr<T>>,Box<Expr<T>>)
}

#[derive(Clone,Hash,Eq,PartialEq,Debug,PartialOrd, Ord)]
enum E<T : Hash + Clone + Eq + PartialEq + PartialOrd + Ord>{
    Base(T),
    Mirror(Box<E<T>>),
    Left(Box<E<T>>,Box<E<T>>),
    Right(Box<E<T>>,Box<E<T>>)
}

impl<T> E<T> where T: Hash + Clone + Eq + PartialEq + PartialOrd + Ord{
    #[allow(dead_code)]
    fn label(s : T) -> E<T> {
        E::Base(s)
    }
    #[allow(dead_code)]
    fn mirror(e : E<T>) -> E<T> {
        E::Mirror(Box::new(e))
    }
    #[allow(dead_code)]
    fn left(e1 : E<T>, e2 : E<T>) -> E<T> {
        E::Left(Box::new(e1),Box::new(e2))
    }
    #[allow(dead_code)]
    fn right(e1 : E<T>, e2 : E<T>) -> E<T> {
        E::Right(Box::new(e1),Box::new(e2))
    }

    fn as_expr(&self) -> Expr<T> {
        self.as_expr_aux(false)
    }

    fn as_expr_aux(&self, flip : bool) -> Expr<T> {
        match self {
            E::Mirror(e) => {
                e.as_expr_aux(!flip)
            },
            E::Base(s) => {
                Expr::Base(s.clone(), flip)
            },
            E::Left(e1, e2) => {
                let e1 = Box::new(e1.as_expr_aux(flip));
                let e2 = Box::new(e2.as_expr_aux(flip));
                if flip {
                    Expr::Right(e1,e2)
                } else {
                    Expr::Left(e1,e2)
                }
            },
            E::Right(e1, e2) => {
                let e1 = Box::new(e1.as_expr_aux(flip));
                let e2 = Box::new(e2.as_expr_aux(flip));
                if flip {
                    Expr::Left(e1,e2)
                } else {
                    Expr::Right(e1,e2)
                }
            }
        }
    }
}

impl<T> Expr<T> where T : Hash + Clone + Eq + PartialEq + PartialOrd + Ord{
    #[allow(dead_code)]
    fn label(s : T) -> Expr<T> {
        Expr::Base(s.clone(), false)
    }
    #[allow(dead_code)]
    fn mirror(s : T) -> Expr<T> {
        Expr::Base(s.clone(), true)
    }
    #[allow(dead_code)]
    fn left(e1 : Expr<T>, e2 : Expr<T>) -> Expr<T> {
        Expr::Left(Box::new(e1),Box::new(e2))
    }
    #[allow(dead_code)]
    fn right(e1 : Expr<T>, e2 : Expr<T>) -> Expr<T> {
        Expr::Right(Box::new(e1),Box::new(e2))
    }

    /* 
    fn is_pred_partial(&self, e : &Expr<T>, context : &mut Relations<T>) -> bool {
        match (self,e) {
            (Expr::Left(e1, e2), Expr::Left(e3, e4)) => {
                if e1 == e3 && *context.get(&(e2.deref().clone(),e4.deref().clone())).unwrap_or(&false) {
                    return true;
                }
                if e2 == e4 && *context.get(&(e1.deref().clone(),e3.deref().clone())).unwrap_or(&false) {
                    return true;
                }
            },
            (Expr::Right(e1, e2), Expr::Right(e3, e4)) => {
                if e1 == e3 && *context.get(&(e2.deref().clone(),e4.deref().clone())).unwrap_or(&false) {
                    return true;
                }
                if e2 == e4 && *context.get(&(e1.deref().clone(),e3.deref().clone())).unwrap_or(&false) {
                    return true;
                }
            },
            _ => {}
        }
        false
    }*/

    fn is_left(&self) -> bool {
        match self {
            Expr::Left(_, _) => { true },
            _ => { false }
        }
    }

    /* 
    fn is_right(&self) -> bool {
        match self {
            Expr::Right(_, _) => { true },
            _ => { false }
        }
    }*/

    fn is_pred(&self, e : &Expr<T>, context : &mut Relations<T>) -> bool {
        if self == e {
            return true;
        }
        if let Some(&result) = context.get(&(self.clone(),e.clone())) {
            return result;
        }
        match e {
            Expr::Right(e1,e2) => {
                if self.is_pred(e1, context) {
                    //println!("{} -> {} because {} -> {}",self,e,self,e1);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
                if self.is_pred(e2, context) {
                    //println!("{} -> {} because {} -> {}",self,e,self,e2);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
            }
            Expr::Left(e1,e2) => {
                if self.is_pred(e1, context) && self.is_pred(e2, context) {
                    //println!("{} -> {} because {} -> {} and {} -> {}",self,e,self,e1,self,e2);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
            }
            _ => {}
        }
        match self {
            Expr::Right(e1,e2) => {
                if e1.is_pred(e, context) && e2.is_pred(e, context) {
                    //println!("{} -> {} because {} -> {} and {} -> {}",self,e,e1,e,e2,e);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
            }
            Expr::Left(e1,e2) => {
                if e1.is_pred(e, context) {
                    //println!("{} -> {} because {} -> {}",self,e,e1,e);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
                if e2.is_pred(e, context) {
                    //println!("{} -> {} because {} -> {}",self,e,e2,e);
                    context.insert((self.clone(),e.clone()),true);
                    return true;
                }
            }
            _ => {}
        }

        context.insert((self.clone(),e.clone()),false);
        false
    }

    #[allow(dead_code)]
    fn all_subexprs(&self, set : &mut HashSet<Expr<T>>) {
        set.insert(self.clone());
        match self {
            Expr::Left(e1, e2) => { e1.all_subexprs(set); e2.all_subexprs(set); },
            Expr::Right(e1, e2) => { e1.all_subexprs(set); e2.all_subexprs(set); },
            _ => {}
        }

    }

    fn convert<U>(&self, mapping : &HashMap<T,U>) -> Expr<U> where U : Hash + Clone + Eq + PartialEq + PartialOrd + Ord {
        match self {
            Expr::Base(e, b) => Expr::Base(mapping[e].clone(),*b),
            Expr::Left(e1, e2) => { Expr::left(e1.convert(mapping),e2.convert(mapping)) },
            Expr::Right(e1, e2) => { Expr::right(e1.convert(mapping),e2.convert(mapping)) }
        }
    }
    fn as_e(&self) -> E<T> {
        match self {
            Expr::Base(x, b) => {
                if *b {
                    E::mirror(E::Base(x.clone()))
                } else {
                    E::Base(x.clone())
                }
            },
            Expr::Left(e1, e2) => {
                E::left(e1.as_e(),e2.as_e())
            },
            Expr::Right(e1, e2) => {
                E::right(e1.as_e(),e2.as_e())
            },
        }
    }

    fn mirrored(&self) -> Self {
        E::mirror(self.as_e()).as_expr()
    }


    fn reduce(&self, relations : &mut Relations<T>) -> Expr<T> {
        match self {
            Expr::Left(e1, e2) => {
                        if e1.is_pred(e2, relations){ return e1.reduce(relations); }
                        if e2.is_pred(e1, relations){ return e2.reduce(relations); }
                        return Expr::left(e1.reduce(relations),e2.reduce(relations));
                    },
            Expr::Right(e1, e2) => { 
                        if e1.is_pred(e2, relations){ return e2.reduce(relations); }
                        if e2.is_pred(e1, relations){ return e1.reduce(relations); }
                        return Expr::right(e1.reduce(relations),e2.reduce(relations));
                    },
            Expr::Base(_, _) => { return self.clone(); },
        }
    }
}

impl<T> Display for Expr<T> where T : Display + Hash  + Clone + Eq + PartialEq + PartialOrd + Ord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Base(s, m) => {
                if *m {
                    write!(f,"m[{}]",s)
                } else {
                    write!(f,"{}",s)
                }
            },
            Expr::Left(e1, e2) => {
                write!(f,"[{}←{}]",e1,e2)
            },
            Expr::Right(e1, e2) => {
                write!(f,"[{}→{}]",e1,e2)
            },
        }
    }
}


type Relations<T> = HashMap<(Expr<T>,Expr<T>),bool>;

#[derive(Default)]
struct Context<T : Hash + Clone + Eq + PartialEq + Ord + PartialOrd> {
    expressions : HashSet<Expr<T>>,
    relations : Relations<T>,
    mapping_label_text : HashMap<Label,String>,
    mapping_expressions_to_ids : HashMap<Expr<Label>,Label>,
    mapping_ids_to_expressions : HashMap<Label,Expr<Label>>,
    fresh_id : Label,
    successors : HashMap<Label,HashSet<Label>>,
    predecessors : HashMap<Label,HashSet<Label>>,
    diagram : Vec<(Label,Label)>,
    diagram_reversed : Vec<(Label,Label)>
}

impl<T> TreeNode<T> where T : Hash + Clone + Eq + PartialEq + Ord + PartialOrd {
    fn to_expr(&self) -> E<T> {
        match self {
            TreeNode::Terminal(x) => E::Base(x.clone()),
            TreeNode::Expr(e1, e2, op) => {
                match op {
                    super::maximize::Operation::Union => { E::right(e1.to_expr(), e2.to_expr()) },
                    super::maximize::Operation::Intersection => { E::left(e1.to_expr(), e2.to_expr()) }
                }
            }
        }
    }
}

impl Context<Label> {
    fn update_diag_succ_pred_from_relations(&mut self) {
        let diagram = self.compute_diagram_from_relations();
        let diagram_reversed = diagram.iter().cloned().map(|(a,b)|(b,a)).collect_vec();
        let ids = self.mapping_ids_to_expressions.keys().cloned().collect_vec();
        self.successors = diagram_indirect_to_reachability_adj(&ids, &diagram);
        self.predecessors = diagram_indirect_to_reachability_adj(&ids, &diagram_reversed);
        self.diagram = diagram;
        self.diagram_reversed = diagram_reversed;
    }

    fn add_expression(&mut self, e : &Expr<Label>) {
        self.fresh_id += 1;
        self.expressions.insert(e.clone());
        self.mapping_expressions_to_ids.insert(e.clone(),self.fresh_id);
        self.mapping_ids_to_expressions.insert(self.fresh_id,e.clone());
        self.successors.entry(self.fresh_id).or_default().insert(self.fresh_id);
        self.predecessors.entry(self.fresh_id).or_default().insert(self.fresh_id);
        self.update_relations_of_expression(e);
    }

    fn remove_expression(&mut self, e : &Expr<Label>) {
        self.expressions.remove(e);
        let id = self.mapping_expressions_to_ids[e];
        for &pred in &self.predecessors[&id] {
            self.successors.get_mut(&pred).unwrap().remove(&id);
        }
        for &succ in &self.successors[&id] {
            self.predecessors.get_mut(&succ).unwrap().remove(&id);
        }
        self.successors.remove(&id);
        self.predecessors.remove(&id);
        self.mapping_ids_to_expressions.remove(&id);
        self.mapping_expressions_to_ids.remove(e);
    }

    fn add_relation(&mut self, e1 : &Expr<Label>, e2 : &Expr<Label>) {
        self.relations.insert((e1.clone(),e2.clone()), true);
    }

    fn compute_diagram_from_relations(&self) -> Vec<(Label,Label)> {
        let expressions = &self.expressions;
        let relations = &self.relations;

        let node_to_id = &self.mapping_expressions_to_ids;
        let mut diagram = vec![];

        for e1 in expressions {
            for e2 in expressions {
                if e1 == e2 {
                    diagram.push((node_to_id[e1],node_to_id[e2]));
                } else if *relations.get(&(e1.clone(),e2.clone())).unwrap_or(&false) {
                    diagram.push((node_to_id[e1],node_to_id[e2]));
                }
            }
        }

        let diagram = diagram_to_indirect(&node_to_id.values().cloned().collect_vec(), &diagram);

        diagram
    }

    fn init_from_problem(p : &Problem) -> Self {
        let mut context = Self::default();
        context.mapping_label_text = p.mapping_label_text.iter().cloned().collect();

        let succs = p.diagram_indirect_to_reachability_adj();
        let labels = p.labels();

        for &label in &labels {
            let expr_label = Expr::Base(label,false);
            let expr_mirror_label = Expr::Base(label,true);

            context.add_expression(&expr_label);
            context.add_expression(&expr_mirror_label);

            for &succ in &succs[&label] {
                let expr_succ = Expr::Base(succ,false);
                context.add_relation(&expr_label,&expr_succ);
            }
            for &succ in &succs[&label] {
                let expr_mirror_succ = Expr::Base(succ,true);
                context.add_relation(&expr_mirror_succ,&expr_mirror_label);
            }
            for compatible in p.labels_compatible_with_label(label) {
                let expr_compatible = Expr::Base(compatible, false);
                context.add_relation(&expr_mirror_label,&expr_compatible);
            }
        }

        context.fix_relations();
        
        context.update_diag_succ_pred_from_relations();

        context
    }


    fn leftmost_expressions(&mut self, expressions: &HashSet<Expr<u32>>) -> HashSet<Expr<Label>> {

        let mut result : HashSet<_> = expressions.iter().cloned().collect();

        for s1 in expressions {
            for s2 in expressions {
                if s1 != s2 {
                    if result.contains(s1) && result.contains(s2) && s1.is_pred(s2, &mut self.relations) {
                        result.remove(s2);
                    }
                }
            }
        }

        result
    }

    fn diagram_for_fixpoint_procedure(&self) -> (Vec<(Label,Label)>,Vec<(Label,Label)>,Vec<(Label,String)>,Vec<(Label,Expr<Label>)>) {
        let ids : HashSet<Label> = self.expressions.iter().map(|e|self.mapping_expressions_to_ids[e]).collect();
        
        let diagram = self.diagram.iter().filter(|(a,b)| ids.contains(a) && ids.contains(b) ).cloned().collect_vec();
        let mapping_node_to_id = &self.mapping_expressions_to_ids;

        let (equiv,diagram) = compute_direct_diagram(&ids.iter().cloned().collect_vec(), &diagram);
        let mapping_id_to_node : HashMap<_,_> = self.mapping_ids_to_expressions.iter().filter(|(id,_)|ids.contains(id)).map(|(id,e)|(*id,e.clone())).collect();
        let mapping_label_text = &self.mapping_label_text;

        let mut mapping_label_newlabel = vec![];
        let mut mapping_newlabel_text = vec![];

        for (_,v) in equiv {
            if v.len() != 1 {
                println!("{}",v.iter().map(|id|mapping_id_to_node[id].convert(&mapping_label_text)).join("      "));
                panic!("got equivalent labels while fixing the diagram");
            }
        }
        for &label in mapping_label_text.keys() {
            let expr = Expr::Base(label.clone(), false);
            let id = mapping_node_to_id[&expr];
            mapping_label_newlabel.push((label,id));
        }

        for newlabel in ids {
            mapping_newlabel_text.push((newlabel,format!("({})",newlabel)));
        }

        (diagram,mapping_label_newlabel,mapping_newlabel_text,mapping_id_to_node.into_iter().collect_vec())        
    }

    fn print_diagram(&self) {
        let (diagram,mapping_label_newlabel,mapping_newlabel_text,mapping_newlabel_expr) = self.diagram_for_fixpoint_procedure();
        let mapping_newlabel_text : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();
        let mapping_label_newlabel : HashMap<_,_> = mapping_label_newlabel.iter().cloned().collect();
        let mapping_label_text = &self.mapping_label_text;
        let mapping_newlabel_expr : HashMap<_,_> = mapping_newlabel_expr.iter().cloned().collect();


        for (l,n) in &mapping_label_newlabel {
            println!("{} = {}",mapping_label_text[&l],mapping_newlabel_text[&n]);
        }
        for (a,b) in &diagram {
            println!("{} -> {}",mapping_newlabel_text[&a],mapping_newlabel_text[&b]);
        }
        println!("");
        //for (l,_) in &mapping_label_newlabel {
        //    println!("{} = ({})",mapping_label_text[&l],mapping_label_text[&l]);
        //}
        for (a,b) in &diagram {
            println!("\"{}\" \"{}\"",mapping_newlabel_expr[a].convert(&mapping_label_text),mapping_newlabel_expr[b].convert(&mapping_label_text));
        }
        println!("");
    }

    

    fn fix_relations(&mut self){

        for e1 in &self.expressions {
            for e2 in &self.expressions {
                if e1 != e2 {
                    let is_pred = e1.is_pred(&e2, &mut self.relations);
                    if is_pred {
                        if let Some(&id_e1) = self.mapping_expressions_to_ids.get(e1) {
                            if let Some(&id_e2) = self.mapping_expressions_to_ids.get(e2) {
                                if self.successors.entry(id_e1).or_default().insert(id_e2) {
                                    self.diagram.push((id_e1,id_e2));
                                    self.diagram_reversed.push((id_e2,id_e1));
                                }
                                self.predecessors.entry(id_e2).or_default().insert(id_e1);
                            }
                        }
                    }
                }
            }
        }

    } 


    fn update_relations_of_expression(&mut self, expr : &Expr<Label>){

        for other in &self.expressions {
            if expr != other {
                let is_pred = expr.is_pred(&other, &mut self.relations);
                if is_pred {
                    //println!("found new relation {}  ->   {}",expr.convert(&self.mapping_label_text),other.convert(&self.mapping_label_text));
                    if let Some(&id_e1) = self.mapping_expressions_to_ids.get(expr) {
                        if let Some(&id_e2) = self.mapping_expressions_to_ids.get(other) {
                            //println!("they are both expressions of the diagram");
                            if self.successors.entry(id_e1).or_default().insert(id_e2) {
                                //println!("this relation was not in the diagram");
                                self.diagram.push((id_e1,id_e2));
                                self.diagram_reversed.push((id_e2,id_e1));
                            }
                            self.predecessors.entry(id_e2).or_default().insert(id_e1);
                        }
                    }
                }
                let is_pred = other.is_pred(&expr, &mut self.relations);
                if is_pred {
                    //println!("found new relation {}  ->   {}",other.convert(&self.mapping_label_text),expr.convert(&self.mapping_label_text));
                    if let Some(&id_e1) = self.mapping_expressions_to_ids.get(other) {
                        if let Some(&id_e2) = self.mapping_expressions_to_ids.get(expr) {
                            //println!("they are both expressions of the diagram");
                            if self.successors.entry(id_e1).or_default().insert(id_e2) {
                                //println!("this relation was not in the diagram");
                                self.diagram.push((id_e1,id_e2));
                                self.diagram_reversed.push((id_e2,id_e1));
                            }
                            self.predecessors.entry(id_e2).or_default().insert(id_e1);
                        }
                    }
                }
            }
        }

    } 



    fn find_good_expression_to_add(&mut self, definitely_fixed : &mut HashSet<Label>, eh : &mut EventHandler) -> Option<Expr<Label>> {
        println!("computing toposort data");
        let reachable = &self.predecessors;
        /*let diagram = self.diagram.iter().filter(|(a,b)|{
            match (self.mapping_ids_to_expressions.get(a),self.mapping_ids_to_expressions.get(b)) {
                (Some(e1), Some(e2)) => { !definitely_fixed.contains(e1) && !definitely_fixed.contains(e2) },
                _ => { false }
            }
        });

        let diagram_without_selfloops = diagram.filter(|(a,b)|{ a!=b });*/

        let ids : HashSet<_> = self.mapping_ids_to_expressions.keys().cloned().collect();
        let nodes = ids.difference(&definitely_fixed).cloned().collect_vec();
        let mut graph = Graph::<(), ()>::new();
        let mut ids_to_graphids = vec![];
        for &v in &nodes {
            let id_v = graph.add_node(());
            ids_to_graphids.push((v,id_v));
        }
        let mapping_ids_to_graphids : HashMap<_,_> = ids_to_graphids.iter().cloned().collect();

        for &v in &nodes {
            let succ = self.successors[&v].iter().cloned();
            for u in succ {
                if v != u {
                    graph.add_edge(mapping_ids_to_graphids[&v], mapping_ids_to_graphids[&u], ());
                }
            }
        }


        let mapping_graphids_to_ids : HashMap<_,_> = ids_to_graphids.iter().cloned().map(|(a,b)|(b,a)).collect();
        //println!("nodes of the graph {}",.join(" "));
        //println!("{:?}",diagram_without_selfloops.clone().collect_vec());

        println!("toposort");
        let sorted_nodes = toposort(&graph,None).unwrap().into_iter().map(|v|mapping_graphids_to_ids[&v]).collect_vec();
        //println!("toposorted {}",sorted_nodes.iter().unique().sorted().join(" "));

        println!("searching for fix");
        for i in 0..sorted_nodes.len() {
            if definitely_fixed.contains(&sorted_nodes[i]) {
                continue;
            }
            for j in i+1..sorted_nodes.len() {
                let id_e1 = sorted_nodes[i];
                let id_e2 = sorted_nodes[j];
                let reachable_e1 = &reachable[&id_e1];
                let reachable_e2 = &reachable[&id_e2];
                let mut reachable_intersection : HashSet<Label> = reachable_e1.intersection(reachable_e2).cloned().collect();
                
                for l in reachable_intersection.clone().into_iter() {
                    for r in reachable[&l].iter().filter(|&&x|x != l) {
                        reachable_intersection.remove(r);
                    }
                }

                if reachable_intersection.len() != 1 {
                    if definitely_fixed.contains(&id_e1) || definitely_fixed.contains(&id_e2) {
                        panic!("sebastian is wrong");
                    }
                    let result = Expr::left(self.mapping_ids_to_expressions[&id_e1].clone(),self.mapping_ids_to_expressions[&id_e2].clone());
                    println!("found in position {} {}, total nodes {}",i,j,sorted_nodes.len());
                    eh.notify("fixpoint autofix", 0, sorted_nodes.len());
                    return Some(result);
                }
            }
            definitely_fixed.insert(sorted_nodes[i]);
        }

        None
    }



    fn discard_useless_expressions(&mut self, new_expressions : &HashSet<Expr<Label>>, added : &Expr<Label>) {

        match added {
            Expr::Left(e1, e2) => {
                if e1.is_left() && new_expressions.contains(e1) && self.predecessors[&self.mapping_expressions_to_ids[e1.deref()]].len() == self.predecessors[&self.mapping_expressions_to_ids[added]].len() + 1 {
                    //println!("removing expression {}",e1.convert(&self.mapping_label_text));
                    self.remove_expression(e1);
                }
                if e2.is_left() && new_expressions.contains(e2) && self.predecessors[&self.mapping_expressions_to_ids[e2.deref()]].len() == self.predecessors[&self.mapping_expressions_to_ids[added]].len() + 1 {
                    //println!("removing expression {}",e2.convert(&self.mapping_label_text));
                    self.remove_expression(e2);
                }
            },
            /*Expr::Right(e1, e2) => {
                if e1.is_right() && new_expressions.contains(e1) {
                    println!("removing expression {}",e1.convert(&context.mapping_label_text));
                    context.expressions.remove(e1);
                }
                if e2.is_right() && new_expressions.contains(e2) {
                    println!("removing expression {}",e2.convert(&context.mapping_label_text));
                    context.expressions.remove(e2);
                }
            },*/
            _ => {  }
        };
    }
    
    fn fix_diagram(&mut self, eh : &mut EventHandler) {

        let mut new_expressions = HashSet::new();

        let sinks = self.expressions.iter().filter(|e|{
            let id = self.mapping_expressions_to_ids[e];
            self.successors[&id].len() == 1
        }).collect_vec();

        if sinks.len() != 1 {
            let mut sink = sinks[0].clone();
            for &e in sinks[1..].iter() {
                sink = Expr::right(sink, e.clone());
            }
            self.add_expression(&sink);
            new_expressions.insert(sink);
        }

        let mut definitely_fixed = HashSet::new();

        //println!("updating edges 1");
        //self.fix();
        //println!("done");

        loop{
            //self.print_diagram();
            
            if let Some(expr) = self.find_good_expression_to_add(&mut definitely_fixed, eh) {
                //let expr = expr.reduce(&mut context.relations);
                //println!("adding expression {}",expr.convert(&self.mapping_label_text));
                self.add_expression(&expr);
                new_expressions.insert(expr.clone());
                //self.fix_for_expression(&expr);
                self.discard_useless_expressions(&new_expressions, &expr);
                
                /*let mirrored = expr.mirrored();
                if !expr.is_pred(&mirrored, &mut context.relations) || !mirrored.is_pred(&expr, &mut context.relations) {
                    println!("adding expression {}",mirrored.convert(&mapping_label_text));
                    context.expressions.insert(mirrored.clone());
                    new_expressions.insert(mirrored.clone());
                    discard_useless_expressions(context, &new_expressions, &mirrored);
               }*/
            } else {
                break;
            }

        }

        println!("diagram fixed\n\n");
    }

}

impl Problem {


    fn nofixpoint(&self, eh : &mut EventHandler) -> Result<Problem,String> {
        let degree = self.active.finite_degree();

        let mut context = Context::init_from_problem(self);

        println!("starting diagram");
        context.print_diagram();
        
        let mut expr_to_check : Vec<Vec<(Expr<u32>, Expr<u32>)>> = vec![];

        loop {
            //context.fix();
            println!("fixing diagram");
            context.fix_diagram(eh);

            println!("new diagram");
            context.print_diagram();

            for not_all_of_these in &expr_to_check {
                if not_all_of_these.iter().all(|(m,e)|{
                    m.is_pred(e,&mut context.relations)
                }) {
                    println!("cannot get a fixed point!");
                    let mut s = String::new();
                    s += "No fixed point can be found. These expressions will be pairwise compatible with any diagram: ";
                    let exprs = not_all_of_these.iter().map(|(_,e)|e).unique();
                    for e in exprs {
                        s += &format!("{}, ",e.convert(&context.mapping_label_text));
                    }
                    return Err(s);
                }
            }

            println!("computing diagram for rerunning fp procedure");


            let (diagram,mapping_label_newlabel,mapping_newlabel_text,_) = context.diagram_for_fixpoint_procedure();
            println!("running fp procedure");
            let tracking = CHashMap::new();
            let tracking_passive = CHashMap::new();

            let (mut p,_) = self.fixpoint_onestep(false,&mapping_label_newlabel,&mapping_newlabel_text,&diagram,Some(&tracking),Some(&tracking_passive),eh).unwrap();

            println!("procedure terminated");

            p.compute_triviality(eh);
            let trivial_sets = p.trivial_sets.clone().unwrap();
            let mapping : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();
            let mapping_newlabel_label : HashMap<_,_> = mapping_label_newlabel.iter().cloned().map(|(l,n)|(n,l)).collect();
            if trivial_sets.is_empty() {
                println!("found a fixed point!\n{}",p);
                return Ok(p);
            } else {
                println!("did not find a fixed point");
                let mut expressions_to_add = HashSet::new();

                for line in p.active.lines {
                    if trivial_sets.iter().any(|sets|{
                        line.parts.iter().all(|part|{
                            sets.contains(&part.group.first())
                        })
                    }) {
                        let mut obtained_expressions = HashSet::new();
                        for i in 0..degree {
                            println!("\n\nposition {}",i);
                            let expr = expression_for_line_at(&line,i, &tracking,&mapping).reduce_rep();
                            let expr : E<Label> = expr.to_expr();
                            let e = expr.as_expr().convert(&mapping_newlabel_label);
                            obtained_expressions.insert(e);
                        }
                        let leftmost_expressions = context.leftmost_expressions(&obtained_expressions);
                        let mut obtained_pairs = vec![];
                        for e in &leftmost_expressions {
                            let me = e.mirrored();
                            println!("adding expressions {} and {}",e.convert(&context.mapping_label_text),me.convert(&context.mapping_label_text));
                            obtained_pairs.push((me.clone(),e.clone()));
                            e.all_subexprs(&mut expressions_to_add);
                            me.all_subexprs(&mut expressions_to_add);
                        }
                        let mut not_all_of_these = vec![];
                        for (me1,_) in &obtained_pairs {
                            for (_,e2) in &obtained_pairs {
                                not_all_of_these.push((me1.clone(),e2.clone()));
                            }
                        }
                        expr_to_check.push(not_all_of_these);
                    }
                }

                let mut really_to_add = vec![];
                'outer: for expr in expressions_to_add {
                    for existing in context.expressions.iter().chain(really_to_add.iter()) {
                        if existing.is_pred(&expr, &mut context.relations) && expr.is_pred(existing, &mut context.relations) {
                            continue 'outer;
                        }
                    }
                    really_to_add.push(expr);
                }
                for expr in really_to_add {
                    let reduced = expr.reduce(&mut context.relations);
                    context.add_expression(&reduced);
                }

            }

        }

    }

    


    

    pub fn fixpoint_loop(&self, eh: &mut EventHandler) -> Result<(Self,Vec<(Label,Label)>,Vec<(Label,Label)>), String> {
        self.nofixpoint(eh).map(|p|(p,vec![],vec![]))
    }


}







/* 
fn find_good_expression_to_add_old(context : &mut Context<Label>) -> Option<Expr<Label>> {
    println!("finding missing sources, current number of nodes is {}",context.expressions.len());
    let missing = nofixpoint_missing_sources_or_sinks(context,true);
    println!("done, got {}",missing.len());
    let old_expressions = context.expressions.clone();

    let mut to_add : HashSet<_> = missing.iter().cloned().collect();
    
    for s1 in &missing {
        for s2 in to_add.clone().into_iter() {
            if s1 != &s2 {
                if to_add.contains(s1) && s1.is_pred_partial(&s2, &mut context.relations) {
                    to_add.remove(&s2);
                }
            }
        }
    }
    let missing = to_add;

    context.expressions.extend(missing.iter().cloned());

    println!("filtered, got {}",missing.len());

    println!("updating edges 2");
    nofixpoint_fix_context(context);
    println!("done");

    let to_add = leftmost_expressions(context,&missing);

    context.expressions = old_expressions;
    to_add.iter().cloned().next()
}

fn nofixpoint_missing_sources_or_sinks(context : &Context<Label>, sources : bool) -> HashSet<Expr<Label>> {
    let mut missing = HashSet::new();

    let (diagram,mapping_node_to_id) = nofixpoint_context_to_diagram(context);
    let ids = mapping_node_to_id.values().cloned().collect_vec();
    let reachable = if !sources {
        diagram_indirect_to_reachability_adj(&ids, &diagram)
    } else {
        let reverse_diagram = diagram.iter().cloned().map(|(a,b)|(b,a)).collect_vec();
        diagram_indirect_to_reachability_adj(&ids, &reverse_diagram)
    };
    
    //let mapping_id_to_node : HashMap<_,_> = mapping_node_to_id.iter().map(|(node,id)|(*id,node.clone())).collect();

    for e1 in &context.expressions {
        for e2 in &context.expressions {
            if e1 < e2 {
                let id_e1 = mapping_node_to_id[e1];
                let id_e2 = mapping_node_to_id[e2];
                let reachable_e1 = &reachable[&id_e1];
                let reachable_e2 = &reachable[&id_e2];
                let mut reachable_intersection : HashSet<Label> = reachable_e1.intersection(reachable_e2).cloned().collect();
                
                for l in reachable_intersection.clone().into_iter() {
                    for r in reachable[&l].iter().filter(|&&x|x != l) {
                        reachable_intersection.remove(r);
                    }
                }

                if reachable_intersection.len() != 1 {
                    //println!("{} and {} have non unique sink",id_e1,id_e2);
                    //println!("{} and {} have non unique sink: {}",e1.convert(mapping_label_text),e2.convert(mapping_label_text),succ_intersection.iter().map(|x|mapping_id_to_node[x].convert(mapping_label_text)).join(" "));
                    if sources {
                        missing.insert(Expr::left(e1.clone(),e2.clone()));
                    } else {
                        missing.insert(Expr::right(e1.clone(),e2.clone()));
                    }
                    
                }
            }
        }
    }

    missing
}*/


