use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{constraint::Constraint, group::Label};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    pub active: Constraint,
    pub passive: Constraint,
    pub mapping_label_text: Vec<(Label, String)>,
    pub mapping_label_oldlabels: Option<Vec<(Label, Vec<Label>)>>,
    pub mapping_oldlabel_text: Option<Vec<(Label, String)>>,
    pub trivial_sets: Option<Vec<Vec<Label>>>,
    pub coloring_sets: Option<Vec<Vec<Label>>>,
    pub diagram_indirect: Option<Vec<(Label, Label)>>,
    pub diagram_indirect_old: Option<Vec<(Label, Label)>>,
    pub diagram_direct: Option<(Vec<(Label, Vec<Label>)>, Vec<(Label, Label)>)>,
}

impl Problem {
    pub fn from_string_active_passive<S: AsRef<str>>(
        active: S,
        passive: S,
    ) -> Result<Self, &'static str> {
        let mut mapping_label_text = HashMap::new();

        let active = Constraint::parse(active, &mut mapping_label_text)?;
        let passive = Constraint::parse(passive, &mut mapping_label_text)?;

        let mapping_label_text = mapping_label_text
            .into_iter()
            .map(|(a, b)| (b, a))
            .collect();

        let p = Problem {
            active,
            passive,
            mapping_label_text,
            mapping_label_oldlabels: None,
            mapping_oldlabel_text: None,
            trivial_sets: None,
            coloring_sets: None,
            diagram_indirect: None,
            diagram_direct: None,
            diagram_indirect_old: None,
        };
        Ok(p)
    }

    pub fn from_string<S: AsRef<str>>(s: S) -> Result<Self, &'static str> {
        let s = s.as_ref();
        let mut lines = s.lines();

        let active = lines.by_ref().take_while(|l| !l.is_empty()).join("\n");
        let passive = lines.take_while(|l| !l.is_empty()).join("\n");

        Self::from_string_active_passive(active, passive)
    }

    pub fn labels(&self) -> Vec<Label> {
        let mut labels: Vec<_> = self.mapping_label_text.iter().map(|(l, _)| *l).collect();
        labels.sort();
        labels
    }

    pub fn diagram_indirect_to_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect
            .as_ref()
            .expect("diagram required, but still not computed")
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_indirect_old_to_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect_old
            .as_ref()
            .expect("old diagram required")
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self
            .mapping_oldlabel_text
            .as_ref()
            .unwrap()
            .iter()
            .map(|(x, _)| *x)
        {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_indirect_to_inverse_reachability_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in self
            .diagram_indirect
            .as_ref()
            .expect("diagram required, but still not computed")
        {
            h.entry(b).or_default().insert(a);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }
}

impl Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mapping = self.mapping_label_text.iter().cloned().collect();
        for line in &self.active.lines {
            write!(f, "{}\n", line.to_string(&mapping))?;
        }
        write!(f, "\n")?;
        for line in &self.passive.lines {
            write!(f, "{}\n", line.to_string(&mapping))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::{algorithms::event::EventHandler, problem::Problem};

    #[test]
    #[should_panic]
    fn parsing_err() {
        let _ = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD**").unwrap();
    }

    #[test]
    fn parsing() {
        let p = Problem::from_string("AB^5 BC^100 CD^3\nABCD^108\n\nAB CD").unwrap();
        assert_eq!(format!("{}", p), "ABCD^108\n\nAB CD\n");

        let p = Problem::from_string("A AB*\nC CD*\n\nAB CD").unwrap();
        assert_eq!(format!("{}", p), "A AB*\nC CD*\n\nAB CD\n");
    }

    #[test]
    fn serialize() {
        let mut p = Problem::from_string("M U*\nP*\n\nM UP*\nU*").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B B\nC D D\n\nAB AB\nCD CD").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B B\nC D D\n\nAB CD").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_coloring_solvability(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("A B AB C\n\nAB AB\nC C").unwrap();
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_coloring_solvability(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);

        let mut p = Problem::from_string("M U*\nP*\n\nM UP*\nU*")
            .unwrap()
            .speedup(&mut eh);
        let mut eh = EventHandler::null();
        p.compute_triviality(&mut eh);
        p.compute_diagram(&mut eh);
        let serialized = serde_json::to_string(&p).unwrap();
        println!("{}", serialized);
    }

    /*    use std::collections::{HashMap, HashSet};

    #[test]
    fn testproblem() {
        let mut eh = EventHandler::with(|(s,a,b)|{print!("                                     \r{} {} {}\r",s,a,b);});
        /*let mut eh = EventHandler::null();
        let mut p = Problem::from_string(format!(r"M M M M M
        P U U U U
        1 1 1 1 1
        2 2 2 2 2
        3 3 3 3 3
        L L L L X
        (12) (12) (12) (12) U
        (13) (13) (13) (13) U
        (23) (23) (23) (23) U
        (123) (123) (123) U U

        MX PU123(12)(13)(23)(123)X PU123(12)(13)(23)(123)X
        LMPU123(12)(13)(23)(123)X PU123(12)(13)(23)(123)X X
        LX U23(23)X UX
        LX 2UX 3UX
        UX U123(12)(13)(23)(123)X UX
        1UX 23(23)UX UX
        2UX 13(13)UX UX
        3UX 12(12)UX UX
        1UX 2UX 3UX")).unwrap();
        p.compute_diagram(&mut eh);

        let mut p = p.speedup(&mut eh);
        p.compute_set_inclusion_diagram();
        p.rename_by_generators();

        let mut p = p.speedup(&mut eh);
        p.compute_set_inclusion_diagram();
        p.rename_by_generators();
        p.sort_active_by_strength();

        println!("{}",p);

        let serialized = serde_json::to_string(&p).unwrap();
        println!("\n\n{}\n\n",serialized);*/
        let s = std::fs::read_to_string("../serialized.txt").unwrap();
        let mut p: Problem = serde_json::from_str(&s).unwrap();
        println!("{}",p);


        let htl : HashMap<_,_> = p.mapping_label_text.iter().map(|(a,b)|(b.clone(),a.clone())).collect();
        p = p.relax_merge(htl["(<<2>>)"],htl["(<<M,U>,<2>>)"]);
        p = p.relax_merge(htl["(<<P>,<M,U>>)"],htl["(<<M,U>,<2>>)"]);
        p = p.relax_merge(htl["(<<M>,<P>>)"],htl["(<<X>>)"]);

        p = p.relax_merge(htl["(<<23>,<L,13>>)"],htl["(<<23>>)"]);
        p = p.relax_merge(htl["(<<123>,<L,13>>)"],htl["(<<123>>)"]);
        p = p.relax_merge(htl["(<<123>,<M,23>,<L,13>>)"],htl["(<<123>,<M,23>>)"]);

        p = p.relax_merge(htl["(<<P>,<L,12>>)"],htl["(<<12>,<L>>)"]);
        p = p.relax_merge(htl["(<<P>,<M,23>>)"],htl["(<<23>,<M,3>>)"]);

        p = p.relax_merge(htl["(<<23>,<M,3>>)"],htl["(<<23>>)"]);


        p.discard_useless_stuff(false, &mut eh);
        p.sort_active_by_strength();

        p.rename(&[
            (htl["(<<M>>)"],"M".into()),
            (htl["(<<M,U>>)"],"L".into()),
            (htl["(<<X>>)"],"X".into()),
            (htl["(<<M,U>,<2>>)"],"2".into()),
            (htl["(<<U>>)"],"U".into()),
            (htl["(<<23>>)"],"23".into()),
            (htl["(<<3>>)"],"3".into()),
            (htl["(<<12>,<L>>)"],"12".into()),
            (htl["(<<1>>)"],"1".into()),
            (htl["(<<123>>)"],"123".into()),
            (htl["(<<13>>)"],"13".into()),
            (htl["(<<P>>)"],"P".into()),
        ]).unwrap();

        println!("\n\n\n{}\n\n\n",p);
    }

    fn testproblem2() {
        //let s = std::fs::read_to_string("../test.txt").unwrap();
        //let p = Problem::from_string(s).unwrap();
        //println!("{}",p);
        //return;

        let mut eh = EventHandler::with(|(s,a,b)|{print!("                                     \r{} {} {}\r",s,a,b);});
        //let mut eh = EventHandler::null();
        let eh = &mut eh;

        let delta = 4;
        let nomerge = delta-2;

        let mut p = Problem::from_string(format!("M^{}\nP U^{}\n\nM UP^{}\nU^{}",delta,delta-1,delta-1,delta)).unwrap();
        p.compute_diagram(eh);

        let mut step = 0;
        let mut last_color = 0;

        for _ in 0.. {
            let serialized = serde_json::to_string(&p).unwrap();
            println!("\n\n{}\n\n",serialized);
            println!("\n\n{}\n\n",p);


            let htl : HashMap<_,_> = p.mapping_label_text.iter().map(|(a,b)|(b.clone(),a.clone())).collect();
            //p.diagram_indirect = None;
            //p.compute_diagram(eh);
            let label_p = htl["P"];
            for (_,group) in p.diagram_direct.as_ref().unwrap().0.clone().iter() {
                if group.contains(&label_p) {
                    for &l in group {
                        if l != label_p {
                            p = p.relax_merge(l,label_p);
                        }
                    }
                }
            }
            if p.diagram_indirect.is_none() {
                p.compute_diagram(eh);
            }

            p.compute_triviality(eh);
            if p.trivial_sets.as_ref().unwrap().len() > 0 {
                println!("got a trivial problem");
                return;
            }

            p = p.speedup(eh);
            p.compute_partial_diagram(eh);
            p.rename_by_generators().unwrap();

            //println!("\n\nIntermediate problem:\n{}\n\n",p);

            if step % nomerge != 0 && false {
                let hlt : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();
                let htl : HashMap<_,_> = p.mapping_label_text.iter().map(|(a,b)|(b.clone(),a.clone())).collect();
                let label_p = htl["(<P>)"];
                let successors_of_p : Vec<_> = p.diagram_direct.as_ref().unwrap().1.iter().filter(|&&(a,b)|a==label_p&&b!=label_p).map(|(_,b)|*b).collect();
                if successors_of_p.len() > 1 {
                    println!("\n\nP has {} successors\n\n",successors_of_p.len());
                    return;
                }
                if successors_of_p.len() == 1 {
                    let succp = successors_of_p[0];
                    //let predecessors_of_succp : Vec<_> = p.diagram_direct.as_ref().unwrap().1.iter().filter(|&&(a,b)|b==succp&&a!=succp).map(|(a,_)|*a).collect();
                    /*if predecessors_of_succp.len() != 2 {
                        println!("\n\nsuccP has {} predecessors\n\n",predecessors_of_succp.len());
                        return;
                    }
                    let otherp = predecessors_of_succp.into_iter().filter(|&x|x != label_p).next().unwrap();
                    */
                    /*for otherp in predecessors_of_succp.into_iter().filter(|&x|x != label_p){
                        println!("merging {} to {}",hlt[&otherp],hlt[&succp]);
                        p = p.relax_merge(otherp,succp);
                    }*/

                    let succ = p.diagram_indirect_to_reachability_adj();
                    let mut toremove = succ[&htl["(<M>)"]].clone();
                    for c in (1..=last_color).step_by(nomerge) {
                        toremove = toremove.intersection(&succ[&htl[&format!("(<{}>)",c)]]).cloned().collect();
                    }

                    for otherp in toremove.into_iter().filter(|&x|x != succp && succ[&x].contains(&succp)) {
                        //println!("merging {} to {}",hlt[&otherp],hlt[&succp]);
                        p = p.relax_merge(otherp,succp);
                    }

                    p.compute_partial_diagram(eh);
                    println!("\n\nIntermediate problem after simplifications:\n{}\n\n",p);
                } else {
                    //println!("\n\nnot doing intermediate relaxations\n\n");
                }
            } else {
                //println!("skipping intermediate relaxations");
            }




            p = p.speedup(eh);
            p.compute_partial_diagram(eh);
            p.rename_by_generators().unwrap();

            let succ = p.diagram_indirect_to_reachability_adj();


            let mut labelname = 1;
            for (_,text) in p.mapping_label_text.iter_mut() {
                match text.as_ref() {
                    "(<<M>>)" => {  *text = "M".into() },
                    "(<<P>>)" => {  *text = "P".into() },
                    "(<<U>>)" => {  *text = "U".into() },
                    "(<<M>,<P>>)" => {  *text = format!("X",); }
                    _ => {
                        if text[3..text.len()-3].parse::<usize>().is_err() {
                            *text = format!("(L{})",labelname);
                            labelname += 1;
                        } else {
                            *text = text[3..text.len()-3].into();
                        }
                    }
                }
            }

            //println!("AFTER RENAMING\n{}\n\n",p);


            let mut colors = vec![];
            for line in &p.active.lines {
                if line.parts.len() == 1 {
                    for (l,s) in p.mapping_label_text.iter_mut() {
                        if *l == line.parts[0].group[0] && s.len() > 1 {
                            last_color += 1;
                            *s = format!("{}",last_color);
                            break;
                        }
                    }
                    colors.push(line.parts[0].group[0]);
                }
            }

            let hlt : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();

            println!("\n\ngot colors: ");
            for c in &colors {
                print!(" {} ",hlt[c]);
            }
            println!("\n\n");

            println!("\n\nProblem BEFORE simplifications\n{}\n\n",p);


            let htl : HashMap<_,_> = p.mapping_label_text.iter().map(|(a,b)|(b.clone(),a.clone())).collect();

            let should_merge = step % nomerge != 0;
            if should_merge {
                let from = last_color;
                let to = step / nomerge * nomerge + 1;
                println!("\n\nmerging color {} to color {}\n\n",from,to);
                p = p.relax_merge(htl[&format!("{}",from)], htl[&format!("{}",to)]);
            }

            let mut successors_of_colors : HashSet<Label> = HashSet::new();
            for &c in &colors {
                let s = succ[&c].iter().filter(|&&x|x != c).filter(|l|hlt[l] != "X" && hlt[l] != "P");
                successors_of_colors.extend(s);
            }

            for label in successors_of_colors {
                let target = if succ[&label].contains(&htl["U"]) {
                    htl["U"]
                }  else {
                    htl["X"]
                };
                //println!("must merge wildcard {} to {}",hlt[&label],hlt[&target]);
                p = p.relax_merge(label, target);
            }

            p.compute_partial_diagram(eh);
            let label_p = htl["P"];
            let label_x = htl["X"];
            for (_,group) in p.diagram_direct.as_ref().unwrap().0.clone().iter() {
                if group.contains(&label_p) {
                    for &l in group {
                        if l != label_p {
                            p = p.relax_merge(l,label_p);
                        }
                    }
                }
                if group.contains(&label_x) {
                    for &l in group {
                        if l != label_p {
                            p = p.relax_merge(l,label_x);
                        }
                    }
                }
            }

            println!("\n\nProblem after some simplifications\n{}\n\n",p);

            p.discard_useless_stuff(true, eh);

            let hlt : HashMap<_,_> = p.mapping_label_text.iter().cloned().collect();
            //println!("\n\n{}\n\n",p);
            let succ = p.diagram_indirect_to_reachability_adj();
            for l1 in succ.keys() {
                for l2 in succ.keys() {
                    if l1 != l2  {
                        let mut sl1 = succ[l1].clone();
                        let mut sl2 = succ[l2].clone();
                        sl1.remove(l1);
                        sl2.remove(l2);
                        if sl1 == sl2 && sl1.len() >= 3 {
                            //println!("merging similar labels {} to {}",hlt[l1],hlt[l2]);
                            p = p.relax_merge(*l1, *l2);
                        }

                    }
                }
            }

            /*
            p.discard_useless_stuff(true, eh);

            //println!("\n\nJust before checking for path:\n{}\n\n",p);


            let dirsucc = p.diagram_direct_to_succ_adj();
            let dirpred = p.diagram_direct_to_pred_adj();
            let mut path : Vec<usize> = dirsucc[&label_p].iter().filter(|x|{
                //println!("succ of P: {}, {} {}",hlt[x],dirsucc[x].len(),dirpred[x].len());
                dirsucc[x].len() == 1 && dirpred[x].len() == 1
            }).cloned().collect();
            while !path.is_empty() {
                let last = path.last().unwrap();
                let next = dirsucc[last].iter().cloned().next().unwrap();
                if dirsucc[&next].len() == 1 && dirpred[&next].len() == 1 {
                    path.push(next);
                } else {
                    break;
                }
            }
            //println!("path length: {}",path.len());
            if path.len() >= 2 {
                println!("merging in the path between P and the color");
                for &x in &path {
                    p = p.relax_merge(x, path[0]);
                }
            }*/

            if p.diagram_indirect.is_none() {
                p.compute_partial_diagram(eh);
            }
            p.sort_active_by_strength();
            println!("\n\nProblem after more simplifications\n{}\n\n",p);


            p.discard_useless_stuff(true, eh);
            p.sort_active_by_strength();

            step += 1;
        }




    }*/
}

impl Problem {
    pub fn diagram_direct_to_succ_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in &self
            .diagram_direct
            .as_ref()
            .expect("diagram required, but still not computed")
            .1
        {
            h.entry(a).or_default().insert(b);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }

    pub fn diagram_direct_to_pred_adj(&self) -> HashMap<Label, HashSet<Label>> {
        let mut h: HashMap<Label, HashSet<Label>> = HashMap::new();
        for &(a, b) in &self
            .diagram_direct
            .as_ref()
            .expect("diagram required, but still not computed")
            .1
        {
            h.entry(b).or_default().insert(a);
        }
        for label in self.labels() {
            h.entry(label).or_default();
        }
        h
    }
}
