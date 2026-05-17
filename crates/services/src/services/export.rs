//! Workspace export service.
//!
//! Produces the canonical sandbox-compatible payload (D-11) consumed by
//! `developer-sandbox`, the `vk-task` skill, and any future external tool.
//! The HTTP routes in `crates/server/src/routes/export.rs` are thin
//! wrappers around this service.

use std::sync::OnceLock;

use db::models::{
    task::Task,
    workspace::{Workspace, WorkspaceError},
};
use indexmap::IndexMap;
use regex::Regex;
use serde::Serialize;
use sqlx::SqlitePool;
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::services::remote_client::RemoteClient;

pub const SCHEMA_V1: &str = "vk-export/v1";

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ExportPayload {
    #[serde(rename = "$schema")]
    #[ts(rename = "$schema")]
    pub schema: String,
    pub workspace: ExportWorkspace,
    pub task: Option<ExportTask>,
    // IndexMap preserves insertion order at serialization time (matches the
    // python parity contract); ts-rs treats it as a string-keyed record.
    #[ts(type = "Record<string, string>")]
    pub metadata: IndexMap<String, String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ExportWorkspace {
    pub id: Uuid,
    pub name: Option<String>,
    pub container_ref: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ExportTask {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub description_source: Option<DescriptionSource>,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DescriptionSource {
    Local,
    Remote,
}

// ----- Env-block parser (parity with developer-sandbox/cli/commands/init_workspace.py:128-153) -----

static META_KEY_RE: OnceLock<Regex> = OnceLock::new();
static FENCE_RE: OnceLock<Regex> = OnceLock::new();

fn meta_key_re() -> &'static Regex {
    META_KEY_RE.get_or_init(|| Regex::new(r"^([A-Z][A-Z0-9_]*)=(.*)$").expect("static regex"))
}

fn fence_re() -> &'static Regex {
    // (?si) = case-insensitive + dotall. Matches `(?:\.?env|workspace)` —
    // covers both "```env" / "```.env" / "```workspace" labels.
    FENCE_RE.get_or_init(|| {
        Regex::new(r"(?si)```(?:\.?env|workspace)\s*\n(.*?)```").expect("static regex")
    })
}

/// Extract KEY=VALUE pairs from the first ` ```env ` or ` ```workspace ` fenced
/// block in the description. Behavior matches
/// `developer-sandbox/cli/commands/init_workspace.py::_parse_workspace_meta`.
pub fn parse_workspace_meta(description: Option<&str>) -> IndexMap<String, String> {
    let Some(description) = description else {
        return IndexMap::new();
    };
    if description.is_empty() {
        return IndexMap::new();
    }
    // Normalize markdown escape sequences: \` → ` and \_ → _
    let normalized = description.replace("\\`", "`").replace("\\_", "_");
    let Some(caps) = fence_re().captures(&normalized) else {
        return IndexMap::new();
    };
    let block = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let mut meta: IndexMap<String, String> = IndexMap::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(caps) = meta_key_re().captures(trimmed) {
            let key = caps.get(1).unwrap().as_str().to_string();
            let val = caps.get(2).unwrap().as_str().trim().to_string();
            if !meta.contains_key(&key) {
                meta.insert(key, val);
            }
        }
    }
    meta
}

// ----- Service -----

/// Workspace export service. Holds a `&SqlitePool` (FileService pattern) plus
/// an optional `&RemoteClient` for the transparent remote-issue fallback.
/// Does NOT depend on `local-deployment` — keeps `crates/services` decoupled.
pub struct ExportService<'a> {
    pool: &'a SqlitePool,
    remote_client: Option<&'a RemoteClient>,
}

impl<'a> ExportService<'a> {
    pub fn new(pool: &'a SqlitePool, remote_client: Option<&'a RemoteClient>) -> Self {
        Self {
            pool,
            remote_client,
        }
    }

    pub async fn export_by_id(&self, id: Uuid) -> Result<ExportPayload, ExportError> {
        let ws = Workspace::find_by_id(self.pool, id)
            .await?
            .ok_or(ExportError::WorkspaceNotFound)?;

        let (task_payload, metadata) = if let Some(task_id) = ws.task_id {
            if let Some(task) = Task::find_by_id(self.pool, task_id).await? {
                let (description, source) = match task.description.as_deref() {
                    Some(local) if !local.is_empty() => {
                        (Some(local.to_string()), Some(DescriptionSource::Local))
                    }
                    _ => match Self::try_remote_description(&ws, self.remote_client).await {
                        Some(d) => (Some(d), Some(DescriptionSource::Remote)),
                        // D-14: serialize null when the remote fallback fails.
                        None => (None, None),
                    },
                };
                let metadata = parse_workspace_meta(description.as_deref());
                let payload = ExportTask {
                    id: task.id,
                    title: task.title,
                    description,
                    description_source: source,
                };
                (Some(payload), metadata)
            } else {
                (None, IndexMap::new())
            }
        } else {
            (None, IndexMap::new())
        };

        Ok(ExportPayload {
            schema: SCHEMA_V1.to_string(),
            workspace: ExportWorkspace {
                id: ws.id,
                name: ws.name,
                container_ref: ws.container_ref,
                archived: ws.archived,
            },
            task: task_payload,
            metadata,
        })
    }

    /// D-14: opportunistic remote-issue description fallback. Mirrors
    /// `developer-sandbox/cli/commands/init_workspace.py:177-196`:
    ///
    /// `workspace.id` → `client.get_workspace_by_local_id` → `issue_id` →
    /// `client.get_issue` → `issue.description`.
    ///
    /// Any failure at any step returns `None` — the export still succeeds
    /// (`description_source: null`). Logs only the workspace_id / issue_id —
    /// never the description body (Security V7).
    async fn try_remote_description(
        workspace: &Workspace,
        remote_client: Option<&RemoteClient>,
    ) -> Option<String> {
        let client = remote_client?;

        let remote_ws = match client.get_workspace_by_local_id(workspace.id).await {
            Ok(rw) => rw,
            Err(err) => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    error = ?err,
                    "remote fallback: get_workspace_by_local_id failed; description_source: null"
                );
                return None;
            }
        };

        let issue_id = remote_ws.issue_id?;

        match client.get_issue(issue_id).await {
            Ok(issue) => issue.description,
            Err(err) => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    issue_id = %issue_id,
                    error = ?err,
                    "remote fallback: get_issue failed; description_source: null"
                );
                None
            }
        }
    }
}

// ----- Tests (parity with the python parser) -----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_description_returns_empty_map() {
        assert!(parse_workspace_meta(None).is_empty());
        assert!(parse_workspace_meta(Some("")).is_empty());
    }

    #[test]
    fn env_fence_with_single_key() {
        let desc = "before\n```env\nFEATURE=blog\n```\nafter";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"blog".to_string()));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn workspace_fence_label_also_accepted() {
        let desc = "```workspace\nFEATURE=blog\n```";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"blog".to_string()));
    }

    #[test]
    fn duplicate_keys_first_wins() {
        let desc = "```env\nFEATURE=blog\nFEATURE=second\n```";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"blog".to_string()));
    }

    #[test]
    fn preserves_insertion_order() {
        let desc = "```env\nFEATURE=a\nSERVICES=x,y\nJIRA_ID=T-1\n```";
        let m = parse_workspace_meta(Some(desc));
        let keys: Vec<&String> = m.keys().collect();
        assert_eq!(
            keys,
            vec![
                &"FEATURE".to_string(),
                &"SERVICES".to_string(),
                &"JIRA_ID".to_string(),
            ]
        );
    }

    #[test]
    fn trims_value_whitespace() {
        let desc = "```env\n  FEATURE=  blog  \n```";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"blog".to_string()));
    }

    #[test]
    fn lowercase_key_rejected() {
        let desc = "```env\nfeature=blog\n```";
        let m = parse_workspace_meta(Some(desc));
        assert!(m.is_empty());
    }

    #[test]
    fn markdown_backtick_escape_normalized() {
        let desc = "\\`\\`\\`env\nFEATURE=blog\n\\`\\`\\`";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"blog".to_string()));
    }

    #[test]
    fn markdown_underscore_escape_normalized() {
        let desc = "```env\nFEATURE\\_NAME=blog\n```";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE_NAME"), Some(&"blog".to_string()));
    }

    #[test]
    fn fence_label_case_insensitive() {
        let desc = "```Env\nFEATURE=x\n```";
        let m = parse_workspace_meta(Some(desc));
        assert_eq!(m.get("FEATURE"), Some(&"x".to_string()));
    }
}
