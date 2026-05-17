use crate::remote_cli::{
    args::LogsArgs,
    compose::{ComposeInvocation, LogsFlags},
    docker,
    env::{resolve_env_file_path, resolve_repo_root},
    error::RemoteCliError,
};

/// `vibe-kanban remote logs [service] [flags]` — thin pass-through to
/// `docker compose logs`. Races subprocess wait against Ctrl-C so the user
/// can interrupt a long `--follow` cleanly without leaving zombie
/// processes (T-01.1-11).
pub async fn run(args: LogsArgs) -> Result<(), RemoteCliError> {
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
    let flags = LogsFlags {
        follow: args.follow,
        tail: args.tail.clone(),
        since: args.since.clone(),
        timestamps: args.timestamps,
        no_color: args.no_color,
    };
    let mut child = invocation
        .logs(args.service.as_deref(), &flags)
        .spawn()
        .map_err(RemoteCliError::Io)?;

    let status = tokio::select! {
        s = child.wait() => s.map_err(RemoteCliError::Io)?,
        _ = tokio::signal::ctrl_c() => {
            // Forward SIGINT to compose; it tears down the log stream cleanly.
            let _ = child.kill().await;
            return Ok(());
        }
    };

    if !status.success() {
        return Err(RemoteCliError::ComposeFailed {
            code: status.code().unwrap_or(-1),
            stderr: String::new(),
        });
    }
    Ok(())
}
