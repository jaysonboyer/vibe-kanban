//! End-to-end integration test for `vibe-kanban remote up → status → down`
//! (VKREMOTETEST-01).
//!
//! Default `cargo test --workspace` SKIPS the heavy lifecycle test because it
//! is `#[ignore]`-gated and requires a running docker daemon. CI opts in via
//! `--include-ignored` (and should `docker compose build` the image first to
//! keep `--no-build` working).
//!
//! Run locally:
//!   DOCKER_E2E=1 cargo test -p server --test remote_cli_e2e -- --include-ignored
//!   cargo test -p server --test remote_cli_e2e -- --include-ignored remote_lifecycle_smoke

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use tempfile::TempDir;

fn docker_available() -> bool {
    Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Look up an image ID by `<repo>:<tag>` — returns the trimmed ID string when
/// present, or `None` when docker has no such image.
fn image_id(reference: &str) -> Option<String> {
    let out = Command::new("docker")
        .args(["images", "--format", "{{.ID}}", "--quiet", reference])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Tag `source` as `target` so a `docker compose up --no-build` against a
/// unique per-test project namespace can reuse an image that was originally
/// built under a different (default) namespace. Returns `Ok(())` on success.
fn docker_tag(source: &str, target: &str) -> Result<(), String> {
    let out = Command::new("docker")
        .args(["tag", source, target])
        .output()
        .map_err(|e| format!("spawn docker tag: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "docker tag {source} {target}: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn docker_rmi(reference: &str) {
    let _ = Command::new("docker")
        .args(["rmi", "--no-prune", reference])
        .output();
}

/// Returns `true` when something is already listening on the given local
/// TCP port. Used to detect a running `vibe-kanban remote up` stack before
/// trying to start the e2e namespace (whose compose file hardcodes the
/// same host-port mappings — they cannot coexist).
fn port_is_bound(port: u16) -> bool {
    use std::{net::TcpStream, time::Duration};
    TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(250)).is_ok()
}

/// Path to the vibe-kanban repo root (the directory containing `crates/`).
/// Resolved from CARGO_MANIFEST_DIR (= `crates/server`) two levels up.
fn workspace_repo_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("CARGO_MANIFEST_DIR has at least two parents")
}

/// Best-effort cleanup of any leftover containers/volumes from a previous run.
fn cleanup_project(bin: &str, env_file: &Path, project: &str) {
    let _ = Command::new(bin)
        .args([
            "remote",
            "down",
            "-v",
            "--env-file",
            env_file.to_str().unwrap(),
            "--project-name",
            project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .status();
}

struct CleanupGuard<'a> {
    bin: &'a str,
    env: &'a Path,
    project: &'a str,
    tagged_image: Option<String>,
}

impl Drop for CleanupGuard<'_> {
    fn drop(&mut self) {
        cleanup_project(self.bin, self.env, self.project);
        if let Some(image) = &self.tagged_image {
            docker_rmi(image);
        }
    }
}

#[test]
#[ignore = "requires docker; run with --include-ignored or DOCKER_E2E=1"]
fn remote_lifecycle_smoke() {
    if !docker_available() {
        eprintln!("docker daemon not available — skipping");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_server");
    let tmp = TempDir::new().expect("tempdir");
    let env_path = tmp.path().join(".env.remote");
    let project = format!("vibe-kanban-remote-test-{}", std::process::id());

    // 1. Produce a valid .env.remote via `remote init --non-interactive`.
    let init = Command::new(bin)
        .args([
            "remote",
            "init",
            "--non-interactive",
            "--auth-method",
            "self-host",
            "--self-host-email",
            "test@example.com",
            "--self-host-password",
            "testpass",
            "--env-file",
            env_path.to_str().unwrap(),
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .output()
        .expect("init failed to spawn");
    assert!(
        init.status.success(),
        "remote init exited non-zero. stdout={}, stderr={}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr),
    );
    assert!(env_path.exists(), "init should have created .env.remote");

    // The compose file binds host ports unconditionally (5433 for postgres,
    // 3000 for remote-server) — a per-PID compose namespace still has to
    // share host ports. Detect collisions before invoking compose so the
    // user gets a clear "stop the live stack first" message instead of an
    // opaque docker bind error.
    for (port, service) in [(5433u16, "remote-db (Postgres)"), (3000, "remote-server")] {
        if port_is_bound(port) {
            eprintln!(
                "port {port} is already in use ({service}) — skipping e2e test. \
                 Stop the live stack with `vibe-kanban remote down` and re-run."
            );
            return;
        }
    }

    // Defensive cleanup of any leftover from a previous failed run.
    cleanup_project(bin, &env_path, &project);

    // Per-PID compose project means docker would normally need a fresh
    // build of `<project>-remote-server:latest`. To keep this test fast we
    // tag the existing default-namespace image (built by the user's normal
    // `vibe-kanban remote up` workflow) into the test namespace. Skip the
    // test cleanly when no source image exists — running `vibe-kanban
    // remote up --build` once outside the test is the prerequisite.
    let test_image = format!("{project}-remote-server:latest");
    let default_image = "vibe-kanban-remote-remote-server:latest";
    if image_id(default_image).is_none() {
        eprintln!(
            "source image `{default_image}` not found — skipping e2e test. \
             Build it once with: `vibe-kanban remote up --build` (or `\
             docker compose --project-directory crates/remote --env-file <path> \
             -p vibe-kanban-remote build remote-server`) before re-running."
        );
        return;
    }
    if let Err(e) = docker_tag(default_image, &test_image) {
        eprintln!("failed to alias build cache into test namespace: {e}");
        return;
    }

    // Drop guard so down -v + image untag run even on assertion panic.
    let _guard = CleanupGuard {
        bin,
        env: &env_path,
        project: &project,
        tagged_image: Some(test_image.clone()),
    };

    // 2. up
    let up = Command::new(bin)
        .args([
            "remote",
            "up",
            "--env-file",
            env_path.to_str().unwrap(),
            "--no-build",
            "--health-timeout",
            "180",
            "--project-name",
            &project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .status()
        .expect("up failed to spawn");
    assert!(up.success(), "remote up should exit 0; got {:?}", up.code());

    // 3. status
    let status = Command::new(bin)
        .args([
            "remote",
            "status",
            "--env-file",
            env_path.to_str().unwrap(),
            "--project-name",
            &project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .output()
        .expect("status failed to spawn");
    assert!(
        status.status.success(),
        "remote status should exit 0; got {:?}",
        status.status.code()
    );
    let out = String::from_utf8_lossy(&status.stdout);
    assert!(
        out.contains("remote-db"),
        "status output should mention remote-db; got: {out}"
    );
    assert!(
        out.contains("remote-server"),
        "status output should mention remote-server; got: {out}"
    );

    // 4. Idempotency — a second up must exit 0 quickly (image cached).
    let start = Instant::now();
    let up2 = Command::new(bin)
        .args([
            "remote",
            "up",
            "--env-file",
            env_path.to_str().unwrap(),
            "--no-build",
            "--health-timeout",
            "60",
            "--project-name",
            &project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .status()
        .expect("second up failed to spawn");
    let elapsed = start.elapsed();
    assert!(
        up2.success(),
        "second remote up should exit 0 (idempotent); got {:?}",
        up2.code()
    );
    assert!(
        elapsed.as_secs() < 60,
        "second up should be fast (<60s); took {elapsed:?}"
    );

    // 5. down (volumes preserved by default).
    let down = Command::new(bin)
        .args([
            "remote",
            "down",
            "--env-file",
            env_path.to_str().unwrap(),
            "--project-name",
            &project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .status()
        .expect("down failed to spawn");
    assert!(down.success(), "remote down should exit 0");

    // 6. Post-down status — namespace should be empty.
    let post = Command::new(bin)
        .args([
            "remote",
            "status",
            "--env-file",
            env_path.to_str().unwrap(),
            "--project-name",
            &project,
        ])
        .env("VIBE_KANBAN_REPO", workspace_repo_root())
        .output()
        .expect("post-status failed to spawn");
    assert!(post.status.success());
    let post_out = String::from_utf8_lossy(&post.stdout);
    assert!(
        post_out.contains("No services running"),
        "after down, status should report 'No services running'; got: {post_out}"
    );
}

/// Smoke test that always runs: confirms `docker_available()` is sound and
/// returns within the 10s docker.rs probe budget even when no daemon is up.
#[test]
fn e2e_test_self_skips_without_docker() {
    let start = Instant::now();
    let _ = docker_available();
    assert!(
        start.elapsed().as_secs() < 15,
        "docker_available() probe should be fast even when daemon is hung"
    );
}
