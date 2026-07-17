use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Triage,
    Todo,
    InProgress,
    Blocked,
    Done,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IssueId(pub Uuid);

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("issue status transition is not allowed")]
    InvalidStatusTransition,
}
