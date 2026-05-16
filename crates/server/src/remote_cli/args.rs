use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

/// Top-level `vibe-kanban remote ...` argument group.
///
/// Attaches to the parent `Commands::Remote(RemoteArgs)` arm in `main.rs`.
#[derive(Debug, Args)]
pub struct RemoteArgs {
    #[command(subcommand)]
    pub command: RemoteCommand,
}

#[derive(Debug, Subcommand)]
pub enum RemoteCommand {
    /// Bring up the VK remote docker stack (VKREMOTE-01)
    Up(UpArgs),
    /// Stop the VK remote docker stack (VKREMOTE-02)
    Down(DownArgs),
    /// Show running services with health (VKREMOTE-03)
    Status(StatusArgs),
    /// Tail compose logs for one or all services (VKREMOTE-04)
    Logs(LogsArgs),
    /// Bootstrap .env.remote (VKREMOTE-06, D-12)
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct UpArgs {
    /// Enable the `attachments` profile (azurite) — D-06
    #[arg(long)]
    pub with_attachments: bool,
    /// Enable the `relay` profile (relay-server) — D-06
    #[arg(long)]
    pub with_relay: bool,
    /// Path to .env.remote (overrides D-10 search order)
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
    /// Force rebuild the remote-server image (passes --build to compose)
    #[arg(long)]
    pub build: bool,
    /// Fail if no cached image (passes --no-build to compose)
    #[arg(long, conflicts_with = "build")]
    pub no_build: bool,
    /// Max seconds to wait for the build phase (default 600)
    #[arg(long, default_value_t = 600, value_name = "SECS")]
    pub build_timeout: u64,
    /// Max seconds to wait for healthchecks (default 120, --timeout aliased)
    #[arg(long, alias = "timeout", default_value_t = 120, value_name = "SECS")]
    pub health_timeout: u64,
    /// Skip the health-wait phase; return as soon as `up -d` exits
    #[arg(long)]
    pub no_wait: bool,
    /// Compose project name override (test-only).
    #[arg(long, hide = true)]
    pub project_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct DownArgs {
    /// Remove named volumes too (destructive — drops postgres data)
    #[arg(short = 'v', long)]
    pub volumes: bool,
    /// Path to .env.remote (overrides D-10 search order)
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
    /// Compose project name override (test-only).
    #[arg(long, hide = true)]
    pub project_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Path to .env.remote (overrides D-10 search order)
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
    /// Compose project name override (test-only).
    #[arg(long, hide = true)]
    pub project_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    /// Service name (e.g. `remote-server`). Omit to tail all services.
    pub service: Option<String>,
    /// Follow log output (passes -f to compose)
    #[arg(short = 'f', long)]
    pub follow: bool,
    /// Number of lines to show from the end of the logs (default "100").
    /// Pass "all" to show every line.
    #[arg(long, default_value = "100", value_name = "N")]
    pub tail: Option<String>,
    /// Show logs since timestamp or relative duration (e.g. "1h", "2024-01-01").
    #[arg(long, value_name = "TIME")]
    pub since: Option<String>,
    /// Prefix each line with the timestamp.
    #[arg(long)]
    pub timestamps: bool,
    /// Disable colored output.
    #[arg(long)]
    pub no_color: bool,
    /// Path to .env.remote (overrides D-10 search order)
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
    /// Compose project name override (test-only).
    #[arg(long, hide = true)]
    pub project_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Skip interactive prompts; consume flags only (CI / dotfiles per D-13).
    #[arg(long)]
    pub non_interactive: bool,
    /// Auth method to provision; required under --non-interactive.
    #[arg(long, value_enum, value_name = "METHOD")]
    pub auth_method: Option<AuthMethod>,
    /// Override the generated JWT secret (base64). Auto-generated if absent.
    #[arg(long, value_name = "BASE64")]
    pub jwt_secret: Option<String>,
    /// Self-host email address (used with --auth-method self-host).
    #[arg(long, value_name = "EMAIL")]
    pub self_host_email: Option<String>,
    /// Self-host password (used with --auth-method self-host).
    #[arg(long, value_name = "PASSWORD")]
    pub self_host_password: Option<String>,
    /// GitHub OAuth client id (used with --auth-method github-oauth).
    #[arg(long, value_name = "ID")]
    pub github_client_id: Option<String>,
    /// GitHub OAuth client secret (used with --auth-method github-oauth).
    #[arg(long, value_name = "SECRET")]
    pub github_client_secret: Option<String>,
    /// Google OAuth client id (used with --auth-method google-oauth).
    #[arg(long, value_name = "ID")]
    pub google_client_id: Option<String>,
    /// Google OAuth client secret (used with --auth-method google-oauth).
    #[arg(long, value_name = "SECRET")]
    pub google_client_secret: Option<String>,
    /// Overwrite an existing .env.remote (destructive — drops existing secrets).
    #[arg(long)]
    pub force: bool,
    /// Add missing keys without touching existing values (D-14).
    #[arg(long, conflicts_with = "force")]
    pub merge: bool,
    /// Explicit target path for .env.remote (otherwise D-10 search order).
    #[arg(long, value_name = "PATH")]
    pub env_file: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum AuthMethod {
    #[value(name = "self-host")]
    SelfHost,
    #[value(name = "github-oauth")]
    GithubOauth,
    #[value(name = "google-oauth")]
    GoogleOauth,
}

#[cfg(test)]
mod tests {
    use clap::Args;

    use super::RemoteArgs;

    #[test]
    fn args_derive_is_valid() {
        let cmd = RemoteArgs::augment_args(clap::Command::new("test"));
        cmd.debug_assert();
    }
}
