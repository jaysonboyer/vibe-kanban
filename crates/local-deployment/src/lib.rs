use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use api_types::LoginStatus;
use async_trait::async_trait;
use client_info::ClientInfo;
use db::DBService;
use deployment::{Deployment, DeploymentError, RelayHostsNotConfigured, RemoteClientNotConfigured};
use executors::profile::ExecutorConfigs;
use git::GitService;
use preview_proxy::PreviewProxyService;
use relay_control::{RelayControl, signing::RelaySigningService};
use relay_hosts::RelayHosts;
use relay_webrtc::WebRtcHost;
use remote_info::RemoteInfo;
use scopeguard::{ScopeGuard, guard};
use services::services::{
    analytics::{AnalyticsConfig, AnalyticsContext, AnalyticsService, generate_user_id},
    approvals::Approvals,
    auth::AuthContext,
    config::{Config, load_config_from_file, save_config_to_file},
    container::ContainerService,
    events::EventService,
    file::FileService,
    file_search::FileSearchCache,
    filesystem::FilesystemService,
    oauth_credentials::OAuthCredentials,
    pr_monitor::PrMonitorService,
    queued_message::QueuedMessageService,
    remote_client::{RemoteClient, RemoteClientError},
    repo::RepoService,
};
use tokio::sync::{Notify, RwLock};
use tokio_util::sync::CancellationToken;
use trusted_key_auth::runtime::TrustedKeyAuthRuntime;
use utils::{
    assets::{config_path, credentials_path, server_signing_key_path, trusted_keys_path},
    msg_store::MsgStore,
};
use uuid::Uuid;
use workspace_manager::WorkspaceManager;
use worktree_manager::WorktreeManager;

use crate::{container::LocalContainerService, pty::PtyService};
mod command;
pub mod container;
mod copy;
pub mod pty;

#[derive(Clone)]
pub struct LocalDeployment {
    config: Arc<RwLock<Config>>,
    user_id: String,
    db: DBService,
    workspace_manager: WorkspaceManager,
    analytics: Option<AnalyticsService>,
    container: LocalContainerService,
    git: GitService,
    repo: RepoService,
    file: FileService,
    filesystem: FilesystemService,
    events: EventService,
    file_search_cache: Arc<FileSearchCache>,
    approvals: Approvals,
    queued_message_service: QueuedMessageService,
    remote_client: Result<RemoteClient, RemoteClientNotConfigured>,
    auth_context: AuthContext,
    oauth_handoffs: Arc<RwLock<HashMap<Uuid, PendingHandoff>>>,
    trusted_key_auth: TrustedKeyAuthRuntime,
    relay_signing: RelaySigningService,
    relay_control: Arc<RelayControl>,
    client_info: ClientInfo,
    remote_info: RemoteInfo,
    preview_proxy: PreviewProxyService,
    relay_hosts: Option<Arc<RelayHosts>>,
    shutdown: CancellationToken,
    webrtc_host: OnceLock<Arc<WebRtcHost>>,
    ssh_config: Arc<russh::server::Config>,
    pty: PtyService,
    pr_sync_notify: Arc<Notify>,
}

#[derive(Debug, Clone)]
struct PendingHandoff {
    provider: String,
    app_verifier: String,
}

#[async_trait]
impl Deployment for LocalDeployment {
    async fn new(shutdown: CancellationToken) -> Result<Self, DeploymentError> {
        // Run one-time process logs migration from DB to filesystem
        services::services::execution_process::migrate_execution_logs_to_files()
            .await
            .map_err(|e| DeploymentError::Other(anyhow::anyhow!("Migration failed: {}", e)))?;

        let mut raw_config = load_config_from_file(&config_path()).await;

        let profiles = ExecutorConfigs::get_cached();
        if !raw_config.onboarding_acknowledged
            && let Ok(recommended_executor) = profiles.get_recommended_executor_profile().await
        {
            raw_config.executor_profile = recommended_executor;
        }

        // Check if app version has changed and set release notes flag
        {
            let current_version = utils::version::APP_VERSION;
            let stored_version = raw_config.last_app_version.as_deref();

            if stored_version != Some(current_version) {
                // Show release notes only if this is an upgrade (not first install)
                raw_config.show_release_notes = stored_version.is_some();
                raw_config.last_app_version = Some(current_version.to_string());
            }
        }

        // Always save config (may have been migrated or version updated)
        save_config_to_file(&raw_config, &config_path()).await?;

        if let Some(workspace_dir) = &raw_config.workspace_dir {
            let path = utils::path::expand_tilde(workspace_dir);
            WorktreeManager::set_workspace_dir_override(path);
        }

        let config = Arc::new(RwLock::new(raw_config));
        let user_id = generate_user_id();
        let analytics = AnalyticsConfig::new().map(AnalyticsService::new);
        let git = GitService::new();
        let repo = RepoService::new();
        let msg_stores = Arc::new(RwLock::new(HashMap::new()));
        let filesystem = FilesystemService::new();

        // Create shared components for EventService
        let events_msg_store = Arc::new(MsgStore::new());
        let events_entry_count = Arc::new(RwLock::new(0));

        // Create DB with event hooks.
        // TODO(VKSTART-11): the event hook fires on every DB connection, which
        // is observably hot under load. Cache hook state at the pool level
        // (or move to per-statement instrumentation) in a follow-up phase.
        let db = {
            let hook = EventService::create_hook(
                events_msg_store.clone(),
                events_entry_count.clone(),
                DBService::new().await?, // Temporary DB service for the hook
            );
            DBService::new_with_after_connect(hook).await?
        };

        // VKTEST-02b injection seam — forces a deterministic mid-init Err so
        // the plan 01-09 integration test can exercise the scopeguard cleanup
        // path. Gated behind #[cfg(any(test, feature = "test-fault-injection"))]
        // so this code path is removed entirely from release builds.
        #[cfg(any(test, feature = "test-fault-injection"))]
        if std::env::var("VK_TEST_FAIL_AFTER_DB").is_ok() {
            tracing::warn!(
                "VK_TEST_FAIL_AFTER_DB set; forcing LocalDeployment::new Err for partial-init test"
            );
            return Err(DeploymentError::Other(anyhow::anyhow!(
                "VK_TEST_FAIL_AFTER_DB injection fault"
            )));
        }

        // VKSTART-07: wrap the DB pool in a scopeguard so any `?`-driven early
        // return from this constructor closes the pool. Without this, the
        // sqlite file lock leaks across re-init attempts (Tauri reopen,
        // integration tests). Dismissed with `ScopeGuard::into_inner` at the
        // end of `new()`.
        let db_guard = guard(db.clone(), |db| {
            tracing::warn!("LocalDeployment::new failed after DB acquired; closing pool");
            let pool = db.pool.clone();
            tokio::spawn(async move { pool.close().await });
        });

        let file = FileService::new(db.clone().pool)?;
        // VKSTART-10: gate the orphan-file cleanup behind a process-wide
        // OnceLock so re-initialization (Tauri reopen, integration tests)
        // does NOT spawn a second concurrent cleanup task.
        static CLEANUP_STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        if CLEANUP_STARTED.set(()).is_ok() {
            let file_service = file.clone();
            tokio::spawn(async move {
                tracing::info!("Starting orphaned file cleanup (once per process)...");
                if let Err(e) = file_service.delete_orphaned_files().await {
                    tracing::error!("Failed to clean up orphaned files: {}", e);
                }
            });
        } else {
            tracing::debug!(
                "Orphaned file cleanup already started by an earlier init in this process; skipping"
            );
        }

        let approvals = Approvals::new();
        let queued_message_service = QueuedMessageService::new();

        let oauth_credentials = Arc::new(OAuthCredentials::new(credentials_path()));
        if let Err(e) = oauth_credentials.load().await {
            tracing::warn!(?e, "failed to load OAuth credentials");
        }

        let profile_cache = Arc::new(RwLock::new(None));
        let auth_context = AuthContext::new(oauth_credentials.clone(), profile_cache.clone());

        let api_base = std::env::var("VK_SHARED_API_BASE")
            .ok()
            .or_else(|| option_env!("VK_SHARED_API_BASE").map(|s| s.to_string()));
        let relay_api_base = std::env::var("VK_SHARED_RELAY_API_BASE")
            .ok()
            .or_else(|| option_env!("VK_SHARED_RELAY_API_BASE").map(|s| s.to_string()));
        let remote_info = RemoteInfo::new();
        if let Some(api_base) = api_base.clone() {
            remote_info
                .set_api_base(api_base)
                .expect("api_base already set");
        }
        if let Some(relay_api_base) = relay_api_base {
            remote_info
                .set_relay_api_base(relay_api_base)
                .expect("relay_api_base already set");
        }

        let remote_client = match remote_info.get_api_base() {
            Some(url) => match RemoteClient::new(&url, auth_context.clone()) {
                Ok(client) => {
                    tracing::info!("Remote client initialized with URL: {}", url);
                    Ok(client)
                }
                Err(e) => {
                    tracing::error!(?e, "failed to create remote client");
                    Err(RemoteClientNotConfigured)
                }
            },
            None => {
                tracing::info!("VK_SHARED_API_BASE not set; remote features disabled");
                Err(RemoteClientNotConfigured)
            }
        };

        let oauth_handoffs = Arc::new(RwLock::new(HashMap::new()));
        let trusted_key_auth = TrustedKeyAuthRuntime::new(trusted_keys_path());
        let relay_signing = RelaySigningService::load_or_generate(&server_signing_key_path())
            .map_err(|e| {
                DeploymentError::Other(anyhow::anyhow!(
                    "Failed to load or generate server signing key: {}",
                    e
                ))
            })?;
        let relay_control = Arc::new(RelayControl::new());
        let client_info = ClientInfo::new();
        let preview_proxy = PreviewProxyService::new();

        let ssh_config = embedded_ssh::config::build_config(relay_signing.signing_key());

        // We need to make analytics accessible to the ContainerService
        // TODO: Handle this more gracefully
        let analytics_ctx = analytics.as_ref().map(|s| AnalyticsContext {
            user_id: user_id.clone(),
            analytics_service: s.clone(),
        });
        let workspace_manager = WorkspaceManager::new(db.clone());
        let container = LocalContainerService::new(
            db.clone(),
            workspace_manager.clone(),
            msg_stores.clone(),
            config.clone(),
            git.clone(),
            file.clone(),
            analytics_ctx,
            approvals.clone(),
            queued_message_service.clone(),
            remote_client.clone().ok(),
        )
        .await;

        let events = EventService::new(db.clone(), events_msg_store, events_entry_count);

        let file_search_cache = Arc::new(FileSearchCache::new());

        let pty = PtyService::new();
        let relay_hosts = match remote_client.clone().ok() {
            Some(remote_client) => Some(Arc::new(
                RelayHosts::load(
                    remote_client,
                    remote_info.clone(),
                    relay_signing.clone(),
                    shutdown.child_token(),
                )
                .await,
            )),
            None => None,
        };
        let pr_sync_notify = Arc::new(Notify::new());
        {
            let db = db.clone();
            let analytics = analytics.as_ref().map(|s| AnalyticsContext {
                user_id: user_id.clone(),
                analytics_service: s.clone(),
            });
            let container = container.clone();
            let rc = remote_client.clone().ok();
            PrMonitorService::spawn(db, analytics, container, rc, pr_sync_notify.clone()).await;
        }
        // Observability-only guard: PrMonitorService listens on the shutdown
        // CancellationToken, so dropping it on early-return causes the task to
        // exit naturally. This guard exists to log the failure path and as a
        // hook for future explicit-handle cleanup if PrMonitorService later
        // returns one.
        let pr_monitor_guard = guard((), |_| {
            tracing::warn!(
                "LocalDeployment::new failed after pr_monitor spawned; shutdown token drop will stop the task"
            );
        });

        let deployment = Self {
            config,
            user_id,
            db,
            workspace_manager,
            analytics,
            container,
            git,
            repo,
            file,
            filesystem,
            events,
            file_search_cache,
            approvals,
            queued_message_service,
            remote_client,
            auth_context,
            oauth_handoffs,
            trusted_key_auth,
            relay_signing,
            relay_control,
            client_info,
            remote_info,
            preview_proxy,
            relay_hosts,
            shutdown,
            webrtc_host: OnceLock::new(),
            ssh_config,
            pty,
            pr_sync_notify,
        };

        // All init steps succeeded — dismiss the scopeguards so their cleanup
        // does NOT run on the success path.
        let _db = ScopeGuard::into_inner(db_guard);
        let _pr_monitor = ScopeGuard::into_inner(pr_monitor_guard);

        Ok(deployment)
    }

    fn user_id(&self) -> &str {
        &self.user_id
    }

    fn config(&self) -> &Arc<RwLock<Config>> {
        &self.config
    }

    fn db(&self) -> &DBService {
        &self.db
    }

    fn analytics(&self) -> &Option<AnalyticsService> {
        &self.analytics
    }

    fn container(&self) -> &impl ContainerService {
        &self.container
    }

    fn git(&self) -> &GitService {
        &self.git
    }

    fn repo(&self) -> &RepoService {
        &self.repo
    }

    fn file(&self) -> &FileService {
        &self.file
    }

    fn filesystem(&self) -> &FilesystemService {
        &self.filesystem
    }

    fn events(&self) -> &EventService {
        &self.events
    }

    fn file_search_cache(&self) -> &Arc<FileSearchCache> {
        &self.file_search_cache
    }

    fn approvals(&self) -> &Approvals {
        &self.approvals
    }

    fn queued_message_service(&self) -> &QueuedMessageService {
        &self.queued_message_service
    }

    fn auth_context(&self) -> &AuthContext {
        &self.auth_context
    }

    fn relay_control(&self) -> &Arc<RelayControl> {
        &self.relay_control
    }

    fn relay_signing(&self) -> &RelaySigningService {
        &self.relay_signing
    }

    fn client_info(&self) -> &ClientInfo {
        &self.client_info
    }

    fn remote_info(&self) -> &RemoteInfo {
        &self.remote_info
    }

    fn preview_proxy(&self) -> &PreviewProxyService {
        &self.preview_proxy
    }

    fn relay_hosts(&self) -> Result<&Arc<RelayHosts>, RelayHostsNotConfigured> {
        self.relay_hosts.as_ref().ok_or(RelayHostsNotConfigured)
    }

    fn trusted_key_auth(&self) -> &TrustedKeyAuthRuntime {
        &self.trusted_key_auth
    }
}

impl LocalDeployment {
    pub fn webrtc_host(&self) -> Option<Arc<WebRtcHost>> {
        let local_addr = self.client_info.get_server_addr()?;

        Some(
            self.webrtc_host
                .get_or_init(|| Arc::new(WebRtcHost::new(local_addr, self.shutdown.child_token())))
                .clone(),
        )
    }

    pub fn workspace_manager(&self) -> &WorkspaceManager {
        &self.workspace_manager
    }

    pub fn remote_client(&self) -> Result<RemoteClient, RemoteClientNotConfigured> {
        self.remote_client.clone()
    }

    pub async fn get_login_status(&self) -> LoginStatus {
        if self.auth_context.get_credentials().await.is_none() {
            self.auth_context.clear_profile().await;
            self.auth_context.clear_remote_auth_degraded_slug().await;
            return LoginStatus::LoggedOut;
        };

        if let Some(cached_profile) = self.auth_context.cached_profile().await {
            return LoginStatus::LoggedIn {
                profile: Some(cached_profile),
            };
        }

        let Ok(client) = self.remote_client() else {
            return LoginStatus::LoggedOut;
        };

        match client.profile().await {
            Ok(profile) => {
                self.auth_context.clear_remote_auth_degraded_slug().await;
                self.auth_context.set_profile(profile.clone()).await;
                LoginStatus::LoggedIn {
                    profile: Some(profile),
                }
            }
            Err(RemoteClientError::Auth) => {
                let _ = self.auth_context.clear_credentials().await;
                self.auth_context.clear_profile().await;
                self.auth_context.clear_remote_auth_degraded_slug().await;
                LoginStatus::LoggedOut
            }
            Err(err) => {
                if self.auth_context.get_credentials().await.is_none() {
                    self.auth_context.clear_profile().await;
                    self.auth_context.clear_remote_auth_degraded_slug().await;
                    return LoginStatus::LoggedOut;
                }

                self.auth_context
                    .set_remote_auth_degraded_slug(
                        err.degraded_slug()
                            .unwrap_or_else(RemoteClientError::generic_degraded_slug),
                    )
                    .await;
                LoginStatus::LoggedIn { profile: None }
            }
        }
    }

    pub async fn store_oauth_handoff(
        &self,
        handoff_id: Uuid,
        provider: String,
        app_verifier: String,
    ) {
        self.oauth_handoffs.write().await.insert(
            handoff_id,
            PendingHandoff {
                provider,
                app_verifier,
            },
        );
    }

    pub async fn take_oauth_handoff(&self, handoff_id: &Uuid) -> Option<(String, String)> {
        self.oauth_handoffs
            .write()
            .await
            .remove(handoff_id)
            .map(|state| (state.provider, state.app_verifier))
    }

    pub fn pty(&self) -> &PtyService {
        &self.pty
    }

    pub fn ssh_config(&self) -> &Arc<russh::server::Config> {
        &self.ssh_config
    }

    pub fn trigger_pr_sync(&self) {
        self.pr_sync_notify.notify_one();
    }
}
