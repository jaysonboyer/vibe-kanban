# External Integrations

**Analysis Date:** 2026-05-16

## APIs & External Services

**AI/Coding Agent Platforms:**
- OpenAI Codex - Code completion and analysis via `codex-protocol` (git tag rust-v0.124.0)
  - SDK/Client: `codex-protocol` crate, imported in `crates/executors`
  - Executor: Configurable via `/api/agents/preset-options` endpoint
  - Integration: MCP server mode allows Claude/other agents to invoke vibe-kanban
  
- Anthropic Claude - Supported as MCP client via `agent-client-protocol` (v0.8 with unstable features)
  - SDK/Client: `agent-client-protocol` in `crates/executors`
  - MCP Mode: `npx vibe-kanban mcp` or via `--mcp` flag
  - Executor discovery: `/api/agents/discovered-options/ws` WebSocket for live agent capabilities

**Model Context Protocol (MCP):**
- Server: vibe-kanban exposes MCP server via `crates/mcp` → `vibe-kanban-mcp` binary (distributed via npx)
- Implementation: `rmcp` v1.2.0 with server + IO transport features
- Entry point: `npx-cli/src/cli.ts` handles `mcp` subcommand, spawns binary with args
- Agent tool schemas: Generated in `shared/schemas/` from Rust types
- Configuration: `/api/mcp-config` endpoint allows dynamic tool server setup (e.g., Anthropic Resources for file access)

**GitHub:**
- Release checking: Queries `https://api.github.com/repos/BloopAI/vibe-kanban/releases` for latest version
- OAuth integration: Supported in remote/cloud deployment (GitHub provider configured in `crates/remote/auth/provider.rs`)
- Git operations: Full Git integration via `crates/git` (libgit2 v0.20.3 with rustls TLS)

**Google:**
- OAuth provider: Configured in remote deployment for user authentication
- Implementation: Custom OAuth flow in `crates/remote/auth/provider.rs`

**Sentry (Error Tracking):**
- Backend: Integrated via `sentry` crate v0.46.2 (features: anyhow, backtrace, panic, debug-images, reqwest, rustls)
- Initialization: `utils::sentry::init_once(SentrySource::Backend)` in `crates/server/src/main.rs`
- DSN sources:
  - Local: `SENTRY_DSN` or `SENTRY_DSN_BACKEND` env var
  - Remote: `SENTRY_DSN_REMOTE` env var (crates/remote)
- Scope updates: Deployment info (workspace, user, session) written to Sentry scope
- Release tracking: Built-in panic handler, backtrace capture

## Data Storage

**Local Deployment:**
- Database: SQLite 3 with preupdate hook support
  - File: `~/.local/share/vibe-kanban/db.v2.sqlite` (Linux/macOS) or `%APPDATA%\vibe-kanban\db.v2.sqlite` (Windows)
  - Legacy migration: Old `db.sqlite` automatically copied to `db.v2.sqlite` on startup if new doesn't exist (safe downgrade support)
  - Client: SQLx 0.8.6 with compile-time query checks
  - Features: Runtime-tokio, TLS-rustls-aws-lc-rs, chrono, uuid support
  - Migrations: Managed via SQLx, embedded in binary (`crates/db/migrations/`)

**Remote Deployment:**
- Database: PostgreSQL 16+ with logical replication (`wal_level=logical`)
  - Connection: Via `sqlx` with Postgres driver (0.8.6)
  - Pool: 10 max connections
  - Auth: Env var `POSTGRES_PASSWORD` / connection string configured in `crates/remote/src/config.rs`
  - Migrations: SQLx-managed in `crates/remote/migrations/` (timestamp-prefixed)
  - ElectricSQL role: Auto-created by migrations with `electric_sync` user for logical replication
  - Offline mode: SQLx query metadata pre-computed via `pnpm run remote:prepare-db` for CI builds

**File Storage:**

*Local:*
- Workspace repositories: Stored in user's asset directory (`~/.local/share/vibe-kanban/repos/` or Windows equivalent)
- Assets/sounds: `assets/` directory in distribution

*Remote (Cloud):*
- Azure Blob Storage or AWS S3 (optional, for issue attachments)
- Azure connection: `AZURE_STORAGE_ACCOUNT_NAME`, `AZURE_STORAGE_ACCOUNT_KEY` env vars (crates/remote)
- S3 connection: AWS SDK with credential auto-discovery
- Azurite (dev): Local Azure Blob Storage mock in `docker-compose.yml` (port 10000)

**Caching:**
- Query result cache: moka LRU cache (0.12) in services for executor discovery
- Global executor options: Pre-loaded on startup via `preload_global_executor_options_cache()`
- Binary cache: NPX CLI downloads cached to `~/.cache/vibe-kanban/{BINARY_TAG}/{platform}/` or `dist/` in local dev mode

## Authentication & Identity

**Local Desktop:**
- Custom handoff auth: OAuth flows managed via `/api/auth/handoff/*` endpoints
- Session storage: JWT tokens stored in browser localStorage (local-web frontend)
- Token refresh: `/api/auth/token` endpoint

**Remote/Cloud:**
- Provider: GitHub OAuth or Google OAuth (at least one must be configured)
- JWT Signing: Signed with `VIBEKANBAN_REMOTE_JWT_SECRET` env var
- Middleware: `require_session` on all protected routes in `crates/remote`
- PKCE Flow: OAuth2 with PKCE in frontend (`packages/remote-web/src/auth/pkce.ts`)
- Session validation: Via `RequestContext` middleware extracts user info

**MCP Server Auth:**
- No auth for local MCP usage (server runs in user's session)
- Transport: stdio or SSE (sse-transport not included, but io-transport via rmcp)

## Monitoring & Observability

**Error Tracking:**
- Sentry v0.46.2 - All panics auto-captured
- Source: SENTRY_DSN env var (separate for local and remote via SENTRY_DSN_REMOTE)
- Release info: Sent on startup (version, commit, environment)

**Logging:**
- Framework: `tracing` + `tracing-subscriber` (structured logging)
- Filter: Per-crate log levels controllable via `RUST_LOG` env var
  - Default: `warn,server={level},services={level},db={level},executors={level},deployment={level},local_deployment={level},utils={level},embedded_ssh={level},desktop_bridge={level},relay_hosts={level},relay_client={level},relay_webrtc={level},codex_core=off`
- Format: JSON in production (via `tracing-json`), human-readable in dev
- Middleware: Tower-HTTP request tracing (request IDs via `ValidateRequestHeaderLayer`)

**Analytics (Optional):**
- PostHog: Optional analytics platform
  - DSN: `POSTHOG_API_KEY` and `POSTHOG_API_ENDPOINT` env vars (remote deployment)
  - Client-side: Frontend tracks events via PostHog SDK
  - Server-side: Tracks session start and user-allowed events
- User opt-in: `analytics_enabled` config in `/api/config` endpoint (user can disable)

**OpenTelemetry (Remote Only):**
- Dependencies: `opentelemetry`, `opentelemetry_sdk`, `tracing-opentelemetry`
- Azure Application Insights: `opentelemetry-application-insights` (v0.44) for remote monitoring
- HTTP propagation: `opentelemetry-http` with reqwest bindings

## CI/CD & Deployment

**Hosting:**
- NPX distribution: Published to npm registry, cached by npx (Node package manager)
- Binaries: Hosted on R2 CDN (Cloudflare), downloaded on-demand by CLI
- Docker: Multi-stage Dockerfile for remote deployment (published to Docker Hub or private registry)
- Self-hosted: Compose file provided (`crates/remote/docker-compose.yml`) for Postgres + server + ElectricSQL setup

**CI Pipeline:**
- GitHub Actions (inferred from release checking to GitHub API)
- Pre-PR checks: Type checks (`pnpm run check`), linting (`pnpm run lint`), tests (`cargo test --workspace`)
- Type generation verification: `pnpm run generate-types:check` and `pnpm run remote:generate-types:check` in CI

**Build Outputs:**
- Local NPX binary: `npx-cli/dist/` → published to npm as `vibe-kanban`
- Backend binaries: `target/release/{server,vibe-kanban-mcp,vibe-kanban-review}` (built by cargo)
- Frontend bundle: `packages/local-web/dist/` (embedded in server binary via rust-embed)
- Remote frontend: `packages/remote-web/dist/` (served from Docker `/srv/static`)

## Webhooks & Callbacks

**Incoming:**
- OAuth callback: `/api/auth/handoff/complete` query-based callback (not webhook, standard OAuth)
- Preview proxy: Captures subdomain requests for code preview (reverse proxy on dynamic port)

**Outgoing:**
- GitHub Release API: Passive polling only (no webhooks)
- None detected for outgoing event notifications

## Real-Time Sync (Remote Only)

**ElectricSQL Integration:**
- Framework: ElectricSQL (official open-source, runs as separate service)
- Connection: Remote server subscribes to Postgres logical replication via ElectricSQL
- Protocol: HTTP-based shape subscriptions (read-only sync from server to clients)
- Proxy: Auth-gated proxy at `/shape/*` in remote server validates org/project membership before forwarding to ElectricSQL
- Mutation sync: Writes go through REST API endpoints, return `MutationResponse<T>` with Postgres `txid` for frontend optimistic UI
- Frontend client: `@tanstack/electric-db-collection` binds Electric shapes to React state

## Relay & Network Tunneling

**Relay Protocol (Custom):**
- Components: `crates/relay-protocol`, `crates/relay-ws`, `crates/relay-client`, `crates/relay-webrtc`, `crates/relay-tunnel-core`
- Purpose: P2P communication between desktop app and remote agents/servers
- WebSocket bridge: `crates/ws-bridge` bridges WebSocket connections to relay
- WebRTC support: `relay-webrtc` for direct peer connections (fallback to relay if direct fails)
- Registration: Spawned via `runtime::relay_registration::spawn_relay()` in main server
- Auth: SPAKE2 key exchange (v0.5.0-pre.0) for relay authentication

## Environment Configuration

**Required env vars (Production):**
- `BACKEND_PORT` or `PORT` - Server bind port (default 3000)
- `HOST` - Bind address (default 127.0.0.1, use 0.0.0.0 for Docker/public)
- `RUST_LOG` - Log level (default "info")
- `VK_ALLOWED_ORIGINS` - CORS origins (e.g., "http://localhost:3000")

**Optional env vars:**
- `SENTRY_DSN` or `SENTRY_DSN_BACKEND` - Error tracking DSN
- `VITE_VK_SHARED_API_BASE` - Frontend remote server URL (connects to cloud if set)
- `PREVIEW_PROXY_PORT` - Port for subdomain proxy (auto-assigned in dev)

**Remote-specific env vars:**
- `VIBEKANBAN_REMOTE_JWT_SECRET` - Secret key for JWT signing
- `DATABASE_URL` - PostgreSQL connection string (or individual POSTGRES_* vars)
- `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` - Postgres credentials
- `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` - GitHub OAuth app credentials
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` - Google OAuth app credentials
- `AZURE_STORAGE_ACCOUNT_NAME`, `AZURE_STORAGE_ACCOUNT_KEY` - Azure Blob Storage (optional)
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` - AWS S3 (optional)
- `POSTHOG_API_KEY`, `POSTHOG_API_ENDPOINT` - PostHog analytics (optional)
- `SENTRY_DSN_REMOTE` - Remote error tracking DSN
- `VITE_APP_BASE_URL`, `VITE_API_BASE_URL` - Frontend build-time config (baked into JS bundle)

**Secrets location:**
- Dev: `.env` file (gitignored, never committed)
- Production: Environment variables passed to container or process
- Never embed in code or commit history

## Platform-Specific Builds

**Windows:**
- Executable: vibe-kanban.exe (bundled as Windows binary distribution)
- Database: %APPDATA%\vibe-kanban\db.v2.sqlite
- Native TLS: Via rustls + aws-lc-rs (no OpenSSL dependency)
- ARM64 support: Detected via PROCESSOR_ARCHITEW6432 env var

**macOS:**
- Intel (x64): Native binary
- Apple Silicon (ARM64): Native binary or via Rosetta (detected via sysctl sysctl.proc_translated)
- Database: ~/.local/share/vibe-kanban/db.v2.sqlite (follows Linux XDG spec)
- Browser: Automatic open via system open command

**Linux (x64, ARM64):**
- Database: ~/.local/share/vibe-kanban/db.v2.sqlite (XDG_DATA_HOME)
- OpenSSL: Vendored (aws-lc-rs avoids native OpenSSL; vendored openssl for musl cross-compile only)
- Browser: Automatic open via xdg-open command

---

*Integration audit: 2026-05-16*
