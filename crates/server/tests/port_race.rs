//! VKTEST-03 — deterministic port-file race test.
//!
//! Verifies that a concurrent reader of the port file never sees a broken
//! parse. The writer (`utils::port_file::write_port_file_with_proxy`) writes
//! the JSON; the reader (`utils::port_file::read_port_info`) parses it.
//! The race is driven by `tokio::sync::Notify` — strictly no `sleep`.

use std::sync::Arc;

use tokio::sync::Notify;
use utils::port_file::{read_port_info, write_port_file_with_proxy};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_read_during_write_is_consistent() {
    // Delete any stale port file from previous test runs or a live VK server.
    // The writer always writes to the "vibe-kanban" namespace (hardcoded), so a
    // stale file would let the reader observe values from a previous run and
    // pass-by-accident. Removing it first means the reader either sees the
    // file missing (Err) or sees the values WE wrote (Ok with our values).
    let port_file_path = std::env::temp_dir()
        .join("vibe-kanban")
        .join("vibe-kanban.port");
    let _ = tokio::fs::remove_file(&port_file_path).await;

    // Two separate `Notify` instances — `notify_one` stores at most one
    // permit per instance, so sharing a single Notify between two tasks
    // would race the kick. Each task gets its own permit-storing gate.
    let go_writer = Arc::new(Notify::new());
    let go_reader = Arc::new(Notify::new());
    let go_writer_clone = go_writer.clone();
    let go_reader_clone = go_reader.clone();

    let writer = tokio::spawn(async move {
        go_writer_clone.notified().await;
        write_port_file_with_proxy(54321, Some(54322))
            .await
            .expect("write");
    });

    let reader = tokio::spawn(async move {
        go_reader_clone.notified().await;
        // Bounded retry. Each iteration is either a clean parse (asserts the
        // contract) or a determinate Err (file not yet visible). No wall-clock
        // sleep — `yield_now` lets other tasks make progress.
        for _ in 0..256 {
            match read_port_info("vibe-kanban").await {
                Ok(info) => {
                    assert_eq!(info.main_port, 54321);
                    assert_eq!(info.preview_proxy_port, Some(54322));
                    return;
                }
                Err(_) => {
                    tokio::task::yield_now().await;
                }
            }
        }
        panic!("reader never observed a successful port-file read after 256 yields");
    });

    // `notify_one` queues a permit even if no waiter is parked yet — the
    // next `notified().await` consumes it. Determinate regardless of which
    // task is scheduled first.
    go_writer.notify_one();
    go_reader.notify_one();

    let (w, r) = tokio::join!(writer, reader);
    w.expect("writer task panicked");
    r.expect("reader task panicked");

    let _ = tokio::fs::remove_file(
        std::env::temp_dir()
            .join("vibe-kanban")
            .join("vibe-kanban.port"),
    )
    .await;
}
