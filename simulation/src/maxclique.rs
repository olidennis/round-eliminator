
use log::trace;


pub struct Graph {
    n : usize,
    adj : Vec<Vec<usize>>,
    m : Vec<Vec<bool>>
}

impl Graph{
    pub fn from_adj( adj : Vec<Vec<usize>>) -> Self {
        // assumes adj contains all bidirectional edges
        let n = adj.len();
        let mut m = vec![vec![false;n];n];
        for (a,v) in adj.iter().enumerate() {
            for &b in v.iter() {
                if a != b {
                    m[a][b] = true;
                    m[b][a] = true;
                }
            }
        }
        Self { n,adj,m }
    }

    pub fn max_clique(&self) -> usize {
        let n = self.n;
        let mut c = vec![0;n];
        let v = self.ordering();
        let mut max = 0;
        for i in (0..n).rev() {
            let vi = v[i];
            let si = v[i..].iter().cloned();
            let u : Vec<_> = si.filter(|&x|self.m[vi][x]).collect();
            let mut found = false;
            self.clique_rec(&u, 1, &c, &mut max, &mut found);
            c[vi] = max;
        }
        max
    }


    fn clique_rec(&self, mut u : &[usize] , size : usize, c : &[usize], max : &mut usize, found : &mut bool) {
        if u.is_empty() {
            if size > *max {
                *max = size;
                *found = true;
            }
        }
        while ! u.is_empty() {
            if size + u.len() <= *max {
                return;
            }
            let vi = u[0];
            if size + c[vi] <= *max {
                return;
            }
            u = &u[1..];
            let newu : Vec<_> = u.iter().cloned().filter(|&x|self.m[vi][x]).collect();
            self.clique_rec(&newu, size+1, c, max, found);
            if *found {
                return;
            }
        }
    }



    pub fn ordering(&self) -> Vec<usize> {
        let n = self.n;

        let mut order = vec![];
        let mut remaining = vec![true; n];
        let mut degrees : Vec<_> = self.adj.iter().map(|v|v.len()).collect();
        let mut ncols = 0;

        while remaining.iter().any(|&x|x) {
            ncols += 1;
            let mut active = remaining.clone();
            loop {
                if let Some(max) = degrees.iter().cloned().enumerate().filter(|&(i,_)|active[i]).max_by_key(|&(_,x)|x) {
                    let max = max.0;
                    for &x in &self.adj[max] {
                        active[x] = false;
                        degrees[x] -= 1;
                    }
                    order.push(max);
                    active[max] = false;
                    remaining[max] = false;
                } else {
                    break;
                }
            }
        }
        trace!("Upper bound on clique size: {}",ncols);
        assert!(order.len() == n);
        order
    }

}