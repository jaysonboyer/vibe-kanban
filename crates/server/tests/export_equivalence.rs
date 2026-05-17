//! VKTEST-04 — `vibe-kanban export workspace <uuid>` stdout JSON must equal
//! the `GET /api/export/workspace/<uuid>` response JSON.
//!
//! Seeds a minimal task-less workspace (task_id NULL) directly via
//! `Workspace::create` so the export returns `task: null, metadata: {}` —
//! enough to validate the equivalence contract without needing a project
//! row + task row.

use std::{process::Command, time::Duration};

use db::models::workspace::{CreateWorkspace, Workspace};
use deployment::Deployment;
use server::startup::start_with_bind;
use services::services::export::ExportPayload;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread")]
async fn cli_export_matches_http_export() {
    let token = CancellationToken::new();
    // Bind explicitly to IPv4 — the CLI subprocess connects via 127.0.0.1 and on
    // macOS "localhost" resolves to ::1 (IPv6) first, which would mismatch.
    let handle = start_with_bind("127.0.0.1:0", "127.0.0.1:0", token.clone())
        .await
        .expect("start_with_bind");
    let url = handle.url();
    let port = handle.port;

    // Seed a minimal workspace directly. CreateWorkspace doesn't take a
    // task_id, so the row has task_id=NULL → export returns task: null.
    let pool = &handle.deployment.db().pool;
    let test_uuid = uuid::Uuid::new_v4();
    let seed = CreateWorkspace {
        branch: "vk-test-export-equivalence".to_string(),
        name: Some("export-equivalence-seed".to_string()),
    };
    Workspace::create(pool, &seed, test_uuid)
        .await
        .expect("seed workspace");

    let serve = tokio::spawn(handle.serve());

    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(r) = client.get(format!("{url}/api/health")).send().await
            && r.status().is_success()
        {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("never ready");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let http_resp = client
        .get(format!("{url}/api/export/workspace/{test_uuid}"))
        .send()
        .await
        .expect("http get");
    assert!(
        http_resp.status().is_success(),
        "http get failed: {:?}",
        http_resp.status()
    );
    let http_payload: ExportPayload = http_resp.json().await.expect("http json parse");

    let server_bin = env!("CARGO_BIN_EXE_server");
    let output = Command::new(server_bin)
        .args(["export", "workspace", &test_uuid.to_string()])
        .env("BACKEND_PORT", port.to_string())
        .output()
        .expect("cli spawn");
    assert!(
        output.status.success(),
        "cli exit: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let cli_payload: ExportPayload =
        serde_json::from_slice(&output.stdout).expect("cli stdout json parse");

    // Equivalence: canonical serde_json::Value comparison. Both sides went
    // through `ExportPayload` deserialize → serialize, so any difference would
    // indicate a contract drift between the HTTP route and the CLI.
    let http_json = serde_json::to_value(&http_payload).expect("to_value http");
    let cli_json = serde_json::to_value(&cli_payload).expect("to_value cli");
    assert_eq!(http_json, cli_json, "CLI and HTTP export payloads differ");

    // Schema invariants — guard against accidental shape drift.
    assert_eq!(
        http_json.get("$schema").and_then(|v| v.as_str()),
        Some("vk-export/v1")
    );
    assert!(http_json.get("task").map(|v| v.is_null()).unwrap_or(false));
    assert!(
        http_json
            .get("metadata")
            .map(|v| v.as_object().is_some_and(|o| o.is_empty()))
            .unwrap_or(false)
    );

    token.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), serve).await;
}
