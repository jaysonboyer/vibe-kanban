use std::path::PathBuf;

use thiserror::Error;

/// Typed error enum for `vibe-kanban remote ...` commands.
///
/// Display copy is the literal message a user sees (no further wrapping).
/// `exit_code()` returns the deterministic exit code per Phase 1 D-15 policy.
#[derive(Debug, Error)]
pub enum RemoteCliError {
    #[error("Docker is not installed or not on PATH. Install: https://docs.docker.com/get-docker/")]
    DockerNotFound,

    #[error(
        "Docker is installed but the daemon is not running. Start Docker Desktop (macOS) or `sudo systemctl start docker` (Linux).\n{stderr}"
    )]
    DockerDaemonNotRunning { stderr: String },

    #[error(
        "No .env.remote found. Searched:\n  {}\nRun `vibe-kanban remote init` to create one.",
        searched.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n  ")
    )]
    EnvFileNotFound { searched: Vec<PathBuf> },

    #[error(
        "Required env var {key} missing from {}. {hint}",
        env_file.display()
    )]
    MissingRequiredEnv {
        key: String,
        hint: String,
        env_file: PathBuf,
    },

    #[error("Cannot determine HOME directory. Set VIBE_KANBAN_REPO or pass --env-file.")]
    HomeUnset,

    #[error(
        ".env.remote already exists at {}.\n  --force overwrite (destructive)\n  --merge add missing keys\n  Or delete the file manually first.",
        path.display()
    )]
    ExistingEnvFile { path: PathBuf },

    #[error("docker compose exited with code {code}{}", suggest_port_check(stderr))]
    ComposeFailed { code: i32, stderr: String },

    #[error("Invalid argument: {0}")]
    Usage(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl RemoteCliError {
    /// D-15 exit code policy — sibling of Phase 1's mapping.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::ExistingEnvFile { .. } => 3,
            Self::EnvFileNotFound { .. } => 4,
            _ => 1,
        }
    }
}

/// Append a port-collision tip when compose's stderr contains the docker bind
/// error string (Discretion #6: rely on docker's own error + value-add tip).
pub fn suggest_port_check(stderr: &str) -> &'static str {
    if stderr.contains("port is already allocated") {
        "\nTip: 5433 and 127.0.0.1:3000 are this stack's ports. Find the conflicting process:\n  lsof -nP -iTCP -sTCP:LISTEN | grep -E '5433|3000'"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn usage_maps_to_exit_2() {
        assert_eq!(RemoteCliError::Usage("bad".into()).exit_code(), 2);
    }

    #[test]
    fn existing_env_file_maps_to_exit_3() {
        let err = RemoteCliError::ExistingEnvFile {
            path: PathBuf::from("/tmp/.env.remote"),
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn env_file_not_found_maps_to_exit_4() {
        let err = RemoteCliError::EnvFileNotFound {
            searched: vec![PathBuf::from("/a"), PathBuf::from("/b")],
        };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn other_variants_map_to_exit_1() {
        assert_eq!(RemoteCliError::DockerNotFound.exit_code(), 1);
        let dd = RemoteCliError::DockerDaemonNotRunning {
            stderr: String::new(),
        };
        assert_eq!(dd.exit_code(), 1);
        let mre = RemoteCliError::MissingRequiredEnv {
            key: "X".into(),
            hint: "Y".into(),
            env_file: PathBuf::from("/z"),
        };
        assert_eq!(mre.exit_code(), 1);
        assert_eq!(RemoteCliError::HomeUnset.exit_code(), 1);
        let cf = RemoteCliError::ComposeFailed {
            code: 7,
            stderr: String::new(),
        };
        assert_eq!(cf.exit_code(), 1);
        let io_err = RemoteCliError::Io(std::io::Error::other("oh no"));
        assert_eq!(io_err.exit_code(), 1);
    }

    #[test]
    fn port_check_tip_when_stderr_mentions_allocation() {
        let tip = suggest_port_check("Error: port is already allocated\n");
        assert!(tip.contains("5433"));
        assert!(tip.contains("3000"));
    }

    #[test]
    fn port_check_silent_when_unrelated() {
        assert_eq!(suggest_port_check("totally different error"), "");
    }
}
