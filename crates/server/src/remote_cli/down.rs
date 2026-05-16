use crate::remote_cli::{
    args::DownArgs,
    compose::ComposeInvocation,
    docker,
    env::{resolve_env_file_path, resolve_repo_root},
    error::RemoteCliError,
};

/// `vibe-kanban remote down` — invokes `docker compose down [-v]`.
///
/// Deliberately does NOT parse or validate `.env.remote` — secrets aren't
/// needed to shut down. Compose just needs to know which file to point
/// `--env-file` at so it can resolve the project namespace correctly.
/// This lets `remote down` recover from a corrupted env file.
pub async fn run(args: DownArgs) -> Result<(), RemoteCliError> {
    let env_path = resolve_env_file_path(args.env_file.as_deref())?;
    docker::detect_docker().await?;
    let invocation = ComposeInvocation {
        repo_root: resolve_repo_root()?,
        env_file: env_path,
        project_name: args
            .project_name
            .clone()
            .unwrap_or_else(|| "vibe-kanban-remote".to_string()),
        profiles: Vec::new(),
    };
    let status = invocation
        .down(args.volumes)
        .status()
        .await
        .map_err(RemoteCliError::Io)?;
    if !status.success() {
        return Err(RemoteCliError::ComposeFailed {
            code: status.code().unwrap_or(-1),
            stderr: String::new(),
        });
    }
    Ok(())
}
