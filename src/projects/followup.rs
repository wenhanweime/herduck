use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FollowupState {
    Queued,
    Sent,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectFollowup {
    pub id: String,
    pub project_key: String,
    pub session_key: String,
    pub pane_id: String,
    pub title: String,
    pub state: FollowupState,
    pub message: String,
    pub created_at: i64,
}
