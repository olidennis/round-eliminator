use crate::line::Line;

use either::Either;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use log::trace;
use crate::bignum::BigBigNum;



/// This struct represents a set of constraints.
/// It is represented by a set of lines.
/// Delta is the degree. Bits is the number of bits required to describe a group of labels.
/// Mask indicates which bits of the group should be considered.
/// Permutations indicates whether all permutations of each line are also included,
/// where true indicates that they are, false indicates that the lines have been minimized by removing permutations,
/// and none indicates that the constraints are arbitrary.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Hash, Ord, PartialOrd)]
pub struct Constraint<BigNum : crate::bignum::BigNum> {
    pub lines: Vec<Line<BigNum>>,
    pub delta: usize,
    pub bits: usize,
    pub mask: BigNum,
    pub permutations: Option<bool>,
}

impl<BigNum : crate::bignum::BigNum> Constraint<BigNum> {
    /// Creates an empty set of constraints, where each line has `delta` groups of `bits` bits.
    pub fn new(delta: usize, bits: usize) -> Self {
        Constraint {
            lines: vec![],
            delta,
            bits,
            mask: BigNum::create_mask(bits),
            permutations: None,
        }
    }

    /// Make the constraints harder, by keeping only labels satisfying the bitmask `keepmask`.
    pub fn harden(&self, keepmask: BigNum) -> Constraint<BigNum> {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        for line in self.lines.iter() {
            if let Some(newline) = line.harden(keepmask.clone()) {
                newlines.push(newline);
            }
        }
        Self {
            lines: newlines,
            delta,
            bits,
            mask: self.mask.clone() & keepmask,
            permutations: None,
        }
    }

    /// Creates a new set of constraints, where for each line the label `from` is replaced by the label `to`.
    pub fn replace(&self, from: usize, to: usize) -> Constraint<BigNum> {
        self.replace_with_group(from, BigNum::one() << to)
    }

    pub fn replace_with_group(&self, from: usize, to: BigNum) -> Constraint<BigNum> {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        for line in self.lines.iter() {
            let newline = line.replace_with_group(from, to.clone());
            newlines.push(newline);
        }
        Self {
            lines: newlines,
            delta,
            bits,
            mask: self.mask.clone().remove(&(BigNum::one() << from)),
            permutations: None,
        }
    }

    /// Add a line to the constraints.
    /// If some old line is included in the current one, remove the old one.
    /// If some old line includes the current one, do nothing.
    /// Does not change self.permutations.
    pub fn add_reduce(&mut self, newline: Line<BigNum>) {
        let l1 = self.lines.len();
        self.lines.retain(|oldline| !newline.includes(oldline));
        let l2 = self.lines.len();
        if l1 != l2 || self.lines.iter().all(|oldline| !oldline.includes(&newline)) {
            self.add(newline);
        }
    }

    pub fn add_reduce_bulk(&mut self, newlines: impl Iterator<Item=Line<BigNum>>) -> Vec<Line<BigNum>> {
        //let mut newlines : Vec<_> = newlines.enumerate().map(|(i,line)|(line.inner.count_ones(),line,i)).collect();
        //newlines.sort_unstable_by_key(|x|x.0);
        //for (i,(_,line,_)) in newlines.into_iter().rev().enumerate() {
        /*for line in newlines {
            self.add_reduce(line);
        }*/
        let newlines : Vec<_> = newlines.collect();

        if self.delta * self.bits <= 64*8 || newlines.len() < 128 {
            let mut removed = vec![];
            for newline in newlines {
                let l1 = self.lines.len();
                self.lines.retain(|oldline|{
                    let keep = !newline.includes(oldline);
                    if !keep {
                        removed.push(oldline.clone());
                    }
                    keep
                });
                let l2 = self.lines.len();
                if l1 != l2 || self.lines.iter().all(|oldline| !oldline.includes(&newline)) {
                    self.add(newline);
                } else {
                    removed.push(newline);
                }
            }
            return removed;
        }

        trace!("extracting bits");

        let old = std::mem::replace(&mut self.lines, vec![]);
        let mut sets = vec![];
        for (line,is_new) in old.into_iter().map(|x|(x,false)).chain(newlines.into_iter().map(|x|(x,true))) {
            let set : Vec<usize> = line.inner.one_bits().collect();
            sets.push((set,line,is_new));
        }


        if sets.is_empty() {
            return vec![];
        }

        trace!("counting");
        let k = self.delta * self.bits;
        let mut counts = vec![0;k];
        for (set,_,_) in &sets {
            for &x in set {
                counts[x] += 1;
            }
        }

        trace!("permuting");

        let mut pairs : Vec<_> = counts.into_iter().enumerate().map(|(a,b)|(b,a)).collect();
        pairs.sort();
        let map : HashMap<usize,usize> = pairs.iter().enumerate().map(|(newpos,&(_count,oldpos))|(oldpos,newpos)).collect();

        let mut new_to_old = HashMap::new();

        let mut sorted = vec![];
        for (set,line,is_new) in sets {
            let mut set : Vec<usize> = set.into_iter().map(|x|map[&x]).collect();
            set.sort();
            new_to_old.insert(set.clone(),line);
            sorted.push(crate::extremalsets::Elem{set,is_new,keep:true});
        }


        crate::extremalsets::find_maximal(&mut sorted);

        let mut removed = vec![];
        for crate::extremalsets::Elem{set,keep,..} in sorted {
            if keep {
                self.add(new_to_old[&set].clone());
            } else {
                removed.push(new_to_old[&set].clone());
            }
        }
        
        removed
    }

    /// Add a line to the constraints, no check is performed.
    pub fn add(&mut self, newline: Line<BigNum>) {
        self.lines.push(newline);
    }

    /// Add all possible permutations of the current lines.
    pub fn add_permutations(&mut self) {
        if self.permutations == Some(true) {
            return;
        }
        let old = std::mem::replace(&mut self.lines, vec![]);
        let mut perms : Vec<_> = old.into_iter().map(|oldline|oldline.permutations()).collect();
        let newlines = perms.iter_mut().flat_map(|perm|perm.iter());
        self.add_reduce_bulk(newlines);
        self.permutations = Some(true);
    }

    /// Minimize lines by removing permutations, that is, discard lines such that there are no two lines where one is a permutation of the other.
    pub fn remove_permutations(&mut self) {
        if self.permutations == Some(false) {
            return;
        }
        let old = std::mem::replace(&mut self.lines, vec![]);
        'outer: for oldline in old {
            for newline in oldline.permutations().iter() {
                if self.satisfies(&newline) {
                    continue 'outer;
                }
            }
            self.add_reduce(oldline.sorted());
        }
        self.lines.sort();
        self.permutations = Some(false);
    }

    /// Returns true if `v` is included in at least one line of the constraints.
    pub fn satisfies(&self, v: &Line<BigNum>) -> bool {
        self.lines.iter().any(|line| line.includes(v))
    }

    /// Rename the labels, that is, for each line, each possible group value gets a single bit in the new line.
    /// `mapping` indicates how to map groups to labels.
    /// For example, if 011010 should be mapped to 1000, then mapping should map `011010` to 3 (the position of the unique bit with value 1).
    pub fn renamed(&self, mapping: &HashMap<BigNum, usize>) -> Constraint<BigNum> {
        let newbits = mapping.len();
        let mut new = Constraint::new(self.delta, newbits);
        //let newlines = self.lines.iter().map(|line|line.renamed(mapping));
        //new.add_reduce_bulk(newlines);
        for line in self.lines.iter() {
            let newline = line.renamed(mapping);
            new.add(newline);
        }
        new
    }

    /// Create constraints starting from their text representation.
    pub fn from_text(text: &str, mapping: &HashMap<String, usize>) -> Result<Constraint<BigNum>, String> {
        let vec = Self::string_to_vec(text)?;
        Self::from_vec(&vec, mapping)
    }

    /// Given a string that represents a set of constraint, the string is parsed and split in a vector representation,
    /// Each resulting vector represents a vector representation of a line, where
    /// each of its resulting vectors represents a single group of the line.
    /// Each group is represented by a vector of strings.
    pub fn string_to_vec(text: &str) -> Result<Vec<Vec<Vec<String>>>,String> {
        text.lines().map(|line| crate::line::string_to_vec(line)).collect()
    }

    /// Creates a set of constraints starting from its vector representation.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn from_vec(
        v: &Vec<Vec<Vec<String>>>,
        mapping: &HashMap<String, usize>,
    ) -> Result<Constraint<BigNum>, String> {
        if v.is_empty() {
            return Err("Constraints can not be empty!".into());
        }
        let first = Line::<BigNum>::from_vec(&v[0], mapping);
        let delta = first.delta;
        let bits = first.bits;

        if delta*bits > BigNum::maxbits() {
            return Err(format!("The currently configured limit for delta*labels is {}, but in order to represent this problem a limit of {}*{} is required.",BigNum::maxbits(),bits,delta));
        }

        let mut c = Constraint::new(delta, bits);

        let mut toadd = vec![];
        for line in v {
            let line = Line::from_vec(line, mapping);
            if line.delta != delta || line.bits != bits {
                return Err("Constraints (of the same side) have different degrees!".into());
            }
            toadd.push(line);
        }
        c.add_reduce_bulk(toadd.into_iter());
        Ok(c)
    }

    /// Creates a vector representation of the constraints.
    /// Each resulting vecot represents a line
    /// Each of its vectors represents a single group of the line.
    /// Each group is represented by a vector of strings.
    /// `mapping` needs to provide a map from string labels to group positions.
    /// For example, if 001 010 001 111 represents the line A B C ABC,
    /// then `mapping` must map `A to 0`, `B to 1`, and `C to 2`
    pub fn to_vec(&self, mapping: &HashMap<usize, String>) -> Vec<Vec<Vec<String>>> {
        self.lines.iter().map(|line| line.to_vec(mapping)).collect()
    }

    /// Returns the unique groups appearing among the lines of the constraints.
    pub fn sets(&self) -> impl Iterator<Item = BigNum> {
        self.lines
            .iter()
            .flat_map(|line| line.groups())
            .unique()
            .sorted()
    }

    /// Return a mask that indicates which labels are actually used
    pub fn real_mask(&self) -> BigNum {
        self.sets().fold(BigNum::zero(), |a, b| a | b)
    }

    /// Performs the existential step on the current constraints.
    /// `mapping` maps old sets to the new labels.
    pub fn new_constraint_exist(&self, mapping: &HashMap<BigNum, usize>) -> Constraint<BigNum> {
        let newbits = mapping.len();
        let mut new = Constraint::new(self.delta, newbits);
        let newlines = self.lines.iter().map(|line|line.anymap(mapping)).filter(|newline|!newline.contains_empty_group());
        new.add_reduce_bulk(newlines);
        new
    }

    /// Performs the universal step on the current constraints.
    /// `pred` contains, for each label, all its (direct and indirect) predecessors
    pub fn new_constraint_forall(&self, pred : &[BigNum], diagram : &[(usize,usize)] ) -> Constraint<BigNum> {
        let delta = self.delta;
        let bits = self.bits;

        let bad = Line::forall_single(delta, bits, self.mask.clone()).filter(
            |line|!self.satisfies(&line)
        );

        let mut successors = vec![vec![];bits];
        let hs : HashSet<(usize,usize)> = diagram.iter().cloned().collect();
        for &(a,b) in diagram {
            if !hs.contains(&(b,a)) {
                successors[a].push(b);
            }
        }

        trace!("reducing bad configurations");
        let bads : HashSet<_> = bad.collect();
        trace!("total bad: {}",bads.len());
        trace!("computed bads");
        let mut maxbads = vec![];
        'outer: for (_,b) in bads.iter().enumerate() {
            for (i,p) in b.inner.one_bits().enumerate() {
                let p = p - (i * bits);
                for &s in &successors[p] {
                    if bads.contains(&b.with_group(i,BigNum::one() << s)) {
                        continue 'outer;
                    }
                }
            }
            maxbads.push(b);
        }

        trace!("starting to compute solution");

        let mut v = vec![];
        let mut nodup = HashSet::new();

        let init = Line::from_groups(delta, bits, std::iter::repeat(self.mask.clone()).take(delta));
        v.push(init);


        let sz = maxbads.len();
        for (i,r) in maxbads.into_iter().enumerate() {


            let sz2 = v.len();
            /*if i%100 == 0*/ { trace!("Enumerating bad configurations: {} / {} (good candidates: {})",i,sz,sz2); }

            let mut new = vec![];
            let mut toadd = vec![];

            //trace!("generating new lines");
            for line in v {
                if !line.includes(&r) {
                    new.push(line);
                } else {
                    nodup.remove(&line.sorted());
                    for x in Self::without_bad(line, r.clone(), pred).filter(|x|!x.contains_empty_group()).filter(|x|nodup.insert(x.sorted()) ) {
                        toadd.push(x);
                    }
                }
            }

            //trace!("reducing new lines ({} {} {})",new.len(),toadd.len(),new.len()+toadd.len());

            let mut c = Constraint::new(delta,bits);
            c.lines = new;
            let removed = c.add_reduce_bulk(toadd.iter().cloned());
            for line in removed {
                nodup.remove(&line.sorted());
            }

            let new = c.lines;
            //trace!("obtained {} lines",new.len());

            v = new;
        }

        let mut result = Constraint::new(delta, bits);
        result.permutations = None;

        for x in v {
            result.add(x);
        }

        result
    }


    fn without_bad(line : Line<BigNum>, bad : Line<BigNum>, pred : &[BigNum]) -> impl Iterator<Item=Line<BigNum>> + '_ {
        let one = BigNum::one();
        let bits = line.bits;
        bad.inner.one_bits().map(move |x|{
            let label = x % bits;
            let pos = x / bits;
            let mask = ((one.clone() << label) | pred[label].clone()) << (pos * bits); 
            Line{ inner : line.inner.clone().remove(&mask), ..line }
        })
    }

    /// Returns an iterator over all possible choices over the constraint that contains the label x at least once
    pub fn choices_iter_containing(&self,x : usize) -> impl Iterator<Item = Line<BigNum>> + '_ {
        Line::forall_single(self.delta-1, self.bits, self.mask.clone())
                    .map(move |line|line.add_column(x))
                    .filter(move |line| self.satisfies(line))
    }

    /// Returns an iterator over all possible choices over the constraints.
    pub fn choices_iter(&self) -> impl Iterator<Item = Line<BigNum>> + '_ {
        // If the current constraints are the left side of the result of a speedup, things can be made fast
        // otherwise, just do forall and check for sat
        let is_easy = self.lines.iter().all(|line| line.is_action());
        if is_easy {
            Either::Left(self.lines.iter().cloned())
        } else {
            Either::Right(
                Line::forall_single(self.delta, self.bits, self.mask.clone())
                    .filter(move |line| self.satisfies(line)),
            )
        }
    }

    /// Add the label to each time from is allowed
    pub fn imply(&self, from : usize, to : usize) -> Constraint<BigNum> {
        let mut newlines = vec![];
        let delta = self.delta;
        let bits = self.bits;
        let mask = self.mask.clone();
        for line in self.lines.iter() {
            let newline = line.imply(from, to);
            newlines.push(newline);
        }
        Self {
            lines: newlines,
            delta,
            bits,
            mask,
            permutations: None,
        }
    }

    pub fn intoo<T:crate::bignum::BigNum>(&self) -> Constraint<T> {
            let lines = self.lines.iter().map(|x|x.intoo()).collect();
            let delta = self.delta;
            let bits = self.bits;
            let mask = self.mask.intoo();
            let permutations = self.permutations;
            Constraint::<T>{ lines, delta, bits, mask, permutations }
    }

    pub fn permute_normalize(&self, renaming : &Vec<usize>) -> Constraint<BigBigNum> {
        use crate::bignum::BigNum;
        let newbits = renaming.len();
        let newlines = self.choices_iter().map(|line|{
            let newgroups = line.groups().map(|group|{
                let pos = group.one_bits().next().unwrap();
                let newpos = renaming[pos];
                let newgroup = BigBigNum::one() << newpos;
                newgroup
            });
            let newline = Line::from_groups(line.delta,newbits,newgroups.sorted());
            newline
        }).unique().sorted().collect();
        Constraint::<BigBigNum>{
            lines : newlines,
            delta : self.delta,
            bits : newbits,
            mask : BigBigNum::create_mask(newbits),
            permutations : None
        }
    }
}

