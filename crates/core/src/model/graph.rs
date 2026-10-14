use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Constraint, DegreeGroup};

/// LLM: The sorted, distinct active and passive degrees on which the problem must be solved.
/// LLM: These are stored independently of feasible output rows, so an impossible degree cannot
/// LLM: silently disappear during a transformation.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct GraphClass {
    active: Vec<u32>,
    passive: Vec<u32>,
}

impl GraphClass {
    /// LLM: Sort and deduplicate the allowed degree lists without inspecting feasibility. Used
    /// LLM: when resolving entry degrees and swapping sides during RE.
    pub fn new(mut active: Vec<u32>, mut passive: Vec<u32>) -> Self {
        // LLM: Canonicalize both degree lists independently; feasibility does not affect which
        // LLM: degrees are required.
        active.sort_unstable();
        active.dedup();
        passive.sort_unstable();
        passive.dedup();
        Self { active, passive }
    }

    /// LLM: Infer the two degree lists from existing constraint groups. Used by parsing
    /// LLM: defaults rather than to mutate an existing problem's graph class.
    pub(crate) fn from_constraints<L: Copy + Ord>(
        active: &Constraint<L>,
        passive: &Constraint<L>,
    ) -> Self {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Self::new(
            active.degrees().iter().map(DegreeGroup::degree).collect(),
            passive.degrees().iter().map(DegreeGroup::degree).collect(),
        )
    }

    /// LLM: Return the active-side constraint or degree list from GraphClass. Used by the
    /// LLM: problem model and its algorithms without exposing mutable storage.
    pub fn active(&self) -> &[u32] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.active
    }
    /// LLM: Return the passive-side constraint or degree list from GraphClass. Used by the
    /// LLM: problem model and its algorithms without exposing mutable storage.
    pub fn passive(&self) -> &[u32] {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        &self.passive
    }
}
