//! GET /api/export/workspace/{id}
//! GET /api/export/workspace?container_ref=<path>
//!
//! Returns the canonical workspace export payload (D-11). Unlike most routes
//! in this codebase, the export endpoints return the **raw** payload —
//! NOT wrapped in `ApiResponse<T>` — so external consumers (sandbox, skills)
//! parse it directly without unwrapping an envelope.

use axum::{
    Router,
    extract::{Path, Query, State},
    response::Json,
    routing::get,
};
use db::models::workspace::{Workspace, WorkspaceError};
use deployment::Deployment;
use serde::Deserialize;
use services::services::export::{ExportPayload, ExportService};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct ContainerRefQuery {
    pub container_ref: Option<String>,
}

pub async fn export_workspace_by_id(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<Json<ExportPayload>, ApiError> {
    let pool = &deployment.db().pool;
    let remote_client = deployment.remote_client().ok();
    let svc = ExportService::new(pool, remote_client.as_ref());
    let payload = svc.export_by_id(id).await?;
    Ok(Json(payload))
}

pub async fn export_workspace_by_path(
    State(deployment): State<DeploymentImpl>,
    Query(q): Query<ContainerRefQuery>,
) -> Result<Json<ExportPayload>, ApiError> {
    let container_ref = q
        .container_ref
        .ok_or_else(|| ApiError::BadRequest("container_ref query parameter is required".into()))?;

    let pool = &deployment.db().pool;
    let info = Workspace::resolve_container_ref_by_prefix(pool, &container_ref)
        .await
        .map_err(|_| ApiError::Workspace(WorkspaceError::WorkspaceNotFound))?;

    let remote_client = deployment.remote_client().ok();
    let svc = ExportService::new(pool, remote_client.as_ref());
    let payload = svc.export_by_id(info.workspace_id).await?;
    Ok(Json(payload))
}

pub(super) fn router(_: &DeploymentImpl) -> Router<DeploymentImpl> {
    let export_router = Router::new()
        .route("/workspace/{id}", get(export_workspace_by_id))
        .route("/workspace", get(export_workspace_by_path));
    Router::new().nest("/export", export_router)
}
