use anyhow::{self, Error as AnyhowError};
use axum::Router;
use clap::{Parser, Subcommand};
use deployment::{Deployment, DeploymentError};
use server::{
    DeploymentImpl, middleware::origin::validate_origin, remote_cli, routes,
    runtime::relay_registration,
};
use services::services::container::ContainerService;
use sqlx::Error as SqlxError;
use strip_ansi_escapes::strip;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tracing_subscriber::{EnvFilter, prelude::*};
use utils::{
    port_file::write_port_file_with_proxy,
    sentry::{self as sentry_utils, SentrySource, sentry_layer},
};

#[derive(Debug, Error)]
pub enum VibeKanbanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
    #[error(transparent)]
    Deployment(#[from] DeploymentError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
    #[error(
        "Port {port} is already in use. Set {env_var}=<n> to choose another, or stop the conflicting process."
    )]
    PortInUse { port: u16, env_var: &'static str },
    #[error(transparent)]
    Remote(#[from] remote_cli::error::RemoteCliError),
}

#[derive(Debug, Parser)]
#[command(
    name = "vibe-kanban",
    version,
    about = "Vibe Kanban server (default) or subcommand"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Bring up, monitor, and tear down VK's cloud-mode docker stack (Phase 01.1, D-01).
    Remote(remote_cli::args::RemoteArgs),
}

async fn run_subcommand(command: Command) -> Result<(), VibeKanbanError> {
    match command {
        Command::Remote(args) => remote_cli::run(args).await.map_err(VibeKanbanError::from),
    }
}

/// Resolve `(host, backend_port, proxy_port)` from env vars with mode-aware
/// defaults. Release builds get deterministic ports (8419/8420) so external
/// consumers (sandbox, skills, MCP clients) can hardcode a canonical URL;
/// debug builds keep the current behaviour (3000 + OS-assigned proxy) so
/// `pnpm run dev` continues to work without env overrides. D-01, D-02, D-06,
/// D-07, D-08.
fn resolve_ports() -> Result<(String, u16, u16), VibeKanbanError> {
    let backend_default: u16 = if cfg!(debug_assertions) { 3000 } else { 8419 };

    let backend = std::env::var("BACKEND_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|s| {
            // Strip ANSI escapes — defends against shell colour codes pasted
            // into env vars (CONCERNS: "Port Parsing Loses Type Safety").
            let cleaned = String::from_utf8(strip(s.as_bytes())).ok()?;
            cleaned.trim().parse::<u16>().ok()
        })
        .unwrap_or_else(|| {
            tracing::info!(
                mode = if cfg!(debug_assertions) {
                    "dev"
                } else {
                    "prod"
                },
                "No BACKEND_PORT/PORT set; using default {}",
                backend_default
            );
            backend_default
        });

    let proxy = match std::env::var("PREVIEW_PROXY_PORT")
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
    {
        Some(p) => p,
        None => {
            if cfg!(debug_assertions) {
                0
            } else {
                backend.checked_add(1).ok_or_else(|| {
                    VibeKanbanError::Other(anyhow::anyhow!(
                        "BACKEND_PORT {backend} too high to derive PREVIEW_PROXY_PORT={backend}+1; explicitly set PREVIEW_PROXY_PORT"
                    ))
                })?
            }
        }
    };

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    Ok((host, backend, proxy))
}

/// Bind a TCP listener and convert `AddrInUse` into a typed error that names
/// the env var the operator can set to choose another port. D-04 (port
/// collision = hard error).
async fn bind_or_die(
    addr: String,
    port: u16,
    env_var: &'static str,
) -> Result<tokio::net::TcpListener, VibeKanbanError> {
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                port,
                env_var,
                "{}",
                VibeKanbanError::PortInUse { port, env_var }
            );
            Err(VibeKanbanError::PortInUse { port, env_var })
        }
        Err(e) => Err(VibeKanbanError::Io(e)),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        let exit_code = match run_subcommand(command).await {
            Ok(()) => 0,
            Err(VibeKanbanError::Remote(e)) => {
                eprintln!("{e}");
                e.exit_code()
            }
            Err(other) => {
                eprintln!("{other}");
                1
            }
        };
        std::process::exit(exit_code);
    }
    if let Err(e) = run_server().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run_server() -> Result<(), VibeKanbanError> {
    // Install rustls crypto provider before any TLS operations (Once-gated,
    // idempotent; safe when both server + MCP share the same process). D-09.
    utils::rustls::install_default_provider();

    sentry_utils::init_once(SentrySource::Backend);

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter_string = format!(
        "warn,server={level},services={level},db={level},executors={level},deployment={level},local_deployment={level},utils={level},embedded_ssh={level},desktop_bridge={level},relay_hosts={level},relay_client={level},relay_webrtc={level},codex_core=off",
        level = log_level
    );
    let env_filter = EnvFilter::try_new(filter_string).expect("Failed to create tracing filter");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .with(sentry_layer())
        .init();

    let shutdown_token = CancellationToken::new();

    // Single startup orchestrator — drives Phase::* (visible via /api/health),
    // validates assets, runs DB copy + migrations, builds the deployment, and
    // pre-warms caches. Shared with the Tauri / test seam in startup.rs.
    let deployment = server::startup::initialize_deployment(shutdown_token.clone()).await?;

    let (host, port, proxy_port) = resolve_ports()?;
    tracing::info!(
        host = %host,
        backend_port = port,
        proxy_port,
        "Resolved listener config"
    );

    let main_listener = bind_or_die(format!("{host}:{port}"), port, "BACKEND_PORT").await?;
    let actual_main_port = main_listener.local_addr()?.port();

    // PREVIEW_PROXY_PORT=0 (dev default) means OS-assign — AddrInUse cannot fire
    // there. The typed error only triggers for an explicit, taken port (prod).
    let proxy_listener = bind_or_die(
        format!("{host}:{proxy_port}"),
        proxy_port,
        "PREVIEW_PROXY_PORT",
    )
    .await?;
    let actual_proxy_port = proxy_listener.local_addr()?.port();

    // D-03 / VKSTART-03: prod no longer writes the port file (race vector for MCP
    // and other consumers). Dev keeps it for `scripts/setup-dev-environment.js`
    // back-compat and the MCP dev-mode fallback chain
    // (see crates/mcp/src/bin/vibe_kanban_mcp.rs::resolve_base_url).
    if cfg!(debug_assertions)
        && let Err(e) = write_port_file_with_proxy(actual_main_port, Some(actual_proxy_port)).await
    {
        tracing::warn!("Failed to write port file: {}", e);
    }

    tracing::info!(
        "Main server on :{}, Preview proxy on :{}",
        actual_main_port,
        actual_proxy_port
    );

    if let Err(_e) = deployment
        .client_info()
        .set_server_addr(main_listener.local_addr()?)
    {
        tracing::warn!(
            "client_info.set_server_addr called twice in the same process; ignoring (benign during re-init)"
        );
    }
    if let Err(_e) = deployment
        .client_info()
        .set_preview_proxy_port(actual_proxy_port)
    {
        tracing::warn!(
            "client_info.set_preview_proxy_port called twice in the same process; ignoring (benign during re-init)"
        );
    }

    let app_router = routes::router(deployment.clone());

    // Production only: open browser
    if !cfg!(debug_assertions) {
        tracing::info!("Opening browser...");
        let browser_port = actual_main_port;
        tokio::spawn(async move {
            if let Err(e) =
                utils::browser::open_browser(&format!("http://127.0.0.1:{browser_port}")).await
            {
                tracing::warn!(
                    "Failed to open browser automatically: {}. Please open http://127.0.0.1:{} manually.",
                    e,
                    browser_port
                );
            }
        });
    }

    let proxy_router: Router = routes::preview::subdomain_router(deployment.clone())
        .layer(ValidateRequestHeaderLayer::custom(validate_origin));

    let main_shutdown = shutdown_token.clone();
    let proxy_shutdown = shutdown_token.clone();

    let main_server = axum::serve(main_listener, app_router)
        .with_graceful_shutdown(async move { main_shutdown.cancelled().await });
    let proxy_server = axum::serve(proxy_listener, proxy_router)
        .with_graceful_shutdown(async move { proxy_shutdown.cancelled().await });

    let main_handle = tokio::spawn(async move {
        if let Err(e) = main_server.await {
            tracing::error!("Main server error: {}", e);
        }
    });
    let proxy_handle = tokio::spawn(async move {
        if let Err(e) = proxy_server.await {
            tracing::error!("Preview proxy error: {}", e);
        }
    });

    relay_registration::spawn_relay(&deployment).await;

    tokio::select! {
        _ = shutdown_signal() => {
            tracing::info!("Shutdown signal received");
        }
        _ = main_handle => {}
        _ = proxy_handle => {}
    }

    shutdown_token.cancel();

    perform_cleanup_actions(&deployment).await;

    Ok(())
}

pub async fn shutdown_signal() {
    // Always wait for Ctrl+C
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {e}");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        // Try to install SIGTERM handler, but don't panic if it fails
        let terminate = async {
            if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                sigterm.recv().await;
            } else {
                tracing::error!("Failed to install SIGTERM handler");
                // Fallback: never resolves
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    {
        // Only ctrl_c is available, so just await it
        ctrl_c.await;
    }
}

pub async fn perform_cleanup_actions(deployment: &DeploymentImpl) {
    deployment
        .container()
        .kill_all_running_processes()
        .await
        .expect("Failed to cleanly kill running execution processes");
}
