<!-- refreshed: 2026-05-16 -->
# Architecture

**Analysis Date:** 2026-05-16

## System Overview

Vibe-Kanban is a hybrid local-first + cloud-connected application built on a Rust backend (Tokio + Axum) and TypeScript/React frontend. The system splits into two deployment modes: **Local** (standalone CLI + desktop app) and **Remote** (cloud-hosted with ElectricSQL sync).

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Frontend Layer (TS/React)                     │
├─────────────────────┬──────────────────────┬───────────────────────┤
│  Local Web          │  Remote Web          │  Tauri Desktop App    │
│  (Vite App)         │  (SPA / Static)      │  (Local + Web Bridge) │
│  `/packages/       │  `/packages/        │  `/crates/tauri-app/  │
│  local-web/src`    │  remote-web/src`    │  `                     │
└─────────────────────┴──────────────────────┴───────────────────────┘
         │                    │                        │
         └────────────────────┼────────────────────────┘
                              │
         ┌────────────────────▼────────────────────────┐
         │         Shared Frontend Library             │
         │    (@vibe/web-core) `/packages/web-core/`  │
         │         Shared UI components (@vibe/ui)     │
         └────────────────────┬────────────────────────┘
                              │
         ┌────────────────────▼────────────────────────┐
         │     Generated Types & API Schemas          │
         │      `/shared/types.ts` (from Rust)        │
         │      `/shared/schemas/` (JSON schemas)     │
         └────────────────────┬────────────────────────┘
                              │
┌──────────────────────────────▼──────────────────────────────────────┐
│                         HTTP/WebSocket API                           │
│                    Axum Router on Port :3000                         │
│                     `/api/` namespace routes                         │
└──────────────────────────────┬───────────────────────────────────────┘
                              │
┌──────────────────────────────▼───────────────────────────────────────┐
│                      Rust Backend Services                            │
├──────────────────┬──────────────────┬──────────────────┬─────────────┤
│ Containers       │ Git/Repos        │ Executors        │ Filesystem  │
│ /crates/server/  │ /crates/git/     │ /crates/         │ /crates/    │
│ /local-deploy/   │ /crates/         │ executors/       │ services/   │
│                  │ git-host/        │                  │             │
├──────────────────┼──────────────────┼──────────────────┼─────────────┤
│ Database         │ Relay/Networking │ Authentication   │ Workspace   │
│ /crates/db/      │ /crates/relay-*/ │ /crates/         │ /crates/    │
│ (SQLx + SQLite)  │ /crates/ws-bridge│ trusted-key-auth/│ workspace   │
│                  │                  │ /crates/mcp/     │ -manager/   │
└──────────────────┴──────────────────┴──────────────────┴─────────────┘
         │
┌────────▼──────────────────────────────────────────────────────────────┐
│                      Data Layer & Storage                              │
├──────────────────┬───────────────────────┬─────────────────────────┤
│  SQLite DB       │  File System           │  Relay Tunnels         │
│  `dev_assets/   │  (Workspaces, repos)   │  (Cloud connectivity)  │
│  db.v2.sqlite`  │                        │                         │
└──────────────────┴───────────────────────┴─────────────────────────┘

┌──────────────────────────────────────────────────────────────────────┐
│          REMOTE DEPLOYMENT (Cloud / Docker)                           │
│  `/crates/remote/` — Separate workspace with ElectricSQL sync        │
│  Postgres DB + Live replication to local SQLite                      │
│  Handles billing, multi-tenancy, attachments                         │
└──────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Local Web App** | Browser UI for local development & workspace management | `packages/local-web/src/` |
| **Web Core Library** | Shared React components, hooks, state management | `packages/web-core/src/` |
| **UI Components** | Design system: buttons, forms, modals (Radix + Tailwind) | `packages/ui/` |
| **Server** | Main Axum HTTP + WebSocket server, request routing | `crates/server/src/main.rs` |
| **Local Deployment** | Local-specific initialization: DB, containers, Pty | `crates/local-deployment/src/` |
| **DB Service** | SQLx wrapper, migrations, model definitions | `crates/db/src/` |
| **Services** | Business logic: containers, files, repos, auth, events | `crates/services/src/` |
| **Executors** | Spawning AI agent processes with MCP (Model Context Protocol) | `crates/executors/src/` |
| **Git** | Git operations wrapper (git2) for repo introspection | `crates/git/src/` |
| **Relay Tunnels** | Remote host connectivity, WebRTC, WebSocket bridge | `crates/relay-*/src/` |
| **MCP Server** | Anthropic Model Context Protocol stdio server | `crates/mcp/src/` |
| **Remote** | Cloud deployment with Postgres + ElectricSQL | `crates/remote/src/` |
| **Tauri Desktop** | Native desktop wrapper (Windows, macOS, Linux) | `crates/tauri-app/src/` |

## Pattern Overview

**Overall:** Multi-tier monolithic architecture with Rust backend + TypeScript frontend. Clear separation of concerns: API layer → service layer → data layer. Workspace-based Rust crate structure enables modular development and independent testing.

**Key Characteristics:**
- **Async-first:** Tokio runtime powers all I/O (database, file system, network).
- **Type-safe RPC:** Rust→TypeScript types generated via `ts-rs` macro at build time.
- **Event-driven:** Server pushes updates to clients via WebSocket (SSE/Axum for some endpoints).
- **Local-first data:** SQLite for quick startup and offline capability; Remote uses Postgres with ElectricSQL sync.
- **Modular executors:** Agents spawn as child processes, communicated via stdio (MCP protocol).

## Layers

**API/Routes Layer:**
- Purpose: Define HTTP endpoints, WebSocket handlers, request validation
- Location: `crates/server/src/routes/` (approvals, config, containers, events, filesystem, etc.)
- Contains: Handler functions, request/response validation, middleware
- Depends on: Services layer, Deployment (Axum state)
- Used by: Axum Router in `crates/server/src/main.rs`

**Service Layer:**
- Purpose: Implement business logic, coordinate between domain models
- Location: `crates/services/src/services/` (container, events, file, repo, auth, etc.)
- Contains: Public traits + implementations, state management, orchestration
- Depends on: DB, Git, utilities, external SDKs (e.g., reqwest for OAuth)
- Used by: Routes layer, Executors, other services

**Database/Model Layer:**
- Purpose: SQLx bindings, schema migrations, typed query results
- Location: `crates/db/src/` (models, migrations, queries)
- Contains: Sqlx `#[sqlx::query]` macros, migration files in `.sql`
- Depends on: sqlx crate, SQLite runtime
- Used by: Service layer queries

**Executor/Process Layer:**
- Purpose: Spawn child processes for AI agents, parse MCP protocol responses
- Location: `crates/executors/src/executors/` (executor spawning, MCP parsing, action dispatch)
- Contains: Executor discovery, action handlers, stdout/stderr parsing
- Depends on: Services, DB, Git (for context passing)
- Used by: Routes (when API calls ask for agent execution)

**Relay/Network Layer:**
- Purpose: Handle remote host connectivity, WebSocket relay, tunneling
- Location: `crates/relay-*/src/` (relay-client, relay-ws, relay-control, etc.)
- Contains: Connection management, message serialization, host registration
- Depends on: tokio-tungstenite, reqwest
- Used by: Main server for outbound relay registration, routes for WebSocket proxying

## Data Flow

### Primary Request Path: Execute Agent

1. **Frontend dispatches action** (`packages/local-web/src/routes/*.tsx`)
   - User clicks "Run Agent" button → calls API endpoint

2. **Route handler receives request** (`crates/server/src/routes/execution_processes.rs`)
   - Validates request payload
   - Extracts execution parameters (agent name, workspace context)

3. **Service spawns executor** (`crates/services/src/services/container.rs` + `crates/executors/src/executors/mod.rs`)
   - Calls `ContainerService::spawn_execution()`
   - Queries DB for executor definition
   - Spawns child process with MCP stdio

4. **Executor process runs** (separate Rust binary or language-specific script)
   - Reads tool schemas from `shared/schemas/*.json`
   - Executes agent logic, calls tools via MCP

5. **Actions invoke server tools** (`crates/executors/src/actions/`)
   - MCP action calls back to parent process
   - Routes → Services → DB/Git/Filesystem
   - Example: `actions/file.rs` handles `read_file` action

6. **Response streams back** (`crates/server/src/routes/execution_processes.rs`)
   - SSE (Server-Sent Events) or WebSocket frame to client
   - Frontend displays output in real-time

### Secondary Flow: Workspace Initialization

1. **User starts dev server** (`pnpm run dev` → `crates/server/src/main.rs`)
   - `LocalDeployment::new()` initializes services
   - Reads `.dev_assets/db.v2.sqlite` or creates new
   - Runs SQLx migrations

2. **Frontend connects** (`packages/local-web/src/app/entry/Bootstrap.tsx`)
   - Queries `/api/health` to verify backend ready
   - Fetches `/api/workspaces` to populate UI
   - WebSocket connection to `/api/events` for real-time updates

3. **Events service broadcasts** (`crates/services/src/services/events.rs`)
   - Workspace changes, file updates, execution progress
   - Routes via Axum handler to SSE/WebSocket clients

### Remote Deployment Flow (ElectricSQL)

1. **Cloud server starts** (`crates/remote/src/main.rs`)
   - Connects to Postgres
   - Initializes ElectricSQL shapes for replication

2. **Local client configures remote** (via OAuth)
   - Stores credentials in `shared/remote-types.ts`
   - Enables bidirectional sync

3. **Changes replicate** (ElectricSQL middleware)
   - Local SQLite ↔ Postgres (ElectricSQL protocol)
   - Conflict resolution per shape definition

**State Management:**
- **Rust side:** Deployment struct holds all service references, passed via Axum state
- **Frontend side:** TanStack Query + Zustand for caching, React context for auth state
- **Database state:** SQLite transactions ensure consistency within single process; Remote uses Postgres transactions

## Key Abstractions

**Deployment (Trait):**
- Purpose: Dependency injection container for all services
- Examples: `LocalDeployment` in `crates/local-deployment/`, `RemoteDeployment` in `crates/remote/`
- Pattern: `#[async_trait] pub trait Deployment` defines interface; implementations provide getters for DB, container, git, etc.

**ContainerService (Trait):**
- Purpose: Abstract execution environment (local Docker, remote containers, Tauri sandbox)
- Examples: `LocalContainerService` in `crates/local-deployment/container.rs`
- Pattern: Trait enables swapping implementations (testing, remote, desktop)

**ExecutorConfigs:**
- Purpose: Registry of available executors (Claude, local binaries, etc.)
- Examples: Read from `EXECUTOR_PRESETS` or external config
- Pattern: Lazy-loaded cache in `executors::profile::ExecutorConfigs::get_cached()`

**MCP Protocol Handler:**
- Purpose: Parse tool schemas and action responses
- Examples: `crates/mcp/src/` defines MCP request/response types
- Pattern: Serde JSON parsing; routes MCP calls back to Rust services

## Entry Points

**NPX CLI:**
- Location: `npx-cli/src/cli.ts` (TypeScript)
- Built by: `npm run build:npx` → esbuild → `npx-cli/bin/cli.js`
- Triggers: User runs `vibe-kanban` command
- Responsibilities: Download binary release, unzip, spawn `crates/server` binary with Cargo

**Server Binary (Dev):**
- Location: `crates/server/src/main.rs` (labeled as `default-run = "server"` in Cargo.toml)
- Triggers: `pnpm run backend:dev:watch` or `pnpm run dev`
- Responsibilities:
  - Parse `BACKEND_PORT`, `HOST` env vars (or defaults to 3000)
  - Initialize `LocalDeployment`
  - Bind TCP listeners (main + proxy)
  - Call `write_port_file_with_proxy()` so frontend discovers port
  - Serve Axum router until Ctrl+C

**Frontend Entry (Dev):**
- Location: `packages/local-web/index.html` → `src/app/entry/Bootstrap.tsx`
- Triggers: `pnpm run local-web:dev`
- Responsibilities:
  - Vite server on port 3000+ (auto-assigned via `setup-dev-environment.js`)
  - Proxy `/api/*` requests to backend (from `vite.config.ts`)
  - Mount React root, initialize router, connect WebSocket

**Frontend Entry (Prod):**
- Location: `packages/local-web/src/app/entry/Bootstrap.tsx` (same, SSR-rendered)
- Served by: Server at `crates/server/src/routes/frontend.rs`
- Responsibilities: Serve bundled `dist/index.html` + static assets; 404 → index.html for SPA routing

**Desktop App (Tauri):**
- Location: `crates/tauri-app/src-tauri/main.rs`
- Triggers: `pnpm run tauri:dev`
- Responsibilities:
  - Spawn backend server (or attach to existing)
  - Wrap frontend in native window (Webview)
  - Provide IPC bridge for system-level operations (file dialogs, etc.)

## Architectural Constraints

- **Threading:** Single Tokio runtime per process; no shared thread pool. Executors are spawned as separate child processes communicating via stdio (MCP).
- **Global state:** `Deployment` instance is cloned and passed via Axum state; service references are Arc-wrapped for safe sharing. No module-level static mutable state.
- **Circular imports:** None detected in workspace crate structure; clear dependency graph (API → Services → DB → utility crates).
- **Port resolution:** Backend writes port to `.dev-ports.json` file so frontend can discover it; no hardcoded ports in dev mode.
- **Asset serving:** Embedded in binary via `rust-embed` for production; dev mode serves from `packages/public/` and `assets/` directories.
- **Database migrations:** SQLx `sqlx::migrate!` macro verifies queries against live database; offline mode requires `prepare-db` script.

## Anti-Patterns

### Blocking I/O in Routes

**What happens:** A route handler calls a blocking operation (e.g., `std::fs::read_to_string`) without yielding to executor.

**Why it's wrong:** Blocks the entire Tokio runtime thread, starving other tasks. Axum is async but doesn't automatically serialize blocking ops.

**Do this instead:** Use `tokio::fs::read_to_string()` or wrap blocking code in `tokio::task::spawn_blocking()`. Example from `crates/services/src/services/filesystem.rs` — all filesystem ops use `tokio::fs::*` APIs.

### Unhandled Promise Rejections in Frontend

**What happens:** Async action in React component doesn't handle `.catch()` or use try/catch in async function.

**Why it's wrong:** Silent failures; hard to debug missing data in UI.

**Do this instead:** All API calls via TanStack Query hooks in `packages/web-core/src/` — they handle errors automatically. Example: `useWorkspaces()` hook catches errors and displays toast notifications.

### Spawning Executors Without Logging

**What happens:** `ContainerService::spawn_execution()` starts a process but doesn't capture or log its output.

**Why it's wrong:** Agent failures are invisible to user; errors lost in stderr.

**Do this instead:** Route handler opens `execution_processes` database record before spawning; executor process is wrapped to redirect stdout/stderr to file. See `crates/local-deployment/src/container.rs` — logs written to `dev_assets/logs/`.

### Inconsistent Error Response Format

**What happens:** Some endpoints return `{ error: "..." }`, others return `{ message: "..." }`.

**Why it's wrong:** Frontend error parsing breaks; inconsistent UX.

**Do this instead:** Use `crates/server/src/error.rs` — all routes return `ApiError` type (derives `Serialize`), which has consistent JSON structure.

## Error Handling

**Strategy:** Multi-layered with automatic Sentry reporting at the top level.

**Patterns:**
- **Rust errors:** Use `thiserror::Error` + custom enum variants in each crate (e.g., `DeploymentError`, `ContainerError`). Chain via `#[from]` attributes.
- **HTTP errors:** Route handlers convert Rust errors → HTTP status codes. `crates/server/src/error.rs` defines `ApiError` struct (implements `IntoResponse` for Axum).
- **Frontend errors:** TanStack Query + React error boundaries catch promise rejections. Toast notifications display to user.
- **Executor errors:** If child process exits non-zero, `ExecutorError` is returned; execution record marked as failed in DB.
- **Database errors:** SQLx errors wrapped in service error types; migrations fail loudly on startup.

## Cross-Cutting Concerns

**Logging:** Tracing crate with ENV_FILTER for log levels. Backend logs to stdout via `tracing-subscriber::fmt`. Frontend uses `console` module via browser DevTools.

**Validation:** 
- Rust: `schemars` derives JSON schema from types; endpoint request bodies validated by Axum (extracts JSON, returns 400 if invalid).
- Frontend: `zod` for form validation before submitting to API.

**Authentication:**
- Local: Workspace selected at startup; no per-request auth (localhost is trusted).
- Remote (Cloud): OAuth2 flow, JWT token stored in `LocalStorage`, passed in `Authorization: Bearer` header.
- Trusted key auth: `crates/trusted-key-auth/` manages SPAKE2 handshake for relay tunnels.

---

*Architecture analysis: 2026-05-16*
