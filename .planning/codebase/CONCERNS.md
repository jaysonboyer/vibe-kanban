# Codebase Concerns

**Analysis Date:** 2026-05-16

## Tech Debt

**Port File Location and Timing Race Condition:**
- Issue: Port file written at `temp_dir()/vibe-kanban/vibe-kanban.port` after server binds, but no guarantee file persists before external consumers (e.g., developer-sandbox, MCP server) try to read it. Race window exists between port binding and file write completion.
- Files: `crates/server/src/main.rs` (line 123), `crates/utils/src/port_file.rs` (line 13-28), `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 100-134)
- Impact: MCP server startup may fail if it attempts to read port file before it's written (see MCP fallback in `vibe_kanban_mcp.rs` line 125 which reads port file when env vars not set)
- Fix approach: Add retry logic with exponential backoff in MCP server port file read, OR pre-allocate and write port file before binding listeners, OR use named pipe/socket for guaranteed atomicity

**Unhandled `expect()` on Port Setting:**
- Issue: Lines 136, 140 in `crates/server/src/main.rs` use `.expect("already set")`, assuming `client_info` state is always fresh. If code re-initializes without proper cleanup, these panic.
- Files: `crates/server/src/main.rs` (line 136, 140)
- Impact: Process crashes with panic instead of graceful error if called multiple times
- Fix approach: Change to `.ok()` or use result type; document that Deployment::new() must be called exactly once

**Database Migration Checksum Workaround on Windows:**
- Issue: Migration checksum mismatch handling (line 42-63 in `crates/db/src/lib.rs`) silently updates stored checksums on Windows platform, masking real migration issues. This is a platform-specific workaround that could hide data migration errors.
- Files: `crates/db/src/lib.rs` (line 42-63)
- Impact: Silent data corruption risk if migration content differs but checksums are recalculated to match
- Fix approach: Log migration content diff before updating checksum; add CI test that validates migration checksums across platforms

**Asset Directory Creation on Every Startup:**
- Issue: `asset_dir()` in `crates/utils/src/assets.rs` calls `create_dir_all()` inside the accessor function (line 14-15), and similarly in `crates/server/src/main.rs` (line 53-55). This creates directories on every access, adding I/O overhead.
- Files: `crates/utils/src/assets.rs` (line 6-22), `crates/server/src/main.rs` (line 52-55)
- Impact: Unnecessary filesystem syscalls; slower startup
- Fix approach: Create asset directories once during initialization, not on every getter call

**Orphaned File Cleanup as Fire-and-Forget Task:**
- Issue: Line 152-157 in `crates/local-deployment/src/lib.rs` spawns file cleanup as untracked background task without waiting for completion. If cleanup fails, error is only logged.
- Files: `crates/local-deployment/src/lib.rs` (line 152-157)
- Impact: Orphaned files may accumulate; cleanup failure goes unnoticed in production
- Fix approach: Either wait for cleanup before considering startup complete, or expose cleanup status in health checks

## Known Bugs

**Port Parsing Loses Type Safety:**
- Symptoms: ANSI escape code stripping required before parsing port (line 100-103 in `crates/server/src/main.rs`); suggests upstream services may be passing color-coded output
- Files: `crates/server/src/main.rs` (line 100-103)
- Trigger: Set `BACKEND_PORT` with ANSI codes (e.g., from colorized shell output)
- Workaround: Ensure port env vars are passed without shell color formatting

**Rustls Default Provider Installation Happens Twice:**
- Symptoms: Same `rustls::crypto::aws_lc_rs::default_provider().install_default()` called in both `crates/server/src/main.rs` (line 35) and `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 137-139)
- Files: `crates/server/src/main.rs` (line 35), `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 137-139)
- Trigger: Starting both main server and MCP server in same process
- Workaround: Install once in shared utils crate initialization

## Security Considerations

**Port Binding Hardcoded to Localhost:**
- Risk: Server binds to `127.0.0.1` by default (line 115 in `crates/server/src/main.rs`), making it inaccessible from remote machines unless HOST env var is overridden. No validation that HOST is safe (could be `0.0.0.0`).
- Files: `crates/server/src/main.rs` (line 115), `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 110-112)
- Current mitigation: Default to 127.0.0.1; users must explicitly set HOST to expose externally
- Recommendations: (1) Validate HOST against allowlist; (2) Document security implications; (3) Add warning log when binding to non-localhost

**Credentials and Keys Stored in Asset Directory:**
- Risk: OAuth credentials, signing keys, and trusted key files all stored in asset directory with default OS permissions. No explicit mention of file permission enforcement.
- Files: `crates/utils/src/assets.rs` (line 40-51)
- Current mitigation: Relies on OS user isolation
- Recommendations: (1) Explicitly set file permissions to 0600 on credentials files; (2) Verify no credentials logged; (3) Add permission validation on startup

**Environment Variable Exposure in Logs:**
- Risk: Server initialization logs port, but PORT env var might be logged before ANSI stripping. MCP server logs all environment variable sources (line 119 in vibe_kanban_mcp.rs).
- Files: `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 119)
- Current mitigation: Logs port after parsing, not raw env var
- Recommendations: Audit all tracing statements to ensure no secrets are logged

## Performance Bottlenecks

**EventService Hook Runs on Every Database Connection:**
- Problem: `LocalDeployment::new()` creates a `DBService` with event hooks (line 140-147 in `crates/local-deployment/src/lib.rs`). The hook is called on every connection in the pool, adding latency to connection acquisition.
- Files: `crates/local-deployment/src/lib.rs` (line 140-147)
- Cause: No pooling of hook results; each connection re-invokes hook logic
- Improvement path: Cache hook state at pool level; only re-initialize if stale

**File Cleanup Spawned Without Concurrency Control:**
- Problem: Multiple startup calls could spawn multiple concurrent cleanup tasks (line 152 in `crates/local-deployment/src/lib.rs`). No semaphore or lock prevents race.
- Files: `crates/local-deployment/src/lib.rs` (line 152-157)
- Cause: Fire-and-forget task spawn with no coordination
- Improvement path: Use `OnceLock` or flag to ensure cleanup runs only once per instance lifetime

**Database Migration Runs Synchronously on Startup:**
- Problem: `run_migrations()` blocks main thread during startup (line 12-68 in `crates/db/src/lib.rs`). Large schema changes could delay server startup by seconds.
- Files: `crates/db/src/lib.rs` (line 85, 151)
- Cause: SQLx migrator is synchronous; no async batching or streaming
- Improvement path: Profile migration time; consider lazy migration if acceptable

## Fragile Areas

**MCP Server Port Resolution Fallback Chain:**
- Files: `crates/mcp/src/bin/vibe_kanban_mcp.rs` (line 100-134)
- Why fragile: (1) Tries `VIBE_BACKEND_URL` env first, then `MCP_PORT`/`PORT`, then reads port file. Fallback to file read happens without timeout, blocking forever if file missing. (2) No validation that resolved URL is actually running. (3) If main server hasn't written port file yet, MCP startup silently fails with "file not found" error.
- Safe modification: (1) Add timeout to port file read with clear error message; (2) Validate backend URL with health check; (3) Document expected env var precedence
- Test coverage: No tests visible for port resolution fallback; MCP server integration tests missing

**Deployment Initialization Order:**
- Files: `crates/local-deployment/src/lib.rs` (line 92-299), `crates/server/src/main.rs` (line 33-82)
- Why fragile: Deployment creation involves 15+ service initializations in strict sequence. Each one must succeed for next to execute. No checkpoint/resume logic. If step 10 fails after step 1 succeeded, no cleanup of prior steps (e.g., file handles, database connections).
- Safe modification: (1) Wrap each service init in a scope that implements Drop for cleanup; (2) Use `ScopeGuard` pattern; (3) Document dependency graph; (4) Add integration test for partial failure scenarios
- Test coverage: Individual service tests exist but no integration test for full deployment startup

**Asset Path Resolution Inconsistency:**
- Files: `crates/utils/src/assets.rs` (line 6-22)
- Why fragile: `asset_dir()` returns different paths based on `cfg!(debug_assertions)` (dev_assets during debug, OS-specific data dir in prod). No test verifies both paths are valid. If dev_assets is deleted during release build, startup fails with cryptic mkdir error.
- Safe modification: (1) Validate asset directory exists at startup with clear error if not; (2) Add test that runs both debug and release builds; (3) Document expected dev vs prod directory structure
- Test coverage: No tests for asset directory resolution

**Proxy Server Binding Doesn't Handle Port 0:**
- Files: `crates/server/src/main.rs` (line 110-121)
- Why fragile: Proxy port defaults to 0 if `PREVIEW_PROXY_PORT` not set, causing OS to auto-assign. If user sets `BACKEND_PORT=3001` expecting deterministic ports, they'll be surprised by random proxy port. This breaks port predictability for external consumers.
- Safe modification: (1) Document that PREVIEW_PROXY_PORT must be set for deterministic startup; (2) Default PREVIEW_PROXY_PORT to a fixed offset from BACKEND_PORT instead of 0; (3) Add validation that ports don't conflict
- Test coverage: No tests for port collision scenarios

## Scaling Limits

**SQLite as Single File Database:**
- Current capacity: SQLite typically handles 10k-100k concurrent reads, but only 1 concurrent writer due to file-level locking
- Limit: Vibe Kanban uses SQLite (line 77-86 in `crates/db/src/lib.rs`). If multiple workspace instances try to write simultaneously (e.g., shared workspace on NAS), database locks will serialize writes and cause stalls.
- Scaling path: (1) Evaluate PostgreSQL migration for multi-writer scenarios; (2) Document SQLite single-writer limitation; (3) Consider moving high-write tables (execution logs) to separate store

**In-Memory Hashmaps for Execution State:**
- Current capacity: Line 72-78 in `crates/local-deployment/src/container.rs` uses `Arc<RwLock<HashMap<Uuid, ...>>>` for child processes, cancellation tokens, and stream handles
- Limit: Memory grows unbounded if execution processes are never removed. No TTL or size limit observed.
- Scaling path: (1) Add automatic cleanup of completed execution handles; (2) Implement LRU eviction; (3) Profile memory usage under 1000+ concurrent executions

**Port File as Coordination Primitive:**
- Current capacity: Single machine only; port file stored in `temp_dir()/vibe-kanban/`
- Limit: If developer-sandbox or other external tool tries to coordinate multiple VK instances, all will write to same port file, overwriting each other
- Scaling path: (1) Use unique port file per instance ID; (2) Add instance tagging; (3) Consider moving to MCP server as single source of truth for port discovery

## Dependencies at Risk

**Sentry Integration Optional but Unvalidated:**
- Risk: Sentry init (line 39 in `crates/server/src/main.rs`) calls `init_once()` which may silently fail if Sentry DSN not configured. No validation that error reporting is working.
- Impact: Production errors may not be reported if Sentry initialization fails
- Migration plan: (1) Add health check for Sentry connectivity; (2) Log warning if DSN not set; (3) Verify error reporting in CI

**ts-rs Custom Fork for TypeScript Generation:**
- Risk: `crates/server/src/bin/generate_types.rs` uses non-standard ts-rs fork (`https://github.com/xazukx/ts-rs.git`, branch `use-ts-enum`)
- Impact: Dependency on fork maintainer; if fork falls behind, miss ts-rs bug fixes
- Migration plan: (1) Contribute enum feature upstream or switch to standard ts-rs version; (2) Pin fork commit hash; (3) Evaluate alternative type-gen libraries (tsync, etc)

**Rustls AWS-LC Pinned to Exact Version:**
- Risk: `aws_lc_rs = "=1.16.0"` is pinned exactly (line 55 in Cargo.toml). Security patches to AWS-LC won't auto-apply.
- Impact: May miss critical crypto fixes
- Migration plan: (1) Switch to `^1.16.0` to allow patch updates; (2) Set up Dependabot to alert on new versions; (3) Review AWS-LC release notes quarterly

## Missing Critical Features

**No Shutdown Coordination Protocol:**
- Problem: `shutdown_signal()` (line 200-234 in `crates/server/src/main.rs`) handles SIGTERM but main/proxy servers are shut down independently via cancellation tokens. If MCP server is running in separate process, it has no signal that main server is shutting down.
- Blocks: External consumers can't reliably wait for graceful shutdown; they need to poll port file or use other heuristics
- Fix: (1) Write shutdown signal to a file or port; (2) Expose graceful shutdown endpoint on API; (3) Document shutdown sequence for multi-process deployments

**No Health Check Endpoint:**
- Problem: External consumers (developer-sandbox, orchestrator) must assume server is healthy once port is available. No readiness probe.
- Blocks: Can't distinguish between "server up but not ready" and "server crashed"
- Fix: (1) Add `/health` endpoint that checks database connectivity, all services initialized; (2) Add `/ready` endpoint that waits for migrations complete; (3) Document expected response codes

**No Configuration Validation at Startup:**
- Problem: `LocalDeployment::new()` loads config from file but doesn't validate required fields (e.g., workspace_dir exists if set, executor profiles valid)
- Blocks: Invalid config silently accepted; errors surface later when features used
- Fix: (1) Run validator pass on config at startup; (2) Fail fast with clear error messages; (3) Add config migration logic for version upgrades

## Test Coverage Gaps

**No Integration Tests for Startup Sequence:**
- Untested area: Full bootstrap path from main() through deployment initialization
- Files: `crates/server/src/main.rs`, `crates/local-deployment/src/lib.rs`
- Risk: Initialization order bugs (e.g., port file written before binding succeeds, or written to wrong location) only discovered in production
- Priority: High

**No Tests for Port File Race Conditions:**
- Untested area: MCP server startup before port file written; concurrent reads of port file
- Files: `crates/mcp/src/bin/vibe_kanban_mcp.rs`, `crates/utils/src/port_file.rs`
- Risk: Rare timing-dependent failures in production that can't be reproduced locally
- Priority: High

**No Tests for Cross-Platform Asset Resolution:**
- Untested area: Asset directory resolution in debug vs release builds; Windows vs Unix paths
- Files: `crates/utils/src/assets.rs`
- Risk: Asset loading fails silently on one platform; unnoticed until users report
- Priority: Medium

**No Tests for Database Migration Failure Scenarios:**
- Untested area: Migration failures (e.g., migration SQL is invalid, database is corrupted)
- Files: `crates/db/src/lib.rs`
- Risk: Startup hangs or crashes with cryptic error if migration fails
- Priority: Medium

**No Tests for Multiple Concurrent Deployments:**
- Untested area: Starting two deployment instances on same machine; port conflicts; database lock contention
- Files: `crates/server/src/main.rs`, `crates/local-deployment/src/lib.rs`
- Risk: Undefined behavior if user tries to run `vibe-kanban` twice simultaneously
- Priority: Medium

---

*Concerns audit: 2026-05-16*
