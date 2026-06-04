//! Agent execution plans (the "todo list" an agent maintains during a turn).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanPriority {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEntryStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanPriority,
    pub status: PlanEntryStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
}

impl Plan {
    /// A plan is complete only when it has entries and all of them are done.
    pub fn is_complete(&self) -> bool {
        !self.entries.is_empty()
            && self
                .entries
                .iter()
                .all(|entry| entry.status == PlanEntryStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_plan_is_not_complete() {
        assert!(!Plan::default().is_complete());
    }

    #[test]
    fn plan_is_complete_when_all_entries_done() {
        let plan = Plan {
            entries: vec![
                PlanEntry {
                    content: "a".into(),
                    priority: PlanPriority::High,
                    status: PlanEntryStatus::Completed,
                },
                PlanEntry {
                    content: "b".into(),
                    priority: PlanPriority::Low,
                    status: PlanEntryStatus::Completed,
                },
            ],
        };
        assert!(plan.is_complete());
    }
}
