use std::time::Duration;

use crate::remote_cli::{
    args::UpArgs,
    compose::{BuildMode, ComposeInvocation},
    docker,
    env::{parse_env_file, resolve_env_file_path, resolve_repo_root, validate},
    error::RemoteCliError,
};

/// `vibe-kanban remote up` — bring the cloud-mode stack to a healthy state
/// (VKREMOTE-01, VKREMOTE-05).
///
/// Flow:
///   1. Resolve `.env.remote` via D-10 search order
///   2. Parse + validate (hard-fail on missing required keys per D-11) —
///      runs BEFORE any docker call (T-01.1-07 / VKREMOTE-06)
///   3. Detect docker daemon (10s timeout — T-01.1-03)
///   4. First-time-build banner (Discretion #1)
///   5. Optional explicit build phase, time-bounded by `--build-timeout`
///   6. `docker compose up -d --wait --wait-timeout` (Discretion #2)
///   7. Print endpoint summary
pub async fn run(args: UpArgs) -> Result<(), RemoteCliError> {
    // 1. Env-file resolve + parse + validate (pre-docker)
    let env_path = resolve_env_file_path(args.env_file.as_deref())?;
    let parsed = parse_env_file(&env_path)?;
    validate(&parsed)?;

    // 2. Docker detection
    docker::detect_docker().await?;

    // 3. ComposeInvocation
    let invocation = ComposeInvocation {
        repo_root: resolve_repo_root()?,
        env_file: env_path,
        project_name: args
            .project_name
            .clone()
            .unwrap_or_else(|| "vibe-kanban-remote".to_string()),
        profiles: collect_profiles(&args),
    };

    // 4. First-time-build banner (cheap heuristic — only when we're about to build)
    if !args.no_build && !image_cached(&invocation).await {
        tracing::info!("First-time build — this can take 5-15 minutes. Re-runs will be fast.");
    }

    // 5. Optional explicit build phase
    if args.build {
        let build_status = tokio::time::timeout(
            Duration::from_secs(args.build_timeout),
            invocation.build_only().status(),
        )
        .await
        .map_err(|_| {
            RemoteCliError::Usage(format!(
                "Build exceeded {}s timeout (--build-timeout)",
                args.build_timeout
            ))
        })?
        .map_err(RemoteCliError::Io)?;
        if !build_status.success() {
            return Err(RemoteCliError::ComposeFailed {
                code: build_status.code().unwrap_or(-1),
                stderr: String::new(),
            });
        }
    }

    // 6. Up + wait
    let build_mode = if args.build {
        BuildMode::Force
    } else if args.no_build {
        BuildMode::Never
    } else {
        BuildMode::Auto
    };
    let status = invocation
        .up(build_mode, args.health_timeout, !args.no_wait)
        .status()
        .await
        .map_err(RemoteCliError::Io)?;
    if !status.success() {
        return Err(RemoteCliError::ComposeFailed {
            code: status.code().unwrap_or(-1),
            stderr: String::new(),
        });
    }

    // 7. Endpoint summary
    print!("{}", endpoint_summary(&args));
    Ok(())
}

fn collect_profiles(args: &UpArgs) -> Vec<&'static str> {
    let mut p = Vec::new();
    if args.with_attachments {
        p.push("attachments");
    }
    if args.with_relay {
        p.push("relay");
    }
    p
}

fn endpoint_summary(args: &UpArgs) -> String {
    let mut s = String::from("\nStack healthy. Endpoints:\n");
    s.push_str("  UI:        http://localhost:3000\n");
    s.push_str("  Postgres:  localhost:5433\n");
    if args.with_relay {
        s.push_str("  Relay:     localhost:8082\n");
    }
    if args.with_attachments {
        s.push_str("  Azurite:   localhost:10000\n");
    }
    s
}

async fn image_cached(inv: &ComposeInvocation) -> bool {
    match inv.images_for_remote_server().output().await {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    fn empty_up_args() -> UpArgs {
        UpArgs {
            with_attachments: false,
            with_relay: false,
            env_file: None,
            build: false,
            no_build: false,
            build_timeout: 600,
            health_timeout: 120,
            no_wait: false,
            project_name: None,
        }
    }

    #[test]
    fn collect_profiles_empty_when_no_flags() {
        assert_eq!(collect_profiles(&empty_up_args()), Vec::<&str>::new());
    }

    #[test]
    fn collect_profiles_attachments_only() {
        let mut a = empty_up_args();
        a.with_attachments = true;
        assert_eq!(collect_profiles(&a), vec!["attachments"]);
    }

    #[test]
    fn collect_profiles_relay_only() {
        let mut a = empty_up_args();
        a.with_relay = true;
        assert_eq!(collect_profiles(&a), vec!["relay"]);
    }

    #[test]
    fn collect_profiles_both_in_order() {
        let mut a = empty_up_args();
        a.with_attachments = true;
        a.with_relay = true;
        assert_eq!(collect_profiles(&a), vec!["attachments", "relay"]);
    }

    #[test]
    fn endpoint_summary_default_has_ui_and_postgres() {
        let s = endpoint_summary(&empty_up_args());
        assert!(s.contains("UI: "));
        assert!(s.contains("http://localhost:3000"));
        assert!(s.contains("Postgres:"));
        assert!(s.contains("localhost:5433"));
        assert!(!s.contains("Relay:"));
        assert!(!s.contains("Azurite:"));
    }

    #[test]
    fn endpoint_summary_with_relay() {
        let mut a = empty_up_args();
        a.with_relay = true;
        let s = endpoint_summary(&a);
        assert!(s.contains("Relay:"));
        assert!(s.contains("localhost:8082"));
    }

    #[test]
    fn endpoint_summary_with_attachments() {
        let mut a = empty_up_args();
        a.with_attachments = true;
        let s = endpoint_summary(&a);
        assert!(s.contains("Azurite:"));
        assert!(s.contains("localhost:10000"));
    }

    /// Critical: this test must pass on a machine WITHOUT docker installed.
    /// It exercises the pre-docker validation path — env file missing the
    /// required JWT secret causes `up::run` to exit non-zero before any
    /// docker subprocess is spawned. If `docker::detect_docker()` were
    /// called first, this test would flake on docker-less hosts.
    #[tokio::test]
    async fn up_returns_missing_env_error_before_docker() {
        let dir = TempDir::new().unwrap();
        let env_path = dir.path().join(".env.remote");
        // Intentionally omit VIBEKANBAN_REMOTE_JWT_SECRET.
        std::fs::write(&env_path, "SOME_OTHER_KEY=value\n").unwrap();
        let mut args = empty_up_args();
        args.env_file = Some(env_path);
        let err = run(args).await.unwrap_err();
        match err {
            RemoteCliError::MissingRequiredEnv { key, .. } => {
                assert_eq!(key, "VIBEKANBAN_REMOTE_JWT_SECRET");
            }
            other => panic!("expected MissingRequiredEnv (pre-docker validation), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn up_returns_env_file_not_found_before_docker() {
        // env_file pointing at a non-existent path should surface Io error
        // from parse_env_file (not get past env validation to docker probe).
        let mut args = empty_up_args();
        args.env_file = Some(PathBuf::from("/definitely/does/not/exist/.env.remote"));
        let err = run(args).await.unwrap_err();
        assert!(
            matches!(err, RemoteCliError::Io(_)),
            "expected Io error from parse_env_file, got {err:?}"
        );
    }
}
