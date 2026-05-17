use axum::{http::StatusCode, response::Json};
use serde_json::json;

use crate::readiness::Phase;

/// Readiness-aware health probe.
///
/// - During startup: `503 Service Unavailable` with body
///   `{ "status": "starting", "phase": <kebab-case phase name> }`.
/// - Once `Phase::Ready` is set: `200 OK` with body
///   `{ "status": "ok", "version": <APP_VERSION> }`.
///
/// Raw payload (NOT wrapped in `ApiResponse<T>`) per D-05 / D-11 — external
/// consumers (sandbox, skills, MCP clients) parse this contract directly.
pub(super) async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    let phase = Phase::current();
    match phase {
        Phase::Ready => (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "version": utils::version::APP_VERSION,
            })),
        ),
        other => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "starting",
                "phase": other,
            })),
        ),
    }
}
