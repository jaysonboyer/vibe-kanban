use std::path::PathBuf;

use tokio::process::Command;

/// Typed builder for `docker compose ...` invocations.
///
/// Every subcommand assembles its argv via this struct so we never pay the
/// "did we forget --env-file?" tax. Argv never crosses a shell — values are
/// passed via `.arg()` so paths with spaces or `$VAR`-looking strings are
/// safe (T-01.1-01).
pub struct ComposeInvocation {
    pub repo_root: PathBuf,
    pub env_file: PathBuf,
    /// Compose project namespace (D-07 Discretion #7) — default
    /// `"vibe-kanban-remote"`; tests pass an alternate name via the hidden
    /// `--project-name` flag.
    pub project_name: String,
    pub profiles: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    /// Use cached image if present, build otherwise (compose default).
    Auto,
    /// Always rebuild (passes `--build`).
    Force,
    /// Fail if no cached image (passes `--no-build`).
    Never,
}

/// Per-flag inputs for the `compose logs` invocation. Mirrors `docker compose
/// logs` natively-supported flags so we can pass them through unchanged.
#[derive(Debug, Default)]
pub struct LogsFlags {
    pub follow: bool,
    pub tail: Option<String>,
    pub since: Option<String>,
    pub timestamps: bool,
    pub no_color: bool,
}

impl ComposeInvocation {
    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new("docker");
        cmd.arg("compose")
            .arg("--project-directory")
            .arg(self.repo_root.join("crates/remote"))
            .arg("-p")
            .arg(&self.project_name)
            .arg("--env-file")
            .arg(&self.env_file);
        for p in &self.profiles {
            cmd.arg("--profile").arg(p);
        }
        cmd
    }

    pub fn up(&self, build: BuildMode, wait_timeout: u64, wait: bool) -> Command {
        let mut cmd = self.base_cmd();
        cmd.arg("up").arg("-d");
        match build {
            BuildMode::Auto => {}
            BuildMode::Force => {
                cmd.arg("--build");
            }
            BuildMode::Never => {
                cmd.arg("--no-build");
            }
        }
        if wait {
            cmd.arg("--wait")
                .arg("--wait-timeout")
                .arg(wait_timeout.to_string());
        }
        cmd
    }

    pub fn build_only(&self) -> Command {
        let mut cmd = self.base_cmd();
        cmd.arg("build");
        cmd
    }

    pub fn down(&self, remove_volumes: bool) -> Command {
        let mut cmd = self.base_cmd();
        cmd.arg("down");
        if remove_volumes {
            cmd.arg("-v");
        }
        cmd
    }

    pub fn ps_json(&self) -> Command {
        let mut cmd = self.base_cmd();
        cmd.arg("ps").arg("--format").arg("json").arg("--all");
        cmd
    }

    pub fn logs(&self, service: Option<&str>, flags: &LogsFlags) -> Command {
        let mut cmd = self.base_cmd();
        cmd.arg("logs");
        if flags.follow {
            cmd.arg("--follow");
        }
        if let Some(tail) = &flags.tail {
            cmd.arg("--tail").arg(tail);
        }
        if let Some(since) = &flags.since {
            cmd.arg("--since").arg(since);
        }
        if flags.timestamps {
            cmd.arg("--timestamps");
        }
        if flags.no_color {
            cmd.arg("--no-color");
        }
        if let Some(svc) = service {
            cmd.arg(svc);
        }
        cmd
    }

    /// `docker images <project>-remote-server --format '{{.ID}}'` —
    /// up.rs uses this to detect whether a cached image already exists,
    /// so the first-build banner only fires when we actually have to build
    /// (Discretion #1).
    pub fn images_for_remote_server(&self) -> Command {
        let mut cmd = Command::new("docker");
        let image = format!("{}-remote-server", self.project_name);
        cmd.arg("images")
            .arg(image)
            .arg("--format")
            .arg("{{.ID}}");
        cmd
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn argv(cmd: &Command) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn fixture() -> ComposeInvocation {
        ComposeInvocation {
            repo_root: PathBuf::from("/repo"),
            env_file: PathBuf::from("/tmp/.env"),
            project_name: "test".into(),
            profiles: vec![],
        }
    }

    #[test]
    fn up_default_argv_matches_expected_sequence() {
        let cmd = fixture().up(BuildMode::Auto, 120, true);
        let args = argv(&cmd);
        assert_eq!(
            args,
            vec![
                "compose",
                "--project-directory",
                "/repo/crates/remote",
                "-p",
                "test",
                "--env-file",
                "/tmp/.env",
                "up",
                "-d",
                "--wait",
                "--wait-timeout",
                "120",
            ]
        );
    }

    #[test]
    fn up_with_build_force_adds_build_flag() {
        let cmd = fixture().up(BuildMode::Force, 120, true);
        let args = argv(&cmd);
        assert!(args.iter().any(|a| a == "--build"));
        assert!(!args.iter().any(|a| a == "--no-build"));
    }

    #[test]
    fn up_with_build_never_adds_no_build_flag() {
        let cmd = fixture().up(BuildMode::Never, 120, true);
        let args = argv(&cmd);
        assert!(args.iter().any(|a| a == "--no-build"));
        assert!(!args.iter().any(|a| a == "--build"));
    }

    #[test]
    fn up_no_wait_omits_wait_flags() {
        let cmd = fixture().up(BuildMode::Auto, 120, false);
        let args = argv(&cmd);
        assert!(!args.iter().any(|a| a == "--wait"));
        assert!(!args.iter().any(|a| a == "--wait-timeout"));
    }

    #[test]
    fn profiles_appear_before_subcommand() {
        let inv = ComposeInvocation {
            repo_root: PathBuf::from("/repo"),
            env_file: PathBuf::from("/tmp/.env"),
            project_name: "test".into(),
            profiles: vec!["attachments", "relay"],
        };
        let args = argv(&inv.up(BuildMode::Auto, 60, true));
        let attachments_idx = args.iter().position(|a| a == "attachments").unwrap();
        let relay_idx = args.iter().position(|a| a == "relay").unwrap();
        let up_idx = args.iter().position(|a| a == "up").unwrap();
        assert!(attachments_idx < up_idx);
        assert!(relay_idx < up_idx);
    }

    #[test]
    fn down_default_omits_volumes_flag() {
        let cmd = fixture().down(false);
        let args = argv(&cmd);
        assert!(args.iter().any(|a| a == "down"));
        assert!(!args.iter().any(|a| a == "-v"));
    }

    #[test]
    fn down_with_volumes_adds_v_flag() {
        let cmd = fixture().down(true);
        let args = argv(&cmd);
        assert!(args.iter().any(|a| a == "-v"));
    }

    #[test]
    fn ps_json_emits_format_and_all() {
        let cmd = fixture().ps_json();
        let args = argv(&cmd);
        let ps_idx = args.iter().position(|a| a == "ps").unwrap();
        let json_idx = args.iter().position(|a| a == "json").unwrap();
        let all_idx = args.iter().position(|a| a == "--all").unwrap();
        assert!(ps_idx < json_idx);
        assert!(ps_idx < all_idx);
    }

    #[test]
    fn logs_with_service_and_flags_forwards_all() {
        let flags = LogsFlags {
            follow: true,
            tail: Some("50".into()),
            since: Some("1h".into()),
            timestamps: true,
            no_color: true,
        };
        let cmd = fixture().logs(Some("remote-server"), &flags);
        let args = argv(&cmd);
        // Subcommand first.
        let logs_idx = args.iter().position(|a| a == "logs").unwrap();
        let follow_idx = args.iter().position(|a| a == "--follow").unwrap();
        let svc_idx = args.iter().position(|a| a == "remote-server").unwrap();
        assert!(logs_idx < follow_idx);
        // Service name comes last (after flags).
        assert!(follow_idx < svc_idx);
        // All flag pairs are present.
        assert!(args.windows(2).any(|w| w[0] == "--tail" && w[1] == "50"));
        assert!(args.windows(2).any(|w| w[0] == "--since" && w[1] == "1h"));
        assert!(args.iter().any(|a| a == "--timestamps"));
        assert!(args.iter().any(|a| a == "--no-color"));
    }

    #[test]
    fn images_argv_uses_project_namespace() {
        let cmd = fixture().images_for_remote_server();
        let args = argv(&cmd);
        assert_eq!(
            args,
            vec!["images", "test-remote-server", "--format", "{{.ID}}"]
        );
    }
}
