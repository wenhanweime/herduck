use serde::{Deserialize, Serialize};

pub use crate::projects::cover::{TopicCover, TopicCoverPatch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TopicCoverGetParams {
    /// canonical_key of an entry in project.snapshot.topics.
    pub topic_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TopicCoverUpdateParams {
    pub topic_key: String,
    pub patch: TopicCoverPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSessionRenameParams {
    pub session_key: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSnapshotParams {
    pub projects_schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSessionAssignParams {
    pub session_key: String,
    pub project_key: String,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSessionUnlockParams {
    pub session_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSessionsPageParams {
    pub projects_schema_version: u32,
    pub project_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<crate::projects::domain::SessionCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

pub use crate::projects::domain::{AdapterScanStatus, ProjectSessionsPage, ProjectsSnapshot};
