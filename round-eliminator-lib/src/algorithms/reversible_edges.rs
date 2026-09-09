//! Certified O(log* n) reverse reductions for additions to the edge constraint.
//! Wire types are available everywhere; synthesis is native-only for now.
use crate::{group::Label, problem::Problem};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub seconds: u64,
    pub attempt_ms: u64,
    pub max_candidates: usize,
    pub max_configurations: usize,
    pub max_states: usize,
    pub max_variables: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            seconds: 60,
            attempt_ms: 1500,
            max_candidates: 128,
            max_configurations: 4096,
            max_states: 1024,
            max_variables: 200_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Subgraph {
    All,
    /// Unordered pairs of ORIGINAL labels; annotations never merge labels.
    Pairs(Vec<[Label; 2]>),
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Step {
    Mis(Subgraph),
    /// Any proper (Delta+1)-vertex coloring of the whole graph.
    Coloring,
    /// One round of communication revealing the opposite incidence's state.
    Exchange,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MappingRow {
    pub input: Vec<String>,
    pub output: Vec<Label>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub added: Vec<[Label; 2]>,
    pub recipe: Vec<Step>,
    /// Ordered occurrences in every (canonically enumerated) annotated star.
    pub mapping: Vec<MappingRow>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub candidates: usize,
    pub mapping_attempts: usize,
    pub bounded_attempts: usize,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub original: Problem,
    pub certificates: Vec<Certificate>,
    pub stats: Stats,
    pub complete: bool,
    pub message: String,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "all"))]
pub use native::{apply, search};
