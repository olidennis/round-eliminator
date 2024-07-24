use std::collections::{HashMap, HashSet};

use bnf::{Grammar, ParseTree, ParseTreeNode, Term};
use itertools::Itertools;

use crate::{group::Label, problem::Problem};

use super::event::EventHandler;

#[derive(Default,Clone,Debug)]
struct SubDiagram {
    edges : Vec<SDEdge>,
    noedges : Vec<SDEdge>,
    constraints : Vec<SDConstraint>,
    types : Vec<SDType>,
    merges : Vec<SDMerge>
}

impl SubDiagram {
    fn nodes(&self) -> Vec<String> {
        self.edges.iter().flat_map(|e|[&e.from,&e.to].into_iter())
        .chain(self.noedges.iter().flat_map(|e|[&e.from,&e.to].into_iter()))
        .chain(self.constraints.iter().map(|c|&c.node))
        .chain(self.types.iter().map(|c|&c.node))
        .unique()
        .cloned()
        .collect()
    }
    
    fn label_satisfies_constraints_of_node(&self, node: &str, indeg : usize, outdeg : usize, is_new : bool) -> bool {
        for c in &self.constraints {
            if !c.label_satisfies_constraint_of_node(node,indeg,outdeg) {
                return false;
            }
        }
        for t in &self.types {
            if !t.label_satisfies_constraint_of_node(node,is_new) {
                return false;
            }
        }
        true
    }
    
    fn labels_satisfy_edges(&self, map_nodes_labels : &HashMap<String,Label>, succ: &HashMap<Label, HashSet<Label>>) -> bool {
        for e in &self.edges {
            let l1 = map_nodes_labels[&e.from];
            let l2 = map_nodes_labels[&e.to];
            if !succ[&l1].contains(&l2) {
                return false;
            }
        }

        for e in &self.noedges {
            let l1 = map_nodes_labels[&e.from];
            let l2 = map_nodes_labels[&e.to];
            if succ[&l1].contains(&l2) {
                return false;
            }
        }

        true
    }
}

#[derive(Clone,Debug)]
enum SDLine {
    Edge(SDEdge),
    NoEdge(SDEdge),
    Constraint(SDConstraint),
    Merge(SDMerge),
    Type(SDType)
}

#[derive(Clone,Debug)]
struct SDEdge {
    from : String,
    to : String
}

#[derive(Clone,Debug)]
struct SDType {
    node : String,
    t : OldNew
}
impl SDType {
    fn label_satisfies_constraint_of_node(&self, node: &str, is_new: bool) -> bool {
        if self.node != node {
            return true;
        }

        match self.t {
            OldNew::OLD => { !is_new },
            OldNew::NEW => { is_new },
        }
    }
}

#[derive(Clone,Debug)]
enum InOut {
    IN,
    OUT
}

#[derive(Clone,Debug)]
enum OldNew {
    OLD,
    NEW
}

#[derive(Clone,Debug)]
enum Ineq {
    GE,
    GT,
    LE,
    LT,
    EQ
}

#[derive(Clone,Debug)]
struct SDConstraint {
    node : String,
    inout : InOut,
    ineq : Ineq,
    value : i32
}
impl SDConstraint {
    fn label_satisfies_constraint_of_node(&self, node: &str, indeg : usize, outdeg : usize) -> bool {
        if self.node != node {
            return true;
        }

        let deg = match self.inout {
            InOut::IN => indeg,
            InOut::OUT => outdeg,
        } as i32;

        match self.ineq {
            Ineq::GE => { return deg >= self.value },
            Ineq::GT => { return deg > self.value },
            Ineq::LE => { return deg <= self.value },
            Ineq::LT => { return deg < self.value },
            Ineq::EQ => { return deg == self.value }
        }
    }
}

#[derive(Clone,Debug)]
struct SDMerge {
    from : String,
    to : String
}

fn parse_string(w : &ParseTreeNode) -> String {
    match w {
        ParseTreeNode::Terminal(s) => s.to_string(),
        ParseTreeNode::Nonterminal(pt) => {
            pt.rhs_iter().map(|x|parse_string(x)).join("")
        }
    }
}

fn parse_number(n : &ParseTreeNode) -> Option<i32> {
    let s = parse_string(n);
    s.parse::<i32>().ok()
}

fn parse_merge(x : &ParseTree) -> SDMerge {
    let mut x = x.rhs_iter();
    x.next();
    x.next();
    let from = parse_string(x.next().unwrap());
    x.next();
    let to = parse_string(x.next().unwrap());
    SDMerge{from, to}
}

fn parse_edge(x : &ParseTree) -> SDEdge {
    let mut x = x.rhs_iter();
    x.next();
    x.next();
    let from = parse_string(x.next().unwrap());
    x.next();
    let to = parse_string(x.next().unwrap());
    SDEdge{from, to}
}

fn parse_constraint(x : &ParseTree) -> Option<SDConstraint> {
    let mut x = x.rhs_iter();
    x.next();
    x.next();
    let node = parse_string(x.next().unwrap());
    x.next();
    let inout = parse_string(x.next().unwrap());
    let inout = if inout == "in" { InOut::IN } else { InOut::OUT };
    x.next();
    let ineq = parse_string(x.next().unwrap());
    let ineq = match ineq.as_str() {
        ">=" => Ineq::GE,
        "<=" => Ineq::LE,
        ">" => Ineq::GT,
        "<" => Ineq::LT,
        "=" | "==" => Ineq::EQ,
        _ => panic!()
    };
    x.next();
    let value = parse_number(x.next().unwrap())?;
    Some(SDConstraint{node,inout,ineq,value})
}

fn parse_type(x : &ParseTree) -> SDType {
    let mut x = x.rhs_iter();
    x.next();
    x.next();
    let node = parse_string(x.next().unwrap());
    x.next();
    let oldnew = parse_string(x.next().unwrap());
    let oldnew = if oldnew == "old" { OldNew::OLD } else { OldNew::NEW };
    SDType{node,t : oldnew}
}

fn extract_nonterminal<'a>(x : &'a ParseTreeNode) -> &'a ParseTree<'a> {
    if let ParseTreeNode::Nonterminal(x) = x {
        return x;
    }
    panic!()
}

fn extract_terminal(x : &ParseTreeNode) -> String {
    if let ParseTreeNode::Terminal(x) = x {
        return x.to_string();
    }
    panic!()
}

fn term_value(term : &Term) -> String {
    match term {
        Term::Terminal(x) => x.to_string(),
        Term::Nonterminal(x) => x.to_string()
    }
}

fn parse_line(x : &ParseTree) -> Option<SDLine> {
    let rhs = extract_nonterminal(x.rhs_iter().next().unwrap());
    match term_value(rhs.lhs).as_str() {
        "edge" => Some(SDLine::Edge(parse_edge(rhs))),
        "noedge" => Some(SDLine::NoEdge(parse_edge(rhs))),
        "merge" => Some(SDLine::Merge(parse_merge(rhs))),
        "constraint" => Some(SDLine::Constraint(parse_constraint(rhs)?)),
        "type" => Some(SDLine::Type(parse_type(rhs))),
        _ => panic!()
    }
}

fn parse_input(x : &ParseTree) -> Option<Vec<SubDiagram>> {
    let mut option_rhs = Some(x.rhs_iter());
    let mut lines = vec![];

    while option_rhs.is_some() {
        let mut rhs = option_rhs.unwrap();
        rhs.next();
        let line = rhs.next();
        if let Some(line) = line {
            let line = parse_line(extract_nonterminal(line));
            if let Some(line) = line {
                lines.push(line);
            } else {
                return None;
            }
            rhs.next();
            option_rhs = Some(extract_nonterminal(rhs.next().unwrap()).rhs_iter());
        } else {
            option_rhs = None;
        }
    }

    let mut result = vec![];
    let mut sd = SubDiagram::default();

    if lines.is_empty() {
        return Some(vec![]);
    }

    for line in lines {
        match line {
            SDLine::Merge(_) => {},
            _ => {
                if !sd.merges.is_empty(){
                    result.push(sd);
                    sd = SubDiagram::default();
                } 
            }
        }

        match line {
            SDLine::Edge(x) => sd.edges.push(x),
            SDLine::NoEdge(x) => sd.noedges.push(x),
            SDLine::Constraint(x) => sd.constraints.push(x),
            SDLine::Merge(x) => sd.merges.push(x),
            SDLine::Type(x) => sd.types.push(x)
        }
    }

    if sd.merges.is_empty() {
        return None;
    }

    result.push(sd);

    Some(result)
}

fn parse_subdiagram(subdiagram : &str) -> Option<Vec<SubDiagram>> {
    let grammar = "
        <input> ::= '' | <opt-newline> <line> <opt-newline> <input> <opt-newline>
        <line> ::= <edge> | <noedge> | <constraint> | <merge> | <type>

        <edge> ::= 'e' <opt-whitespace> <word> <opt-whitespace> <word>
        <noedge> ::= 'x' <opt-whitespace> <word> <opt-whitespace> <word>

        <constraint> ::= 'c' <opt-whitespace> <word> <opt-whitespace> <in-out> <opt-whitespace> <ineq> <opt-whitespace> <number>
        <in-out> ::= 'in' | 'out'
        <ineq> ::= '>=' | '<=' | '>' | '<' | '=' | '=='

        <type> ::= 't' <opt-whitespace> <word> <opt-whitespace> <old-new>
        <old-new> ::= 'old' | 'new'

        <merge> ::= 'm' <opt-whitespace> <word> <opt-whitespace> <word>

        <opt-newline> ::= '' | '\n' <opt-newline>
        <opt-whitespace> ::= '' | ' ' <opt-whitespace>
        <word> ::= <letter> | <letter> <word>
        <number> ::= <digit> | <digit> <number>
        <letter> ::= 'A' | 'B' | 'C' | 'D' | 'E' | 'F'
                | 'G' | 'H' | 'I' | 'J' | 'K' | 'L'
                | 'M' | 'N' | 'O' | 'P' | 'Q' | 'R'
                | 'S' | 'T' | 'U' | 'V' | 'W' | 'X'
                | 'Y' | 'Z' | 'a' | 'b' | 'c' | 'd'
                | 'e' | 'f' | 'g' | 'h' | 'i' | 'j'
                | 'k' | 'l' | 'm' | 'n' | 'o' | 'p'
                | 'q' | 'r' | 's' | 't' | 'u' | 'v'
                | 'w' | 'x' | 'y' | 'z'
        <digit>  ::= '0' | '1' | '2' | '3' | '4' | '5'
                | '6' | '7' | '8' | '9'
    ";

    let grammar: Grammar = grammar.parse().unwrap();


    let parsed = grammar.parse_input(subdiagram).next()?;
    parse_input(&parsed)
}


impl Problem {

    pub fn find_subdiagram(&self, sd: &SubDiagram) -> Option<Vec<(String,Label)>> {
        let pred = self.diagram_direct_to_pred_adj();
        let succ = self.diagram_direct_to_succ_adj();
        let (_,new) = self.split_labels_original_new();
        let new : HashSet<_> = new.into_iter().collect();
        //let new : HashSet<_> = self.mapping_label_generators().into_iter().filter(|(_,g)|g.len() != 1).map(|(l,_)|l).collect();

        let nodes = sd.nodes();
        let mut candidates = vec![];

        for node in &nodes {
            let mut v = vec![];
            for l in self.labels() {
                if sd.label_satisfies_constraints_of_node(node,pred[&l].len(),succ[&l].len(), new.contains(&l) ) {
                    v.push(l);
                }
            }
            candidates.push(v);
        }

        for choice in candidates.iter().map(|v|v.iter().copied()).multi_cartesian_product() {
            if choice.iter().unique().count() != choice.len() {
                continue;
            }

            let h : HashMap<_,_> = nodes.iter().cloned().zip(choice.into_iter()).collect();

            if sd.labels_satisfy_edges(&h,&succ) {
                return Some(h.into_iter().collect());
            }
        }

        None
    }

    pub fn merge_subdiagram(&self, subdiagram : &str, eh : &mut EventHandler) -> Option<Problem> {
        
        let sds = parse_subdiagram(subdiagram)?;
        let mut p = self.repeat_merge_equivalent_labels(eh);
        let label_to_string : HashMap<_,_> = self.mapping_label_text.iter().cloned().collect();

        loop {
            let mut merged = false;
            for sd in &sds {
                loop {
                    p.compute_direct_diagram();
                    if let Some(v) = p.find_subdiagram(&sd) {
                        let h : HashMap<_,_> = v.into_iter().collect();
                        for merge in &sd.merges {
                            merged = true;
                            let l1 = h[&merge.from];
                            let l2 = h[&merge.to];
                            println!("merging from {} to {}", label_to_string[&l1], label_to_string[&l2]);
                            p = p.relax_merge(l1, l2);
                            p = p.repeat_merge_equivalent_labels(eh);
                        }
                    } else {
                        break;
                    }
                }
            }
            if !merged {
                break;
            }
        }

        Some(p)
    }
}
