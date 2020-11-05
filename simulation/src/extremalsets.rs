#![allow(non_snake_case)]

use log::trace;



#[derive(Ord,PartialOrd,Eq,PartialEq)]
pub struct Elem {
    pub set : Vec<usize>,
    pub is_new : bool,
    pub keep : bool
}

impl Elem {
    fn includes(&self, other : &Self) -> bool {
        if !self.is_new && !other.is_new {
            return false;
        }
        let v1 = &self.set;
        let v2 = &other.set;
        let mut i1 = v1.iter().cloned();
        let i2 = v2.iter().cloned();
        for x in i2 {
            loop {
                let y = i1.next();
                if let Some(y) = y {
                    if x == y {
                        break;
                    }
                } else {
                    return false;
                }
            }
        }
        true
    }
}

pub fn find_maximal(v : &mut Vec<Elem>) {

    v.sort();

    /*
    {
        use std::io::Write;
        let mut file = std::fs::File::create("dataset.bin").unwrap();
        for (i,Elem{set,..}) in v.iter().enumerate() {
            let mut towrite = vec![];
            let bytes: [u8; 8] = unsafe { std::mem::transmute(i+1) };
            for i in 0..4 {
                towrite.push(bytes[i]);
            }
            let len = set.len();
            let bytes: [u8; 8] = unsafe { std::mem::transmute(len) };
            for i in 0..4 {
                towrite.push(bytes[i]);
            }
            for x in set {
                let bytes: [u8; 8] = unsafe { std::mem::transmute(x+1) };
                for i in 0..4 {
                    towrite.push(bytes[i]);
                }
            }
            file.write_all(&towrite).unwrap();
        }
    }
    
    trace!("dumped (size = {})",v.len());*/


    mark_prefix(v);
    let index = build_index(v);


    let mut seeks = 0;

    for i in 0..v.len()-1 {
        let (first,others) = v[i..].split_first_mut().unwrap();
        mark_subsumed(&mut seeks, &index,first, others,i+1,0,0);
    }

    trace!("total seeks {}",seeks);

}


fn mark_subsumed(seeks : &mut usize, index : &[usize], S : &Elem, mut D : &mut [Elem], mut b : usize, mut j : usize, d : usize) {
    while !D.is_empty() {
        if S.set[j] < D[0].set[d] {
            j = next_item(&S.set,j,D[0].set[d]);
            if j >= S.set.len() {
                return;
            }
        }
        if S.set[j] == D[0].set[d] {
            *seeks += 1;
            let off = next_end_range(index, D, b, S.set[j], d);
            let mut skip = 0;
            if S.set.len() > d+1 {
                while skip < off && D[skip].set.len() == d+1 {
                    D[skip].keep = false;
                    skip += 1;
                }
            }
            if j+1 < S.set.len() && skip < off {
                stacker::maybe_grow(32 * 1024, 1024 * 1024, || {
                    mark_subsumed(seeks, index, S, &mut D[skip..off], b+skip, j+1, d+1);
                });
            }
            b += off;
            D = &mut D[off..];
        } else {
            *seeks += 1;
            let off = next_begin_range(index, D,b,S.set[j],d);
            b += off;
            D = &mut D[off..];
        }
    }
}

fn mark_prefix(v : &mut [Elem]) {
    let len = v.len();
    for i in 0..len-1 {
        if v[i+1].includes(&v[i]) {
            v[i].keep = false;
        }
    }
}

fn build_index(S : &[Elem]) -> Vec<usize> {
    if S.is_empty() {
        return vec![];
    }
    let mut v = vec![0;S.last().unwrap().set[0] +1];
    let mut x = S[0].set[0];
    v[x] = 0;

    for (i,e) in S.iter().enumerate() {
        let y = e.set[0];
        if y != x {
            for t in x+1..=y {
                v[t] = i;
            }
            x = y;
        }
    }
    v
}

fn next_item(S : &[usize],j : usize, x : usize) -> usize {
    j + S[j..].partition_point(|&y| y < x )
}

fn next_end_range(index : &[usize], D : &[Elem], b : usize, x : usize, d : usize) -> usize {
    if d == 0 {
        if x +1 >= index.len() {
            D.len()
        } else {
            index[x +1] - b
        }
    } else {
        D.partition_point(|e| e.set[d] <= x )
    }
}

fn next_begin_range(index : &[usize], D : &[Elem], b : usize, x : usize, d : usize) -> usize {
    if d == 0 {
        if x >= index.len() {
            D.len()
        } else {
            index[x] - b
        }
    } else {
        D.partition_point(|e| e.set[d] < x )
    }
}





        /*
        AMS-CARD: too slow

        //trace!("sorting by size");

        sorted.sort_by_key(|x|x.0.len());

        let mut o = vec![vec![];k];

        let mut currentsize = 0;

        let mut group : Vec<(Vec<usize>,bool)> = vec![];

        let includes = |v1 : &Vec<usize>, v2 : &Vec<usize>| -> bool {
            let mut i1 = v1.iter().cloned();
            let i2 = v2.iter().cloned();
            for x in i2 {
                loop {
                    let y = i1.next();
                    if let Some(y) = y {
                        if x == y {
                            break;
                        }
                    } else {
                        return false;
                    }
                }
            }
            true
        };

        
        trace!("finding maxima");

        let mut removed = vec![];

        for (set,is_new) in sorted {
            if set.len() != currentsize {
                currentsize = set.len();
                for (smaller,is_new) in group.into_iter() {
                    o[smaller[0]].push(Some((smaller,is_new)));
                }
                group = vec![];
            }
            for (j,&x) in set.iter().enumerate() {
                for t in 0..o[x].len() {
                    if let Some((smaller,smaller_is_new)) = &o[x][t] {
                        if smaller.len() > set.len() - j {
                            break;
                        }
                        if (is_new || *smaller_is_new) && includes(&set,smaller) {
                            removed.push(smaller.clone());
                            o[x][t] = None;
                        }
                    }
                }
            }
            group.push((set,is_new));
        }

        for (smaller,is_new) in group.into_iter() {
            o[smaller[0]].push(Some((smaller,is_new)));
        }

        trace!("retrieving result");

        /*
        for list in o {
            for set in list {
                let line = set.into_iter().fold(BigNum::zero(),|n,x|n | (BigNum::one() << invmap[&x]));
                self.add(Line::from(self.delta,self.bits,line));
            }
        }

        removed.into_iter().map(|set|set.into_iter().fold(BigNum::zero(),|n,x|n | (BigNum::one() << invmap[&x]))).map(|x|Line::from(self.delta,self.bits,x)).collect()
        */
        for list in o {
            for x in list {
                if let Some((set,_)) = x {
                    self.add(new_to_old[&set].clone());
                }
            }
        }
        removed.into_iter().map(|set|new_to_old[&set].clone()).collect()*/
