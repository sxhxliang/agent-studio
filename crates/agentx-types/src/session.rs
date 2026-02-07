use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    #[default]
    Active,
    Idle,
    InProgress,
    Pending,
    Completed,
    Closed,
    Failed,
}
