//! VKTEST-02b — partial-init failure releases resources.
//!
//! Isolated in its own integration-test binary (one process per `tests/*.rs`)
//! so the `VK_TEST_FAIL_AFTER_DB` env var cannot leak into sibling tests in
//! `startup_integration.rs`. Uses plan 01-06's injection seam to force a
//! deterministic mid-init Err inside `LocalDeployment::new`; verifies the
//! constructor returned Err naming the seam; then re-invokes `start_with_bind`
//! to prove the scopeguard cleanup left the process in a usable state.

use std::time::Duration;

use server::startup::start_with_bind;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread")]
async fn partial_init_cleanup_releases_resources() {
    // SAFETY (Rust 2024): mutating the process-global env requires `unsafe`.
    // Removed before the assertion so a later panic cannot leak it.
    unsafe {
        std::env::set_var("VK_TEST_FAIL_AFTER_DB", "1");
    }

    let token = CancellationToken::new();
    let result =
        <local_deployment::LocalDeployment as deployment::Deployment>::new(token.clone()).await;

    unsafe {
        std::env::remove_var("VK_TEST_FAIL_AFTER_DB");
    }

    let err = match result {
        Ok(_) => panic!("expected VK_TEST_FAIL_AFTER_DB to force Err; got Ok"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("VK_TEST_FAIL_AFTER_DB"),
        "error message should name the injection seam; got: {msg}"
    );

    // Post-fault: a fresh init succeeds (proves the scopeguard cleanup did
    // not wedge the process — DB pool closed cleanly, OnceLock didn't latch).
    let token2 = CancellationToken::new();
    let handle = start_with_bind("localhost:0", "localhost:0", token2.clone())
        .await
        .expect("post-fault start_with_bind should succeed");
    let serve = tokio::spawn(handle.serve());
    token2.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), serve).await;
}
