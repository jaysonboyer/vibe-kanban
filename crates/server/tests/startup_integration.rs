//! VKTEST-01 + VKTEST-02 — Phase 1 startup integration tests.
//!
//! Uses `server::startup::start_with_bind` as the in-process fixture. Each
//! test gets a fresh OS-assigned port pair so they can run in parallel.

use std::time::{Duration, Instant};

use server::startup::start_with_bind;
use tokio_util::sync::CancellationToken;

/// VKTEST-01 — full boot → /api/health 200 → graceful shutdown.
#[tokio::test(flavor = "multi_thread")]
async fn full_boot_then_shutdown() {
    let token = CancellationToken::new();
    let handle = start_with_bind("localhost:0", "localhost:0", token.clone())
        .await
        .expect("start_with_bind should succeed");
    let url = handle.url();
    let serve = tokio::spawn(handle.serve());

    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last_status = None;
    loop {
        if let Ok(resp) = client
            .get(format!("{url}/api/health"))
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            last_status = Some(resp.status().as_u16());
            if resp.status().is_success() {
                let body: serde_json::Value =
                    resp.json().await.expect("health body should be JSON");
                assert_eq!(body.get("status").and_then(|v| v.as_str()), Some("ok"));
                assert!(
                    body.get("version").is_some(),
                    "ok body must include version"
                );
                break;
            }
        }
        if Instant::now() > deadline {
            panic!("Server never became ready; last status: {:?}", last_status);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    token.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), serve).await;
}

/// VKTEST-02a — graceful shutdown releases the bound port.
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_releases_listener() {
    let token = CancellationToken::new();
    let handle = start_with_bind("localhost:0", "localhost:0", token.clone())
        .await
        .expect("start_with_bind should succeed");
    let url = handle.url();
    let port = handle.port;
    let serve = tokio::spawn(handle.serve());

    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if client.get(format!("{url}/api/health")).send().await.is_ok() {
            break;
        }
        if Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    token.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), serve).await;

    // After shutdown the listener is released. Confirm by re-binding the same port.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut rebound = false;
    while Instant::now() < deadline {
        match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
            Ok(_) => {
                rebound = true;
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    assert!(
        rebound,
        "port {port} was not released within 2s of shutdown"
    );
}

/// VKTEST-02c — `start_with_bind` is reentrant: two invocations in the same
/// process do not panic. Validates plan 02 (.expect → .ok() on client_info
/// setters) and plan 06 (OnceLock-gated orphan cleanup so the second init
/// doesn't race a duplicate).
#[tokio::test(flavor = "multi_thread")]
async fn start_with_bind_is_reentrant() {
    for i in 0..2 {
        let token = CancellationToken::new();
        let handle = start_with_bind("localhost:0", "localhost:0", token.clone())
            .await
            .unwrap_or_else(|e| panic!("attempt {i} failed: {e}"));
        let serve = tokio::spawn(handle.serve());
        token.cancel();
        let _ = tokio::time::timeout(Duration::from_secs(5), serve).await;
    }
}
