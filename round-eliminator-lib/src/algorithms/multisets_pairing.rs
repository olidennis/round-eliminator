use std::ops::Deref;

use streaming_iterator::StreamingIterator;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Comb {
    max: Vec<usize>,
    state: Vec<usize>,
    first: bool,
}

impl Deref for Comb {
    type Target = Vec<usize>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Comb {
    fn new(n: usize, max: Vec<usize>) -> Self {
        let mut state = vec![0; max.len()];
        let mut res = n;
        let mut i = 0;
        while res > 0 {
            let cur = std::cmp::min(max[i], res);
            state[i] = cur;
            res -= cur;
            i += 1;
        }
        Comb {
            max,
            state,
            first: true,
        }
    }

    fn transform(&mut self, n: usize, max: impl Iterator<Item = usize>) {
        let mut i = 0;
        for x in max {
            self.max[i] = x;
            i += 1;
        }
        assert!(i == self.max.len());
        let mut res = n;
        let mut i = 0;
        while res > 0 {
            let cur = std::cmp::min(self.max[i], res);
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
        } else {
            let v = &mut self.state;
            let m = &mut self.max;
            let mut i = 0;
            loop {
                if i == v.len() - 1 {
                    return None;
                }
                if v[i] > 0 {
                    v[i + 1] += 1;
                    v[i] -= 1;
                    if v[i + 1] <= m[i + 1] {
                        break;
                    }
                }
                i += 1;
            }
            let mut res = v[0..=i].iter().sum();
            let mut j = 0;
            while res > 0 {
                let cur = std::cmp::min(m[j], res);
                v[j] = cur;
                res -= cur;
                j += 1;
            }
            for k in j..=i {
                v[k] = 0;
            }
            Some(&self.state)
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Pairings {
    state: Vec<Comb>,
    first: bool,
    v1: Vec<usize>,
    end: bool,
}

impl Pairings {
    pub fn new(v1: Vec<usize>, mut v2: Vec<usize>) -> Self {
        let mut s = vec![];
        for &x in &v1 {
            let mut c = Comb::new(x, v2.clone());
            c.next();
            for i in 0..v2.len() {
                v2[i] -= c.state[i];
            }
            s.push(c);
        }

        Self {
            v1,
            state: s,
            first: true,
            end: false,
        }
    }
}

impl StreamingIterator for Pairings {
    type Item = Vec<Comb>;

    fn advance(&mut self) {
        self.end = true;
        if self.first {
            self.first = false;
            self.end = false;
        } else {
            for i in (0..self.state.len()).rev() {
                if self.state[i].next() != None {
                    for j in i + 1..self.state.len() {
                        let split = self.state.split_at_mut(j);
                        let p = &split.0[j - 1];
                        let p2 = &mut split.1[0];
                        let pmax = &p.max;
                        let ps = &p.state;
                        let n = self.v1[j];
                        p2.transform(n, pmax.iter().zip(ps.iter()).map(|(m, x)| m - x));
                        //let v : Vec<_> = pmax.iter().zip(ps.iter()).map(|(m,x)|m-x).collect();
                        //self.state[j] = Comb::new(n,v);
                        p2.next();
                    }
                    self.end = false;
                    break;
                }
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        if self.end {
            None
        } else {
            Some(&self.state)
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use streaming_iterator::StreamingIterator;

    use crate::algorithms::multisets_pairing::Pairings;

    use super::Comb;

    fn to_matrix(m: &Vec<Comb>) -> Vec<Vec<usize>> {
        let mut r = vec![];

        for v in m {
            r.push(v.state.clone());
        }

        r
    }

    #[test]
    fn all_pairings() {
        let v1 = vec![1, 2];
        let v2 = vec![2, 1];

        //two possibilities

        // bc bc ad
        //    cc d
        // a   0 1
        // bb  2 0

        // ac bc bd
        //    cc d
        // a   1 0
        // bb  1 1

        let p = Pairings::new(v1, v2);
        let all: Vec<_> = p.map(|x| to_matrix(x)).owned().sorted().collect();
        assert_eq!(all, [[[0, 1], [2, 0]], [[1, 0], [1, 1]]]);
    }
}
