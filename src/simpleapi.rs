#![allow(dead_code)]


use crate::problem::Problem;
use crate::problem::ResultProblem;
use crate::autolb::AutoLb;
use crate::autoub::AutoUb;
use crate::auto::AutomaticSimplifications;
use serde::{Deserialize, Serialize};

pub type Simpl = (usize,usize);
pub type RProblem = (Problem,ResultProblem);
pub type RSimplifications = Vec<(Simpl,(String,String))>;
pub type RLowerBoundStep = Vec<(Problem,crate::autolb::ResultStep,ResultProblem)>;
pub type RUpperBoundStep = Vec<(Problem,crate::autoub::ResultStep,ResultProblem)>;

pub fn new_problem(left : &str, right : &str) -> RProblem {
    let p = Problem::from_text(left, right);
    let r = p.as_result();
    (p,r)
}

pub fn speedup(p : &Problem) -> RProblem {
    let np = p.speedup();
    let nr = np.as_result();
    (np,nr)
}

pub fn possible_simplifications(p : &Problem) -> RSimplifications {
    let r = p.as_result();
    let pdiag = p.diagram.iter().cloned();
    let rdiag = r.diagram.iter().cloned();
    pdiag.zip(rdiag).collect()
}

pub fn simplify(p : &Problem, (a,b) : Simpl) -> RProblem  {
    let np = p.replace(a,b);
    let nr = np.as_result();
    (np,nr)
}

pub fn autolb(p : &Problem, maxiter : usize, maxlabels : usize) -> impl Iterator<Item=RLowerBoundStep>{
    let auto = AutomaticSimplifications::<AutoLb>::new(p.clone(), maxiter, maxlabels);
    auto.into_iter().map(|seq|{
        seq.as_result().steps.into_iter().map(|s|{
            let r = s.1.as_result();
            (s.1,s.0,r)
        }).collect()
    })
}

pub fn autoub(p : &Problem, maxiter : usize, maxlabels : usize) -> impl Iterator<Item=RUpperBoundStep>{
    let auto = AutomaticSimplifications::<AutoUb>::new(p.clone(), maxiter, maxlabels);
    auto.into_iter().map(|seq|{
        seq.as_result().steps.into_iter().map(|s|{
            let r = s.1.as_result();
            (s.1,s.0,r)
        }).collect()
    })
}


#[derive(Deserialize, Serialize)]
pub enum Request{
    NewProblem(String,String),
    Speedup(Problem),
    PossibleSimplifications(Problem),
    Simplify(Problem,Simpl),
    AutoLb(Problem,usize,usize),
    AutoUb(Problem,usize,usize)
}

#[derive(Deserialize, Serialize)]
pub enum Response{
    Done,
    P(RProblem),
    S(RSimplifications),
    L(RLowerBoundStep),
    U(RUpperBoundStep)
}

pub fn request<F>(req : Request, mut f : F) where F : FnMut(Response) {
    match req {
        Request::NewProblem(s1,s2) => {
            let r = new_problem(&s1, &s2);
            f(Response::P(r));
        }
        Request::Speedup(p) => {
            let r = speedup(&p);
            f(Response::P(r));
        }
        Request::PossibleSimplifications(p) => {
            let r = possible_simplifications(&p);
            f(Response::S(r));
        }
        Request::Simplify(p,s) => {
            let r = simplify(&p,s);
            f(Response::P(r));
        }
        Request::AutoLb(p,i,l) => {
            for r in autolb(&p,i,l) {
                f(Response::L(r));
            }
        }
        Request::AutoUb(p,i,l) => {
            for r in autoub(&p,i,l) {
                f(Response::U(r));
            }
        }
    }
    f(Response::Done);
}