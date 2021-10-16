
use itertools::Itertools;
use std::collections::HashMap;
use std::collections::HashSet;
use crate::Line;
use crate::Constraint;
use crate::Problem;
use indicatif::ProgressStyle;
use indicatif::ProgressBar;
use chrono::prelude::*;

struct Comb{
    max : Vec<usize>,
    state : Vec<usize>,
    first : bool
}

impl Comb {
    fn new(n : usize, max : Vec<usize>) -> Self {
        let mut state = vec![0;max.len()];
        let mut res = n;
        let mut i = 0;
        while res > 0 {
            let cur = std::cmp::min(max[i],res);
            state[i] = cur;
            res -= cur;
            i += 1;
        }
        Comb {
            max, state, first:true
        }
    }

    fn transform(&mut self, n : usize, max : impl Iterator<Item=usize>) {
        let mut i = 0;
        for x in max {
            self.max[i] = x;
            i += 1;
        }
        assert!(i == self.max.len());
        let mut res = n;
        let mut i = 0;
        while res > 0 {
            let cur = std::cmp::min(self.max[i],res);
            self.state[i] = cur;
            res -= cur;
            i += 1;
        }
        for j in i..self.state.len() {
            self.state[j] = 0;
        }
        self.first = true;
    }
    
    fn next(&mut self) -> Option<&Vec<usize>> {
        if self.first {
            self.first = false;
            Some(&self.state)
        }else {
            let v = &mut self.state;
            let m = &mut self.max;
            let mut i = 0;
            loop {
                if i == v.len()-1 {
                    return None;
                }
                if v[i] > 0 {
                    v[i+1] += 1;
                    v[i] -= 1;
                    if v[i+1] <= m[i+1] {
                        break;
                    }
                }
                i += 1;
            }
            let mut res = v[0..=i].iter().sum();
            let mut j = 0;
            while res > 0 {
                let cur = std::cmp::min(m[j],res);
                v[j] = cur;
                res -= cur;
                j += 1;
            }
            for k in j..=i {
                v[k] = 0;
            }
            return Some(&self.state);
        }
    }
}


struct Matches {
    state : Vec<Comb>,
    first : bool,
    v1 : Vec<usize>
}

impl Matches {
    fn new(v1 : Vec<usize>, mut v2 : Vec<usize>) -> Self {
        let mut s = vec![];
        for &x in &v1 {
            let mut c = Comb::new(x,v2.clone());
            c.next();
            for i in 0..v2.len() {
                v2[i] -= c.state[i];
            }
            s.push(c);
        }
        
        Self {
            v1, state : s, first : true
        }
    }
    
    fn next(&mut self) -> Option<&Vec<Comb>> {
        if self.first {
            self.first = false;
            Some(&self.state)
        }else {
            for i in (0..self.state.len()).rev() {
                if self.state[i].next() != None {
                    for j in i+1..self.state.len() {
                        let split = self.state.split_at_mut(j);
                        let p = &split.0[j-1];
                        let p2 = &mut split.1[0];
                        let pmax = &p.max;
                        let ps = &p.state;
                        let n = self.v1[j];
                        p2.transform(n,pmax.iter().zip(ps.iter()).map(|(m,x)|m-x));
                        //let v : Vec<_> = pmax.iter().zip(ps.iter()).map(|(m,x)|m-x).collect();
                        //self.state[j] = Comb::new(n,v);
                        p2.next();
                    }
                    return Some(&self.state);
                }
            }
            None
        }
    }
}

fn count_map<BigNum>(v : &[BigNum]) -> HashMap<BigNum,usize> where BigNum : crate::bignum::BigNum{
    let mut h = HashMap::new();
    for n in v {
        *h.entry(n.clone()).or_default() += 1;
    }
    h
}

#[inline(never)]
fn intersections<BigNum>(uni : BigNum, c1 : &[(BigNum,usize)], c2 : &[(BigNum,usize)], delta : usize , bits : usize) -> Vec<Line<BigNum>> where BigNum : crate::bignum::BigNum {
    let v1 : Vec<_> = c1.iter().map(|(_,c)|*c).collect();
    let v2 : Vec<_> = c2.iter().map(|(_,c)|*c).collect();

    let mut m = Matches::new(v1,v2);
    let mut r = vec![];

    let mut oldbad : Option<(usize,usize,usize,usize)> = None;

    'outer: while let Some(x) = m.next() {
        if let Some((i1,i2,j1,j2)) = oldbad {
            if x[i1].state[j1] != 0 && x[i2].state[j2] != 0 {
                continue 'outer;
            }
        }
        for i1 in 0..c1.len() {
            for j1 in 0..c2.len() {
                if x[i1].state[j1] != 0 {
                    for i2 in i1+1..c1.len() {
                        for j2 in 0..c2.len() {
                            if x[i2].state[j2] != 0 {
                                let u1 = c1[i1].0.clone() & c2[j1].0.clone();
                                let u2 = c1[i2].0.clone() & c2[j2].0.clone();
                                let u3 = c1[i1].0.clone() & c2[j2].0.clone();
                                let u4 = c1[i2].0.clone() & c2[j1].0.clone();

                                if (u4.is_superset(&u1) && u3.is_superset(&u2) && (u1 != u4 || u2 != u3)) || (u3.is_superset(&u1) && u4.is_superset(&u2) && (u1 != u3 || u2 != u4)) {
                                    oldbad = Some((i1,i2,j1,j2));
                                    continue 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut groups = Vec::with_capacity(delta);
        groups.push(uni.clone());
        for (i,(ga,_)) in c1.iter().enumerate() {
            for (j,(gb,_)) in c2.iter().enumerate() {
                for _ in 0..x[i].state[j] {
                    groups.push(ga.clone() & gb.clone());
                }
            }
        }
        if !groups.contains(&BigNum::zero()) {
            r.push(Line::from_groups(delta, bits, groups.into_iter()).sorted());
        }
    }
    r
}

#[inline(never)]
fn perm_includes<BigNum>(line : &Line<BigNum>, other : &Line<BigNum>) -> bool where BigNum : crate::bignum::BigNum {
    let g1 : Vec<_> = line.groups().collect();
    let g2 : Vec<_> = other.groups().collect();
    let d = g1.len();
    let mut g = contest_algorithms::graph::flow::FlowGraph::new(2*d+2,d*d);
    for i in 1..=d {
        g.add_edge(0, i, 1, 0, 0);
    }
    for i in d+1..=2*d {
        g.add_edge(i, 2*d+1, 1, 0, 0);
    }
    for i in 0..d {
        for j in 0..d {
            if g1[i].is_superset(&g2[j]) {
                g.add_edge(1+i, 1+d+j, 1, 0, 0);
            }
        }
    }

    g.dinic(0, 2*d+1).0 == d as i64
}

#[inline(never)]
fn add_reduce_maximal<BigNum>(lines : &mut Vec<Line<BigNum>>, newline : Line<BigNum>) where BigNum : crate::bignum::BigNum {
    let l1 = lines.len();
    lines.retain(|oldline| !perm_includes(&newline, oldline));
    let l2 = lines.len();
    if l1 != l2 || lines.iter().all(|oldline|!perm_includes(oldline,&newline)) {
        lines.push(newline);
    }
}


#[inline(never)]
fn find_good_unions<BigNum>(u1 : &[BigNum], u2 : &[BigNum]) -> HashMap<BigNum,Vec<(BigNum,BigNum)>> where BigNum : crate::bignum::BigNum {
    let mut unions = HashMap::new();
    for x in u1.iter() {
        for y in u2.iter() {
            if x.is_superset(y) || y.is_superset(x) {
                continue;
            }
            let uni = x.clone() | y.clone();
            let unis : &mut Vec<(BigNum,BigNum)> = unions.entry(uni).or_insert(vec![]);
            let len = unis.len();
            unis.retain(|(xc,yc)| !(xc.is_superset(x) && yc.is_superset(y)) );
            if unis.len() != len || unis.iter().all(|(xc,yc)| !(x.is_superset(xc) && y.is_superset(yc)) ) {
                unis.push((x.clone(),y.clone()));
            }
        }
    }
    unions
}



pub fn forall<BigNum>(nc : &Constraint<BigNum>, problem : &Problem<BigNum>) -> Constraint<BigNum> where BigNum : crate::bignum::BigNum {
    let mut nc = nc.clone();

    let maplt = problem.map_label_text();
    let set_to_string = |s:&BigNum|{
        let r = s.one_bits().map(|elem|&maplt[&elem]).join("");
        if r == "" {
            String::from("_")
        }else{
            r
        }
    };

    let make_right_closed = |g : BigNum|{g.clone()|problem.successors(g.one_bits().next().unwrap(),false)};

    for line in &mut nc.lines {
        *line = line.edited(|g|{make_right_closed(g)}).sorted();
    }

    let mut seen : HashSet<_> = nc.lines.iter().cloned().collect();

    let lines = std::mem::replace(&mut nc.lines, vec![]);
    for line in lines {
        seen.insert(line.clone());
        add_reduce_maximal(&mut nc.lines, line);
    }

    {
        println!("\n--- Constraints ---");
        for line in &nc.lines {
            let s1 = line.groups().map(|x|String::from("(")+&set_to_string(&x)+")").join(" ");
            println!("{}",s1);
        }
    }


    let mut pairs = HashSet::new();
    loop {
        let mut newc = nc.clone();
        let size = nc.lines.len();
        let lines = &nc.lines;
        
        let mut without_one = vec![];
        for line in &nc.lines {
            let mut h = HashMap::new();
            let g : Vec<_> = line.groups().collect();
            for i in 0..g.len() {
                if !h.contains_key(&g[i]){
                    let v : Vec<_> = [&g[0..i],&g[i+1..g.len()]].concat();
                    let v : Vec<_> = count_map(&v).into_iter().sorted().collect();
                    h.insert(g[i].clone(),v);
                }
            }
            without_one.push(h);
        }
        let mut line_groups = vec![];
        for line in lines {
            line_groups.push(line.groups().unique().collect::<Vec<_>>());
        }

        #[cfg(not(target_arch = "wasm32"))]
        let pb = ProgressBar::new((size*size) as u64);
        #[cfg(not(target_arch = "wasm32"))]
        pb.set_style(ProgressStyle::default_bar()
            .template("\n[elapsed: {elapsed_precise}] [{wide_bar:.green/red}] [eta: {eta_precise}]\n{msg}")
            /*.progress_chars("#>-")*/);
        

        for i in 0..lines.len() {
            #[cfg(not(target_arch = "wasm32"))]
            {
            pb.set_position((i*i) as u64);
            let est = pb.eta().as_secs();
            let dest = chrono::Duration::seconds(est as i64);
            let whenfinish = (Local::now() + dest).to_rfc2822();
            pb.set_message(format!("[i: {}/{}] [new lines: {}] [eta: {}]",i,size,newc.lines.len(),whenfinish));
            }

            let mut candidates2 = vec![];
            
            for j in 0..=i {
                
                let mut candidates = vec![];
                let pair = (lines[i].clone(),lines[j].clone());
                if pairs.contains(&pair) || pairs.contains(&(pair.1.clone(),pair.0.clone())) {
                    continue;
                }
                pairs.insert(pair);

                let u1 = &line_groups[i];
                let u2 = &line_groups[j];
                let unions = find_good_unions(u1,u2);

                for (uni,v) in unions {
                    for (x,y) in v {
                        let c1 = &without_one[i][&x];
                        let c2 = &without_one[j][&y];
                        let lines = intersections(uni.clone(),c1,c2,nc.delta, nc.bits);
                        for newline in lines {
                            if !seen.contains(&newline){
                                seen.insert(newline.clone());
                                add_reduce_maximal(&mut candidates, newline);
                            }
                        }
                    }
                }
                for newline in candidates {
                    add_reduce_maximal(&mut candidates2, newline);
                }
                
            }

            for newline in candidates2 {
                add_reduce_maximal(&mut newc.lines, newline);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        pb.finish_and_clear();

        if newc == nc { break; }
        println!("new iteration...");
        nc = newc;

        {
            println!("\n--- Constraints ---");
            for line in &nc.lines {
                let s1 = line.groups().map(|x|String::from("(")+&set_to_string(&x)+")").join(" ");
                println!("{}",s1);
            }
        }

    }

    nc
}
