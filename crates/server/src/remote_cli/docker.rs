use std::time::Duration;

use tokio::process::Command;

use crate::remote_cli::error::RemoteCliError;

/// Probe the docker daemon by running `docker version --format '{{.Server.Version}}'`.
///
/// - Returns `Err(DockerNotFound)` when `docker` is not on PATH.
/// - Returns `Err(DockerDaemonNotRunning)` when the binary exists but the
///   daemon is unreachable (typical Docker Desktop "still starting" state).
/// - Wraps the probe in a 10s `tokio::time::timeout` (T-01.1-03) so a hung
///   daemon cannot block the caller indefinitely.
pub async fn detect_docker() -> Result<(), RemoteCliError> {
    let probe = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output();

    let output = tokio::time::timeout(Duration::from_secs(10), probe)
        .await
        .map_err(|_| RemoteCliError::DockerDaemonNotRunning {
            stderr: "Docker daemon not responding (10s timeout). Is Docker Desktop still starting?"
                .to_string(),
        })?
        .map_err(|_| RemoteCliError::DockerNotFound)?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err(RemoteCliError::DockerDaemonNotRunning {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}
