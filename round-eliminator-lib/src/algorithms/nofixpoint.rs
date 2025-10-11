use std::{clone, collections::{HashMap, HashSet}, fmt::Display, ops::Deref};
use std::hash::Hash;

use dashmap::DashMap as CHashMap;
use itertools::Itertools;

use crate::{algorithms::{diagram::{compute_direct_diagram, diagram_indirect_to_reachability_adj, diagram_to_indirect}, event::EventHandler, fixpoint::{expression_for_line_at, FixpointDiagram, TreeNode}}, group::Label, problem::Problem};




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
struct Context<T : Hash + Clone + Eq + PartialEq + Ord + PartialOrd> {
    expressions : HashSet<Expr<T>>,
    relations : Relations<T>
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

impl Problem {
    fn nofixpoint_initial_context(&self) -> Context<Label> {
        let succs = self.diagram_indirect_to_reachability_adj();
        let labels = self.labels();
        let mut expressions = HashSet::new();
        let mut relations = HashMap::new();

        for &label in &labels {
            let expr_label = Expr::Base(label,false);
            let expr_mirror_label = Expr::Base(label,true);

            expressions.insert(expr_label.clone());
            expressions.insert(expr_mirror_label.clone());

            for &succ in &succs[&label] {
                let expr_succ = Expr::Base(succ,false);
                relations.insert((expr_label.clone(),expr_succ),true);
            }
            for &succ in &succs[&label] {
                let expr_mirror_succ = Expr::Base(succ,true);
                relations.insert((expr_mirror_succ,expr_mirror_label.clone()),true);
            }
            for compatible in self.labels_compatible_with_label(label) {
                let expr_compatible = Expr::Base(compatible, false);
                relations.insert((expr_mirror_label.clone(),expr_compatible),true);
            }
        }
        
        Context{expressions,relations}
    }

    fn nofixpoint(&self) -> Option<Problem> {
        let mapping_label_text : HashMap<_, _> = self.mapping_label_text.iter().cloned().collect();

        let eh = &mut EventHandler::null();

        let mut context = self.nofixpoint_initial_context();
        
        let mut expr_to_check : Vec<Vec<(Expr<u32>, Expr<u32>)>> = vec![];

        loop {
            nofixpoint_fix_context(&mut context);
            println!("fixing diagram");
            self.nofixpoint_fix_diagram(&mut context);


            println!("new diagram");
            self.nofixpoint_print_diagram(&context);

            for not_all_of_these in &expr_to_check {
                if not_all_of_these.iter().all(|(m,e)|{
                    m.is_pred(e,&mut context.relations)
                }) {
                    println!("cannot get a fixed point!");
                    return None;
                }
            }


            let (diagram,mapping_label_newlabel,mapping_newlabel_text,_) = self.nofixpoint_diagram(&context);

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
                return Some(p);
            } else {
                println!("did not find a fixed point");
                let mut expressions_to_add = HashSet::new();

                for line in p.active.lines {
                    if trivial_sets.iter().any(|sets|{
                        line.parts.iter().all(|part|{
                            sets.contains(&part.group.first())
                        })
                    }) {
                        let len = if let Some(rg) = tracking.get(&line) {
                            let (_,_,before_norm,_,_) = &*rg;
                            before_norm.parts.len()
                        } else {
                            line.parts.len()
                        };
                        let mut obtained_expressions = vec![];
                        for i in 0..len {
                            let expr = expression_for_line_at(&line,i,false, &tracking,&mapping).reduce_rep();
                            let expr : E<Label> = expr.to_expr();
                            let e = expr.as_expr().convert(&mapping_newlabel_label);
                            let me = E::mirror(expr).as_expr().convert(&mapping_newlabel_label);
                            println!("adding expressions {} and {}",e.convert(&mapping_label_text),me.convert(&mapping_label_text));
                            obtained_expressions.push((me.clone(),e.clone()));
                            e.all_subexprs(&mut expressions_to_add);
                            me.all_subexprs(&mut expressions_to_add);
                        }
                        let mut not_all_of_these = vec![];
                        for (me1,_) in &obtained_expressions {
                            for (_,e2) in &obtained_expressions {
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
                context.expressions.extend(really_to_add.into_iter());
            }

        }

    }

    fn nofixpoint_fix_diagram(&self, context : &mut Context<Label>) {
        let mapping_label_text : HashMap<_, _> = self.mapping_label_text.iter().cloned().collect();

        loop{
            nofixpoint_fix_context(context);
            let (missing_sources,missing_sinks) = nofixpoint_missing_sources_and_sinks(context,&mapping_label_text);
            let old_expressions = context.expressions.clone();

            context.expressions.extend(missing_sources.iter().cloned());
            context.expressions.extend(missing_sinks.iter().cloned());

            nofixpoint_fix_context(context);

            let mut sources_to_add : HashSet<_> = missing_sources.iter().cloned().collect();
            for s1 in &missing_sources {
                for s2 in &missing_sources {
                    if s1 != s2 {
                        if sources_to_add.contains(s1) && s1.is_pred(s2, &mut context.relations) {
                            sources_to_add.remove(s2);
                        }
                    }
                }
            }
            let mut sinks_to_add : HashSet<_> = missing_sinks.iter().cloned().collect();
            for s1 in &missing_sinks {
                for s2 in &missing_sinks {
                    if s1 != s2 {
                        if sinks_to_add.contains(s2) && s1.is_pred(s2, &mut context.relations) {
                            sinks_to_add.remove(s1);
                        }
                    }
                }
            }
            let mut stuff_to_add = sources_to_add.iter().chain(sinks_to_add.iter());
            context.expressions = old_expressions;
            if let Some(expr) = stuff_to_add.next() {
                println!("adding expression {}",expr.convert(&mapping_label_text));
                context.expressions.insert(expr.clone());
                let mirrored = expr.mirrored();
                if !expr.is_pred(&mirrored, &mut context.relations) || !mirrored.is_pred(expr, &mut context.relations) {
                    context.expressions.insert(mirrored);
                }
            } else {
                //let (missing_sources,missing_sinks) = nofixpoint_missing_sources_and_sinks(context,&mapping_label_text);
                break;
            }

        }



    }

    fn nofixpoint_print_diagram(&self, context : &Context<Label>) {
        let (diagram,mapping_label_newlabel,mapping_newlabel_text,mapping_newlabel_expr) = self.nofixpoint_diagram(context);
        let mapping_newlabel_text : HashMap<_,_> = mapping_newlabel_text.iter().cloned().collect();
        let mapping_label_newlabel : HashMap<_,_> = mapping_label_newlabel.iter().cloned().collect();
        let mapping_label_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        //let mapping_newlabel_expr : HashMap<_,_> = mapping_newlabel_expr.iter().cloned().collect();


        for (l,n) in &mapping_label_newlabel {
            println!("{} = {}",mapping_label_text[&l],mapping_newlabel_text[&n]);
        }
        for (a,b) in &diagram {
            println!("{} -> {}",mapping_newlabel_text[&a],mapping_newlabel_text[&b]);
        }
        /*println!("");
        for (l,_) in &mapping_label_newlabel {
            println!("{} = ({})",mapping_label_text[&l],mapping_label_text[&l]);
        }
        for (a,b) in diagram {
            println!("({}) -> ({})",mapping_newlabel_expr[&a].convert(&mapping_label_text),mapping_newlabel_expr[&b].convert(&mapping_label_text));
        }*/
    }

    fn nofixpoint_diagram(&self, context : &Context<Label>) -> (Vec<(Label,Label)>,Vec<(Label,Label)>,Vec<(Label,String)>,Vec<(Label,Expr<Label>)>) {
        let (diagram,mapping_node_to_id) = nofixpoint_context_to_diagram(&context);
        let ids = mapping_node_to_id.values().cloned().collect_vec();
        let (equiv,diagram) = compute_direct_diagram(&ids, &diagram);
        let mapping_id_to_node : HashMap<_,_> = mapping_node_to_id.iter().map(|(e,i)|(*i,e.clone())).collect();
        let mapping_label_text : HashMap<_, _> = self.mapping_label_text.iter().cloned().collect();

        let mut mapping_label_newlabel = vec![];
        let mut mapping_newlabel_text = vec![];

        for (_,v) in equiv {
            if v.len() != 1 {
                println!("{}",v.iter().map(|id|mapping_id_to_node[id].convert(&mapping_label_text)).join("      "));
                panic!("got equivalent labels while fixing the diagram");
            }
        }
        for label in self.labels() {
            let expr = Expr::Base(label.clone(), false);
            let id = mapping_node_to_id[&expr];
            mapping_label_newlabel.push((label,id));
        }

        for newlabel in ids {
            mapping_newlabel_text.push((newlabel,format!("({})",newlabel)));
        }

        (diagram,mapping_label_newlabel,mapping_newlabel_text,mapping_id_to_node.into_iter().collect_vec())        
    }

    pub fn fixpoint_loop(&self, eh: &mut EventHandler) -> Result<(Self,Vec<(Label,Label)>,Vec<(Label,Label)>), &'static str> {
        self.nofixpoint().map(|p|(p,vec![],vec![])).ok_or("No fixed point can be found.")
    }


}


fn nofixpoint_context_to_diagram(context : &Context<Label>) -> (Vec<(Label,Label)>, HashMap<Expr<Label>,Label>) {
    let expressions = &context.expressions;
    let relations = &context.relations;

    let node_to_id : HashMap<_,_> = expressions.iter().sorted().enumerate().map(|(i,e)|(e.clone(),i as Label)).collect();
    let mut diagram = vec![];

    for e1 in expressions {
        for e2 in expressions {
            if *relations.get(&(e1.clone(),e2.clone())).unwrap_or(&false) {
                diagram.push((node_to_id[e1],node_to_id[e2]));
            }
        }
    }

    let diagram = diagram_to_indirect(&node_to_id.values().cloned().collect_vec(), &diagram);

    (diagram,node_to_id)
}

fn nofixpoint_missing_sources_and_sinks(context : &Context<Label>, mapping_label_text : &HashMap<Label,String>) -> (HashSet<Expr<Label>>,HashSet<Expr<Label>>) {
    let mut missing_sources = HashSet::new();
    let mut missing_sinks = HashSet::new();

    let (diagram,mapping_node_to_id) = nofixpoint_context_to_diagram(context);
    let ids = mapping_node_to_id.values().cloned().collect_vec();
    let succ = diagram_indirect_to_reachability_adj(&ids, &diagram);
    let reverse_diagram = diagram.iter().cloned().map(|(a,b)|(b,a)).collect_vec();
    let pred = diagram_indirect_to_reachability_adj(&ids, &reverse_diagram);
    //let mapping_id_to_node : HashMap<_,_> = mapping_node_to_id.iter().map(|(node,id)|(*id,node.clone())).collect();

    for e1 in &context.expressions {
        for e2 in &context.expressions {
            if e1 < e2 {
                let id_e1 = mapping_node_to_id[e1];
                let id_e2 = mapping_node_to_id[e2];
                let succ_e1 = &succ[&id_e1];
                let succ_e2 = &succ[&id_e2];
                let mut succ_intersection : HashSet<Label> = succ_e1.intersection(succ_e2).cloned().collect();
                let pred_e1 = &pred[&id_e1];
                let pred_e2 = &pred[&id_e2];
                let mut pred_intersection : HashSet<Label> = pred_e1.intersection(pred_e2).cloned().collect();

                for l in succ_intersection.clone().into_iter() {
                    for r in succ[&l].iter().filter(|&&x|x != l) {
                        succ_intersection.remove(r);
                    }
                }

                for r in pred_intersection.clone().into_iter() {
                    for l in pred[&r].iter().filter(|&&x|x != r) {
                        pred_intersection.remove(l);
                    }
                }

                if succ_intersection.len() != 1 {
                    //println!("{} and {} have non unique sink",id_e1,id_e2);
                    //println!("{} and {} have non unique sink: {}",e1.convert(mapping_label_text),e2.convert(mapping_label_text),succ_intersection.iter().map(|x|mapping_id_to_node[x].convert(mapping_label_text)).join(" "));
                    missing_sinks.insert(Expr::right(e1.clone(),e2.clone()));
                }
                if pred_intersection.len() != 1 {
                    //println!("{} and {} have non unique source",id_e1,id_e2);
                    //println!("{} and {} have non unique source: {}",e1.convert(mapping_label_text),e2.convert(mapping_label_text),pred_intersection.iter().map(|x|mapping_id_to_node[x].convert(mapping_label_text)).join(" "));
                    missing_sources.insert(Expr::left(e1.clone(),e2.clone()));
                }
            }
        }
    }

    (missing_sources,missing_sinks)
}



fn nofixpoint_fix_context(context : &mut Context<Label>){

    for e1 in &context.expressions {
        for e2 in &context.expressions {
            if e1 != e2 {
                let _ = e1.is_pred(&e2, &mut context.relations);
            }
        }
    }

} 
