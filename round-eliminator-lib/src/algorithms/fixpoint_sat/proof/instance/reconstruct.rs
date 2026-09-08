//! Invert only the identities used by native proof replay: commutativity and
//! idempotency. This is a finite, memoized search over the supplied subterms,
//! not certificate synthesis. Every nontrivial split strictly decreases both
//! parent tuples' total term size, so the search graph is acyclic.

use super::*;

pub(super) fn synchronized(terms: &[Term]) -> bool {
    if terms.iter().all(|t| matches!(t, Term::Terminal(_))) {
        return true;
    }
    let mut children = [Vec::new(), Vec::new()];
    let mut joins = 0;
    for term in terms {
        let Term::Expr(a, b, op) = term else {
            return false;
        };
        joins += usize::from(*op == Operation::Union);
        children[0].push(a.as_ref().clone());
        children[1].push(b.as_ref().clone());
    }
    joins == 1 && children.iter().all(|side| synchronized(side))
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Subterm {
    Atom(Label),
    Expr { join: bool, children: [usize; 2] },
}

enum Recipe {
    Input(Vec<usize>),
    Combine {
        parents: [usize; 2],
        permutations: [Vec<usize>; 2],
        pivot: usize,
    },
}

struct Restore<'a> {
    terms: Vec<Subterm>,
    ids: HashMap<Subterm, usize>,
    memo: HashMap<Vec<usize>, Option<usize>>,
    recipes: Vec<Recipe>,
    checkpoint: &'a mut dyn FnMut(usize) -> bool,
    stopped: bool,
    visits: usize,
}

impl Restore<'_> {
    fn intern(&mut self, term: &Term) -> usize {
        let value = match term {
            Term::Terminal(l) => Subterm::Atom(*l),
            Term::Expr(a, b, op) => {
                let mut children = [self.intern(a), self.intern(b)];
                if children[0] == children[1] {
                    return children[0];
                }
                children.sort_unstable();
                Subterm::Expr {
                    join: *op == Operation::Union,
                    children,
                }
            }
        };
        if let Some(&id) = self.ids.get(&value) {
            return id;
        }
        let id = self.terms.len();
        self.terms.push(value);
        self.ids.insert(value, id);
        id
    }

    fn check(&mut self) -> bool {
        self.visits += 1;
        if self.visits % 256 == 1 && !(self.checkpoint)(self.memo.len()) {
            self.stopped = true;
        }
        // Bound auxiliary memory independently of the solver's later budget.
        if self.memo.len() >= 200_000 {
            self.stopped = true
        }
        !self.stopped
    }

    fn derive(&mut self, key: &[usize]) -> Option<usize> {
        if !self.check() {
            return None;
        }
        if let Some(result) = self.memo.get(key) {
            return *result;
        }
        for pivot in 0..key.len() {
            let choices: Vec<Vec<[usize; 2]>> = key
                .iter()
                .enumerate()
                .map(|(i, &id)| {
                    let mut choices = Vec::new();
                    if let Subterm::Expr {
                        join,
                        children: [a, b],
                    } = self.terms[id]
                    {
                        if join == (i == pivot) {
                            // Prefer exposing existing operations before inserting
                            // a redundant one. Both child orders remain available.
                            choices.push([a, b]);
                            choices.push([b, a]);
                        }
                    }
                    choices.push([id, id]);
                    choices
                })
                .collect();
            if let Some(id) = self.choose(key, pivot, &choices, &mut Vec::new()) {
                self.memo.insert(key.to_vec(), Some(id));
                return Some(id);
            }
            if self.stopped {
                return None;
            }
        }
        self.memo.insert(key.to_vec(), None);
        None
    }

    fn choose(
        &mut self,
        key: &[usize],
        pivot: usize,
        choices: &[Vec<[usize; 2]>],
        rows: &mut Vec<[usize; 2]>,
    ) -> Option<usize> {
        if !self.check() {
            return None;
        }
        if rows.len() < choices.len() {
            for &pair in &choices[rows.len()] {
                rows.push(pair);
                let found = self.choose(key, pivot, choices, rows);
                rows.pop();
                if found.is_some() || self.stopped {
                    return found;
                }
            }
            return None;
        }
        if rows.iter().zip(key).all(|(pair, &id)| *pair == [id, id]) {
            return None; // No progress: an entirely idempotent column.
        }
        let mut keys = [Vec::new(), Vec::new()];
        let mut permutations = [Vec::new(), Vec::new()];
        for side in 0..2 {
            // Sorting occurrences (not just distinct terms) preserves ports
            // when a tuple contains repeated labels/subexpressions.
            let mut ordered: Vec<_> = rows
                .iter()
                .enumerate()
                .map(|(i, row)| (row[side], i))
                .collect();
            ordered.sort_unstable();
            permutations[side] = vec![0; key.len()];
            for (position, (id, original)) in ordered.into_iter().enumerate() {
                keys[side].push(id);
                permutations[side][original] = position;
            }
        }
        if keys[0] > keys[1] {
            return None;
        } // Swap both whole parents instead.
        let left = self.derive(&keys[0])?;
        let right = self.derive(&keys[1])?;
        let id = self.recipes.len();
        self.recipes.push(Recipe::Combine {
            parents: [left, right],
            permutations,
            pivot,
        });
        Some(id)
    }

    fn expand(
        &mut self,
        root: usize,
        counts: &mut HashMap<usize, usize>,
    ) -> std::result::Result<Option<Vec<Term>>, String> {
        fn size(recipes: &[Recipe], root: usize, counts: &mut HashMap<usize, usize>) -> usize {
            if let Some(&n) = counts.get(&root) {
                return n;
            }
            let n = match &recipes[root] {
                Recipe::Input(ids) => ids.len(),
                Recipe::Combine {
                    parents,
                    permutations,
                    ..
                } => size(recipes, parents[0], counts)
                    .saturating_add(size(recipes, parents[1], counts))
                    .saturating_add(permutations[0].len()),
            };
            counts.insert(root, n);
            n
        }
        if size(&self.recipes, root, counts) > 1_000_000 {
            return Err(
                "Reconstructed expression expansion exceeds one million tree nodes; inconclusive"
                    .into(),
            );
        }
        if !self.check() {
            return Ok(None);
        }
        match &self.recipes[root] {
            Recipe::Input(ids) => Ok(Some(
                ids.iter()
                    .map(|&id| match self.terms[id] {
                        Subterm::Atom(label) => Term::Terminal(label),
                        _ => unreachable!(),
                    })
                    .collect(),
            )),
            Recipe::Combine {
                parents,
                permutations,
                pivot,
            } => {
                let (parents, permutations, pivot) = (*parents, permutations.clone(), *pivot);
                let Some(left) = self.expand(parents[0], counts)? else {
                    return Ok(None);
                };
                let Some(right) = self.expand(parents[1], counts)? else {
                    return Ok(None);
                };
                Ok(Some(
                    (0..left.len())
                        .map(|i| {
                            Term::Expr(
                                Box::new(left[permutations[0][i]].clone()),
                                Box::new(right[permutations[1][i]].clone()),
                                if i == pivot {
                                    Operation::Union
                                } else {
                                    Operation::Intersection
                                },
                            )
                        })
                        .collect(),
                ))
            }
        }
    }
}

pub(super) fn restore(
    inputs: &[Vec<Term>],
    terms: &[Term],
    checkpoint: &mut dyn FnMut(usize) -> bool,
) -> std::result::Result<Option<Vec<Term>>, String> {
    let mut state = Restore {
        terms: Vec::new(),
        ids: HashMap::new(),
        memo: HashMap::new(),
        recipes: Vec::new(),
        checkpoint,
        stopped: false,
        visits: 0,
    };
    for input in inputs {
        let mut key: Vec<_> = input.iter().map(|t| state.intern(t)).collect();
        key.sort_unstable();
        let id = state.recipes.len();
        state.recipes.push(Recipe::Input(key.clone()));
        state.memo.insert(key, Some(id));
    }
    let wanted: Vec<_> = terms.iter().map(|t| state.intern(t)).collect();
    let mut ordered: Vec<_> = wanted
        .iter()
        .copied()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    ordered.sort_unstable();
    let key: Vec<_> = ordered.iter().map(|&(id, _)| id).collect();
    let Some(root) = state.derive(&key) else {
        return if state.stopped {
            Ok(None)
        } else {
            Err("No synchronized active derivation can be reconstructed using commutativity and idempotency".into())
        };
    };
    let Some(expanded) = state.expand(root, &mut HashMap::new())? else {
        return Ok(None);
    };
    let mut result = terms.to_vec();
    for (value, (_, position)) in expanded.into_iter().zip(ordered) {
        result[position] = value
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests;
