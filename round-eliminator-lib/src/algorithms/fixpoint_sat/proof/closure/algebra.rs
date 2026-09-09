//! Hash-consed universal lattice terms and exact finite observation profiles.
//! No concrete-diagram equality is used by the symbolic normalizer.

use super::*;

pub(super) type Id = usize;
pub(super) type Row = Vec<Id>;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum JoinMeet {
    Join,
    Meet,
}

#[derive(Clone)]
pub(super) struct Node {
    pub op: Option<JoinMeet>,
    pub children: Row,
    pub default: u32,
}

#[derive(Default)]
pub(super) struct Observer {
    kinds: Vec<Option<JoinMeet>>,
    children: Vec<Row>,
    pub values: Vec<u128>,
    ids: HashMap<u128, Id>,
    operations: HashMap<(Id, Id, JoinMeet), Id>,
    atoms: Row,
}

impl Observer {
    fn intern(&mut self, value: u128) -> Id {
        if let Some(&id) = self.ids.get(&value) {
            return id;
        }
        let id = self.values.len();
        self.values.push(value);
        self.ids.insert(value, id);
        id
    }

    pub fn operation(&mut self, mut a: Id, mut b: Id, op: JoinMeet) -> Id {
        if a == b {
            return a;
        }
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if let Some(&id) = self.operations.get(&(a, b, op)) {
            return id;
        }
        let mut value = if op == JoinMeet::Meet {
            self.values[a] & self.values[b]
        } else {
            self.values[a] | self.values[b]
        };
        if op == JoinMeet::Join {
            // C(x meet y,t) = C(x,t) AND C(y,t).
            // For x join y, process t in subterm order: a target meet is AND;
            // a target join additionally permits decomposition of that target.
            // Target atoms have only C(x,t) OR C(y,t). These are precisely the
            // recursive C rules, specialized to a finite subterm-closed set.
            for t in 0..self.kinds.len() {
                let bit = match self.kinds[t] {
                    Some(JoinMeet::Meet) => self.children[t].iter().all(|&c| value & (1 << c) != 0),
                    Some(JoinMeet::Join) => {
                        value & (1 << t) != 0
                            || self.children[t].iter().any(|&c| value & (1 << c) != 0)
                    }
                    None => value & (1 << t) != 0,
                };
                if bit {
                    value |= 1 << t;
                } else {
                    value &= !(1 << t);
                }
            }
        }
        let id = self.intern(value);
        self.operations.insert((a, b, op), id);
        id
    }

    pub fn precedes(&self, a: Id, b: Id) -> bool {
        self.values[a] & !self.values[b] == 0
    }
}

pub(super) struct Algebra {
    pub terms: Vec<Node>,
    order: Vec<u32>,
    adjacent: Vec<u32>,
    intern: HashMap<(JoinMeet, Row), Id>,
    orders: HashMap<(Id, Id), bool>,
    compatibility: HashMap<(Id, Id), bool>,
    pub observer: Observer,
    evaluations: Vec<Option<Id>>,
}

impl Algebra {
    pub fn new(order: Vec<u32>, adjacent: Vec<u32>) -> Self {
        let terms = order
            .iter()
            .map(|&default| Node {
                op: None,
                children: Vec::new(),
                default,
            })
            .collect();
        Self {
            terms,
            order,
            adjacent,
            intern: HashMap::new(),
            orders: HashMap::new(),
            compatibility: HashMap::new(),
            observer: Observer::default(),
            evaluations: Vec::new(),
        }
    }

    pub fn over_budget(&self) -> bool {
        self.terms.len() >= 100_000
            || self.observer.values.len() >= 20_000
            || self.observer.operations.len() >= 1_000_000
    }

    pub fn precedes(&mut self, a: Id, b: Id) -> bool {
        if a == b {
            return true;
        }
        // A countermodel in the default lattice is only a NEGATIVE filter.
        if self.terms[a].default & self.terms[b].default != self.terms[b].default {
            return false;
        }
        if let Some(&yes) = self.orders.get(&(a, b)) {
            return yes;
        }
        let (x, y) = (self.terms[a].op, self.terms[b].op);
        let mut yes = false;
        if x.is_none() && y.is_none() {
            yes = self.order[a] & (1 << b) != 0;
        } else {
            if y == Some(JoinMeet::Join) {
                for i in 0..self.terms[b].children.len() {
                    let c = self.terms[b].children[i];
                    if self.precedes(a, c) {
                        yes = true;
                        break;
                    }
                }
            }
            if y == Some(JoinMeet::Meet) {
                yes = true;
                for i in 0..self.terms[b].children.len() {
                    let c = self.terms[b].children[i];
                    if !self.precedes(a, c) {
                        yes = false;
                        break;
                    }
                }
            }
            if !yes && x == Some(JoinMeet::Join) {
                yes = true;
                for i in 0..self.terms[a].children.len() {
                    let c = self.terms[a].children[i];
                    if !self.precedes(c, b) {
                        yes = false;
                        break;
                    }
                }
            }
            if !yes && x == Some(JoinMeet::Meet) {
                for i in 0..self.terms[a].children.len() {
                    let c = self.terms[a].children[i];
                    if self.precedes(c, b) {
                        yes = true;
                        break;
                    }
                }
            }
        }
        if self.orders.len() < 4_000_000 {
            self.orders.insert((a, b), yes);
        }
        yes
    }

    pub fn compatible(&mut self, mut a: Id, mut b: Id) -> bool {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if let Some(&yes) = self.compatibility.get(&(a, b)) {
            return yes;
        }
        let (x, y) = (self.terms[a].op, self.terms[b].op);
        let yes = if x.is_none() && y.is_none() {
            self.adjacent[a] & (1 << b) != 0
        } else if x == Some(JoinMeet::Meet) {
            let mut yes = true;
            for i in 0..self.terms[a].children.len() {
                let c = self.terms[a].children[i];
                if !self.compatible(c, b) {
                    yes = false;
                    break;
                }
            }
            yes
        } else if y == Some(JoinMeet::Meet) {
            let mut yes = true;
            for i in 0..self.terms[b].children.len() {
                let c = self.terms[b].children[i];
                if !self.compatible(a, c) {
                    yes = false;
                    break;
                }
            }
            yes
        } else {
            let mut yes = false;
            for i in 0..self.terms[a].children.len() {
                let c = self.terms[a].children[i];
                if self.compatible(c, b) {
                    yes = true;
                    break;
                }
            }
            if !yes {
                for i in 0..self.terms[b].children.len() {
                    let c = self.terms[b].children[i];
                    if self.compatible(a, c) {
                        yes = true;
                        break;
                    }
                }
            }
            yes
        };
        if self.compatibility.len() < 1_000_000 {
            self.compatibility.insert((a, b), yes);
        }
        yes
    }

    pub fn operation(&mut self, a: Id, b: Id, op: JoinMeet) -> Id {
        if self.precedes(a, b) {
            return if op == JoinMeet::Join { b } else { a };
        }
        if self.precedes(b, a) {
            return if op == JoinMeet::Join { a } else { b };
        }
        let mut row = if self.terms[a].op == Some(op) {
            self.terms[a].children.clone()
        } else {
            vec![a]
        };
        if self.terms[b].op == Some(op) {
            row.extend_from_slice(&self.terms[b].children);
        } else {
            row.push(b);
        }
        row.sort_unstable();
        row.dedup();
        let mut reduced = Vec::new();
        for c in row {
            let mut redundant = false;
            let mut i = 0;
            while i < reduced.len() {
                let r = reduced[i];
                if if op == JoinMeet::Join {
                    self.precedes(c, r)
                } else {
                    self.precedes(r, c)
                } {
                    redundant = true;
                    break;
                }
                if if op == JoinMeet::Join {
                    self.precedes(r, c)
                } else {
                    self.precedes(c, r)
                } {
                    reduced.remove(i);
                } else {
                    i += 1;
                }
            }
            if !redundant {
                reduced.push(c);
            }
        }
        if reduced.len() == 1 {
            return reduced[0];
        }
        let key = (op, reduced);
        if let Some(&id) = self.intern.get(&key) {
            return id;
        }
        let default = if op == JoinMeet::Join {
            self.terms[a].default & self.terms[b].default
        } else {
            self.terms[a].default | self.terms[b].default
        };
        let id = self.terms.len();
        self.terms.push(Node {
            op: Some(op),
            children: key.1.clone(),
            default,
        });
        self.intern.insert(key, id);
        id
    }

    pub fn subterms(&self, t: Id, selected: &mut BTreeSet<Id>) {
        if selected.insert(t) {
            for &c in &self.terms[t].children {
                self.subterms(c, selected);
            }
        }
    }

    pub fn set_observers(&mut self, selected: &[Id]) {
        assert!(selected.len() <= 128);
        let index: HashMap<_, _> = selected.iter().enumerate().map(|(i, &t)| (t, i)).collect();
        let mut observer = Observer::default();
        for &t in selected {
            observer.kinds.push(self.terms[t].op);
            observer
                .children
                .push(self.terms[t].children.iter().map(|c| index[c]).collect());
        }
        for a in 0..self.order.len() {
            let mut value = 0;
            for (i, &t) in selected.iter().enumerate() {
                if self.compatible(a, t) {
                    value |= 1 << i;
                }
            }
            let id = observer.intern(value);
            observer.atoms.push(id);
        }
        self.observer = observer;
        self.evaluations.clear();
    }

    pub fn observe(&mut self, t: Id) -> Id {
        if self.evaluations.len() < self.terms.len() {
            self.evaluations.resize(self.terms.len(), None);
        }
        if let Some(id) = self.evaluations[t] {
            return id;
        }
        let id = if let Some(op) = self.terms[t].op {
            let mut result = self.observe(self.terms[t].children[0]);
            for i in 1..self.terms[t].children.len() {
                let other = self.observe(self.terms[t].children[i]);
                result = self.observer.operation(result, other, op);
            }
            result
        } else {
            self.observer.atoms[t]
        };
        self.evaluations[t] = Some(id);
        id
    }
}
