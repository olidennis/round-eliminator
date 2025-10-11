use std::{clone, collections::{HashMap, HashSet}, fmt::Display};
use std::hash::Hash;

use itertools::Itertools;

use crate::{algorithms::{diagram::{compute_direct_diagram, diagram_indirect_to_reachability_adj, diagram_to_indirect}, event::EventHandler}, group::Label, problem::Problem};




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


    fn is_pred(&self, e : &Expr<T>, context : &mut Context<T>) -> bool {
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

}

impl<T> Display for Expr<T> where T : Display + Hash  + Clone + Eq + PartialEq + PartialOrd + Ord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Base(s, m) => {
                if *m {
                    write!(f,"m({})",s)
                } else {
                    write!(f,"{}",s)
                }
            },
            Expr::Left(e1, e2) => {
                write!(f,"({}←{})",e1,e2)
            },
            Expr::Right(e1, e2) => {
                write!(f,"({}→{})",e1,e2)
            },
        }
    }
}

type Context<T> = HashMap<(Expr<T>,Expr<T>),bool>;

impl Problem {
    fn nofixpoint_initial_context(&self) -> (HashSet<Expr<Label>>,Context<Label>) {
        let succs = self.diagram_indirect_to_reachability_adj();
        let labels = self.labels();
        let mut expressions = HashSet::new();
        let mut context: Context<Label> = HashMap::new();

        for &label in &labels {
            let expr_label = Expr::Base(label,false);
            let expr_mirror_label = Expr::Base(label,true);

            expressions.insert(expr_label.clone());
            expressions.insert(expr_mirror_label.clone());

            for &succ in &succs[&label] {
                let expr_succ = Expr::Base(succ,false);
                context.insert((expr_label.clone(),expr_succ),true);
            }
            for &succ in &succs[&label] {
                let expr_mirror_succ = Expr::Base(succ,true);
                context.insert((expr_mirror_succ,expr_mirror_label.clone()),true);
            }
            for compatible in self.labels_compatible_with_label(label) {
                let expr_compatible = Expr::Base(compatible, false);
                context.insert((expr_mirror_label.clone(),expr_compatible),true);
            }
        }
        
        (expressions,context)
    }

    fn nofixpoint(&self) {
        let mapping_label_text : HashMap<_, _> = self.mapping_label_text.iter().cloned().collect();
        //println!("computing initial context");
        let (mut expressions,mut context) = self.nofixpoint_initial_context();

        //println!("adding arrows");
        loop {
            let before = context.clone();
            //println!("fixing context");
            nofixpoint_fix_context(&expressions, &mut context);

            //self.nofixpoint_print_diagram(&expressions, &context);

            loop{
                //println!("computing missing sources and sinks");
                let (missing_sources,missing_sinks) = nofixpoint_missing_sources_and_sinks(&expressions, &context,&mapping_label_text);
                //println!("adding missing sources and sinks");
                let mut new_expressions = expressions.clone();
                new_expressions.extend(missing_sources.iter().cloned());
                new_expressions.extend(missing_sinks.iter().cloned());

                nofixpoint_fix_context(&new_expressions,&mut context);

                let mut sources_to_add : HashSet<_> = missing_sources.iter().cloned().collect();
                for s1 in &missing_sources {
                    for s2 in &missing_sources {
                        if s1 != s2 {
                            if s1.is_pred(s2, &mut context) {
                                sources_to_add.remove(s2);
                            }
                        }
                    }
                }
                let mut sinks_to_add : HashSet<_> = missing_sinks.iter().cloned().collect();
                for s1 in &missing_sinks {
                    for s2 in &missing_sinks {
                        if s1 != s2 {
                            if s1.is_pred(s2, &mut context) {
                                sinks_to_add.remove(s1);
                            }
                        }
                    }
                }
                let mut stuff_to_add = sources_to_add.iter().chain(sinks_to_add.iter());
                if let Some(expr) = stuff_to_add.next() {
                    //println!("adding expression {}",expr.convert(&mapping_label_text));
                    expressions.insert(expr.clone());
                } else {
                    break;
                }

            }


            //println!("checking if we got something new");
            if context == before {
                break;
            }
        }

        //println!("printing diagram");
        self.nofixpoint_print_diagram(&expressions,&context);
        

    }

    fn nofixpoint_print_diagram(&self, expressions : &HashSet<Expr<Label>>, context : &Context<Label>) {
        let (diagram,mapping_node_to_id) = nofixpoint_context_to_diagram(expressions,context);
        let ids = mapping_node_to_id.values().cloned().collect_vec();
        let (equiv,diagram) = compute_direct_diagram(&ids, &diagram);
        let mapping_label_text : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();
        let mapping_id_to_node : HashMap<_,_> = mapping_node_to_id.iter().map(|(e,i)|(*i,e.clone())).collect();

        println!("\n\n");

        for (_,v) in equiv {
            if v.len() != 1 {
                //println!("equivalence {}",v.iter().join(" "));
                println!("{}",v.iter().map(|id|&mapping_id_to_node[id]).join("      "));

            }
        }
        for label in self.labels() {
            let expr = Expr::Base(label.clone(), false);
            let id = mapping_node_to_id[&expr];
            println!("{} = ({})",mapping_label_text[&label],id);
        }

        for (a,b) in &diagram {
            //let id1 = &mapping_id_to_node[&a];
            //let id2 = &mapping_id_to_node[&b];
            //println!("{} -> {}",id1.convert(&mapping_label_text),id2.convert(&mapping_label_text));
            println!("({}) -> ({})",a,b);
        }
        println!("\n\n");

    }

    pub fn fixpoint_loop(&self, eh: &mut EventHandler) -> Result<(Self,Vec<(Label,Label)>,Vec<(Label,Label)>), &'static str> {
        self.nofixpoint();
        unimplemented!();
    }


}


fn nofixpoint_context_to_diagram(expressions : &HashSet<Expr<Label>>, context : &Context<Label>) -> (Vec<(Label,Label)>, HashMap<Expr<Label>,Label>) {

    let node_to_id : HashMap<_,_> = expressions.iter().sorted().enumerate().map(|(i,e)|(e.clone(),i as Label)).collect();
    let mut diagram = vec![];

    for e1 in expressions {
        for e2 in expressions {
            if *context.get(&(e1.clone(),e2.clone())).unwrap_or(&false) {
                diagram.push((node_to_id[e1],node_to_id[e2]));
            }
        }
    }

    let diagram = diagram_to_indirect(&node_to_id.values().cloned().collect_vec(), &diagram);

    (diagram,node_to_id)
}

fn nofixpoint_missing_sources_and_sinks(expressions : &HashSet<Expr<Label>>, context : &Context<Label>, mapping_label_text : &HashMap<Label,String>) -> (HashSet<Expr<Label>>,HashSet<Expr<Label>>) {
    let mut missing_sources = HashSet::new();
    let mut missing_sinks = HashSet::new();

    let (diagram,mapping_node_to_id) = nofixpoint_context_to_diagram(expressions, context);
    let ids = mapping_node_to_id.values().cloned().collect_vec();
    let succ = diagram_indirect_to_reachability_adj(&ids, &diagram);
    let reverse_diagram = diagram.iter().cloned().map(|(a,b)|(b,a)).collect_vec();
    let pred = diagram_indirect_to_reachability_adj(&ids, &reverse_diagram);
    //let mapping_id_to_node : HashMap<_,_> = mapping_node_to_id.iter().map(|(node,id)|(*id,node.clone())).collect();

    for e1 in expressions {
        for e2 in expressions {
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



fn nofixpoint_fix_context(expressions : &HashSet<Expr<Label>>, context : &mut Context<Label>){

    for e1 in expressions {
        for e2 in expressions {
            let _ = e1.is_pred(e2, context);
        }
    }

} 
