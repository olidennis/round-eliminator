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
    /// Direct-target workers. Zero chooses up to four available cores.
    /// When enabled, RE² has one additional independent worker.
    pub threads: usize,
    pub re2: bool,
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
            threads: 0,
            re2: true,
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
    /// Remove annotated states/contexts that have no locally legal extension.
    Prune,
    /// Expose the entire current node context on each of its incidences.
    NodeContext,
    /// An oriented maximal matching of a subgraph, obtainable via line-graph MIS.
    Matching(Subgraph),
    /// Proper greedy coloring, including the colors of all neighbors.
    GreedyColoring(Subgraph),
    /// Process original node configurations in order, finding MIS in the
    /// remaining undominated vertices of each class.
    PriorityMis {
        graph: Subgraph,
        order: Vec<Vec<Label>>,
    },
    /// MIS in the square of the selected subgraph: centers are at distance
    /// at least three and every vertex is within distance two of a center.
    RulingSet(Subgraph),
    /// Remove these edge pairs by universally checked two-endpoint repairs,
    /// scheduled using a strong edge coloring.
    RepairPairs(Vec<[Label; 2]>),
    /// Search-only placeholder: synthesize an MIS edge predicate jointly with
    /// its mapping. Published certificates contain the concrete Mis step.
    FindMis(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MappingRow {
    pub input: Vec<String>,
    pub output: Vec<Label>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Re2Target {
    /// Exact, unsimplified speedups, including their set-label dictionaries.
    /// Replay verifies the two local decoding obligations independently.
    pub first: Problem,
    pub second: Problem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub added: Vec<[Label; 2]>,
    pub recipe: Vec<Step>,
    /// Ordered occurrences in every (canonically enumerated) annotated star.
    pub mapping: Vec<MappingRow>,
    /// Absent in legacy/direct certificates; outputs otherwise belong to second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Box<Re2Target>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub candidates: usize,
    pub mapping_attempts: usize,
    pub bounded_attempts: usize,
    pub elapsed_ms: u64,
    pub re2_mapping_attempts: usize,
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
