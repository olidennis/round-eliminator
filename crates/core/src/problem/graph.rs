use serde::Serialize;
use ts_rs::TS;

/// Possible degrees on each side, independently of which outputs are allowed.
/// Resolved defaults are stored here, so changing constraints cannot change the class.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
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

    pub fn active(&self) -> &[u32] {
        &self.active
    }
    pub fn passive(&self) -> &[u32] {
        &self.passive
    }
}
