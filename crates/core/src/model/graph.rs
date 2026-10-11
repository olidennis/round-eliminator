use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Constraint, DegreeGroup};

/// When considering a problem, we want to explicitly remember which graph class we are interested about.
/// In more detail, we store which active and passive degrees are allowed.
/// For example, we could have a problem that allows only active configurations of degree 5,
/// but we want to know whether it is solvable on graphs that also contain nodes of degree 4.
/// So we don't want to just infer the consider degrees from the allowed configurations.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
pub struct GraphClass {
    active: Vec<u32>,
    passive: Vec<u32>,
}

impl GraphClass {
    pub fn new(mut active: Vec<u32>, mut passive: Vec<u32>) -> Self {
        active.sort_unstable();
        active.dedup();
        passive.sort_unstable();
        passive.dedup();
        Self { active, passive }
    }

    /// Infer degrees from constraints
    pub(crate) fn from_constraints<L: Copy + Ord>(
        active: &Constraint<L>,
        passive: &Constraint<L>,
    ) -> Self {
        Self::new(
            active.degrees().iter().map(DegreeGroup::degree).collect(),
            passive.degrees().iter().map(DegreeGroup::degree).collect(),
        )
    }

    pub fn active(&self) -> &[u32] {
        &self.active
    }
    pub fn passive(&self) -> &[u32] {
        &self.passive
    }
}
