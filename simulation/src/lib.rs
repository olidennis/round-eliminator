#![feature(partition_point)]
mod auto;
mod autolb;
mod autoub;
mod bignum;
mod constraint;
mod line;
mod maxclique;
mod problem;
mod simpleapi;
mod extremalsets;

pub use crate::auto::AutomaticSimplifications;
pub use crate::autolb::AutoLb;
pub use crate::autoub::AutoUb;
pub use crate::problem::DiagramType;
pub use crate::problem::Problem;
pub use crate::simpleapi::request_json;
pub use crate::bignum::BigBigNum;
pub use crate::problem::GenericProblem;
