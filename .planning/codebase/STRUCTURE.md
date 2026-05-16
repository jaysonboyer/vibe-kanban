# Codebase Structure

**Analysis Date:** 2026-05-16

## Directory Layout

```
vibe-kanban/
├── crates/                      # Rust workspace members
│   ├── server/                  # Main HTTP server binary + routes
│   ├── local-deployment/        # Local-specific initialization (containers, pty)
│   ├── remote/                  # Cloud server (separate workspace, excluded)
│   ├── db/                      # Database models, migrations, SQLx queries
│   ├── services/                # Business logic (containers, repos, events, auth)
│   ├── executors/               # AI executor spawning, MCP protocol handling
│   ├── git/                     # Git operations wrapper
│   ├── git-host/                # SSH Git server implementation
│   ├── deployment/              # Deployment trait definition
│   ├── api-types/               # Shared API types (TS/Rust JSON serialization)
│   ├── utils/                   # Common utilities (assets, port management)
│   ├── mcp/                     # Model Context Protocol server
│   ├── relay-*/                 # Relay network infrastructure
│   │   ├── relay-control/
│   │   ├── relay-protocol/
│   │   ├── relay-hosts/
│   │   ├── relay-ws/
│   │   ├── relay-client/
│   │   ├── relay-webrtc/
│   │   ├── relay-tunnel-core/
│   │   └── relay-tunnel/        # (excluded from workspace)
│   ├── tauri-app/               # Desktop app (Tauri wrapper)
│   ├── ws-bridge/               # WebSocket bridge for relay
│   ├── desktop-bridge/          # Desktop-specific features
│   ├── worktree-manager/        # Git worktree management
│   ├── workspace-manager/       # Workspace operations
│   ├── embedded-ssh/            # SSH server for remote access
│   ├── preview-proxy/           # Subdomain routing for preview
│   ├── trusted-key-auth/        # SPAKE2 key exchange
│   ├── review/                  # PR review tool
│   ├── client-info/             # Client metadata
│   ├── remote-info/             # Remote deployment info
│   ├── server-info/             # Server info APIs
│   └── Cargo.toml               # Workspace root config
│
├── packages/                    # NPM/pnpm workspace (TypeScript)
│   ├── local-web/               # Main web app (Vite + React)
│   │   ├── src/
│   │   │   ├── app/             # App root, entry point, providers
│   │   │   ├── routes/          # TanStack Router route definitions
│   │   │   └── shared/          # Local-specific utilities
│   │   ├── vite.config.ts
│   │   ├── index.html
│   │   └── package.json
│   ├── remote-web/              # Cloud deployment frontend
│   ├── web-core/                # Shared React components & hooks
│   │   ├── src/
│   │   │   ├── app/             # App shell, layout
│   │   │   ├── features/        # Feature modules (user, workspace, etc)
│   │   │   ├── pages/           # Page-level components
│   │   │   ├── shared/          # Shared components, hooks, types
│   │   │   └── styles/          # Global CSS, Tailwind config
│   │   └── package.json
│   ├── ui/                      # Design system (Radix + Tailwind)
│   │   ├── src/
│   │   │   └── components/      # Button, Modal, Input, etc
│   │   └── package.json
│   ├── public/                  # Static assets (favicons, manifests)
│   └── pnpm-workspace.yaml
│
├── npx-cli/                     # NPM CLI package (builds to bin/cli.js)
│   ├── src/
│   │   └── cli.ts              # CLI entry point (esbuild bundled)
│   ├── bin/
│   │   └── cli.js              # (generated) Bundled CLI
│   └── package.json
│
├── shared/                      # Generated types & schemas
│   ├── types.ts                 # (generated) TS types from crates/server/src/bin/generate_types.rs
│   ├── remote-types.ts          # (generated) TS types from crates/remote/src/bin/generate_types.rs
│   ├── jwt.ts                   # JWT parsing utilities
│   └── schemas/                 # (generated) JSON schemas for executors
│       └── *.json
│
├── scripts/                     # Dev helpers
│   ├── setup-dev-environment.js # Port allocation, .dev-ports.json
│   ├── prepare-db.js            # SQLx offline prepare
│   ├── build-bippy-bundle.mjs   # Bundler for binary assets
│   └── check-unused-i18n-keys.mjs
│
├── assets/                      # Production-bundled assets
│   └── (static files embedded in binary via rust-embed)
│
├── dev_assets_seed/             # Dev database templates
│   └── (copied to dev_assets/ on first run)
│
├── docs/                        # Mintlify documentation
│   ├── AGENTS.md                # Doc writing guidelines
│   └── (markdown docs for doc site)
│
├── bin/                         # Script wrappers
│   └── start-dev                # Docker compose helper
│
├── docker-compose.dev.yml       # Dev Docker setup (PostgreSQL, etc)
├── Dockerfile                   # Production build
├── Dockerfile.dev               # Dev build
├── Caddyfile.example            # Reverse proxy example
│
├── Cargo.toml                   # Rust workspace root
├── Cargo.lock                   # (committed) Dependency lock file
├── pnpm-workspace.yaml          # pnpm monorepo config
├── package.json                 # Root scripts (dev, build, lint)
├── pnpm-lock.yaml               # (committed) pnpm lock file
│
├── CLAUDE.md                    # (symlink to AGENTS.md) Project guidelines
├── AGENTS.md                    # Project structure & guidelines (this is the real file)
├── API.md                       # API endpoint documentation
├── README.md                    # Project readme
└── .planning/
    └── codebase/                # Generated codebase maps
        ├── ARCHITECTURE.md      # (this file)
        ├── STRUCTURE.md
        ├── CONVENTIONS.md
        ├── TESTING.md
        ├── STACK.md
        ├── INTEGRATIONS.md
        └── CONCERNS.md
```

## Directory Purposes

**`crates/`:**
- Purpose: Rust workspace containing all backend crates
- Contains: Source code (`src/`), test modules, Cargo manifests
- Key files: `crates/server/Cargo.toml`, `crates/Cargo.toml` (workspace root)

**`crates/server/`:**
- Purpose: Main HTTP server binary and API routes
- Contains: `main.rs` (binary entry), `startup.rs` (ServerHandle for Tauri), `error.rs` (error types), `routes/` (endpoint handlers), `middleware/` (auth, CORS, logging)
- Key files: `crates/server/src/main.rs` (listens on BACKEND_PORT, serves frontend)

**`crates/local-deployment/`:**
- Purpose: Local-mode initialization (in-process containers, PTY, SQLite)
- Contains: `LocalDeployment` struct, `LocalContainerService` (spawns child processes), PTY service
- Key files: `crates/local-deployment/src/lib.rs`

**`crates/db/`:**
- Purpose: SQLx database layer
- Contains: Model definitions, migration SQL files, typed query bindings
- Key files: `crates/db/src/lib.rs`, `crates/db/migrations/` (numbered .sql files)

**`crates/services/`:**
- Purpose: Business logic and domain services
- Contains: Container management, repo operations, file handling, events, auth, approvals
- Key files: `crates/services/src/services/container.rs`, `services/repo.rs`, `services/events.rs`

**`crates/executors/`:**
- Purpose: AI agent spawning and MCP (Model Context Protocol) handling
- Contains: Executor discovery, executor spawning, action dispatching, stdout parsing
- Key files: `crates/executors/src/executors/mod.rs`, `actions/` (file, shell, etc)

**`crates/deployment/`:**
- Purpose: Trait definition for Deployment abstraction
- Contains: `Deployment` trait, `DeploymentError`, common types
- Key files: `crates/deployment/src/lib.rs`

**`crates/utils/`:**
- Purpose: Shared utilities across crates
- Contains: Asset directory resolution, port file writing, Sentry integration, browser launcher
- Key files: `crates/utils/src/lib.rs`, `src/assets.rs`, `src/port_file.rs`

**`crates/api-types/`:**
- Purpose: Shared types for JSON serialization (Rust ↔ TypeScript)
- Contains: Request/response structs with `#[ts(export)]` macro
- Key files: `crates/api-types/src/lib.rs`

**`crates/remote/`:**
- Purpose: Cloud deployment server (separate Cargo workspace)
- Contains: Postgres integration, ElectricSQL shapes, billing, multi-tenancy
- Key files: `crates/remote/Cargo.toml`, `crates/remote/src/main.rs`
- **Note:** Excluded from main workspace due to different dependencies (postgres vs sqlite)

**`packages/local-web/`:**
- Purpose: Main React web app (Vite, TanStack Router, TanStack Query)
- Contains: Frontend source, routes, components, API hooks
- Key files: `packages/local-web/vite.config.ts`, `packages/local-web/index.html`, `packages/local-web/src/app/entry/Bootstrap.tsx`

**`packages/web-core/`:**
- Purpose: Shared React library (components, hooks, utilities)
- Contains: Design system, state management (Zustand), API client hooks, authentication
- Key files: `packages/web-core/src/shared/` (hooks, utils)

**`packages/ui/`:**
- Purpose: Design system components (Radix UI + Tailwind)
- Contains: Buttons, modals, forms, dialogs, etc
- Key files: `packages/ui/src/components/`

**`packages/public/`:**
- Purpose: Static public assets
- Contains: Favicons, manifests, site.webmanifest
- Key files: `packages/public/favicon-*.svg`, `packages/public/site.webmanifest`

**`npx-cli/`:**
- Purpose: Global NPM CLI package (published to npm registry)
- Contains: CLI argument parsing (cac), binary downloader, unzipper
- Key files: `npx-cli/src/cli.ts` (esbuild'd to `npx-cli/bin/cli.js`)

**`shared/`:**
- Purpose: Generated TypeScript types and JSON schemas
- Contains: Types auto-generated from Rust via `ts-rs`, executor JSON schemas
- Key files: `shared/types.ts` (generated), `shared/schemas/*.json` (generated)

**`scripts/`:**
- Purpose: Development helper scripts
- Contains: Port allocation, database preparation, bundle building
- Key files: `scripts/setup-dev-environment.js` (allocates ports, creates `.dev-ports.json`)

**`docs/`:**
- Purpose: Mintlify documentation source
- Contains: Markdown docs for doc site, API documentation
- Key files: `docs/AGENTS.md` (guidelines), various `.mdx` files

**`assets/` & `dev_assets_seed/`:**
- Purpose: Static files and dev database templates
- Contains: Favicons, site manifests, dev database initial state
- Key files: `dev_assets_seed/db.v2.sqlite` (copied to `dev_assets/` on first run)

## Key File Locations

**Entry Points:**

**Server (Production Binary):**
- `crates/server/src/main.rs` — Standalone server entry point (uses `LocalDeployment`)
  - Reads `BACKEND_PORT`, `HOST` env vars
  - Binds TCP listeners, initializes deployment, runs Axum router

**Server (Tauri Integration):**
- `crates/server/src/startup.rs` — `ServerHandle` + `initialize_deployment()` (used by Tauri)
  - Called from `crates/tauri-app/src-tauri/main.rs`
  - Returns handle with port for frontend to connect

**Frontend (Web App):**
- `packages/local-web/index.html` → `packages/local-web/src/app/entry/Bootstrap.tsx`
  - Entry point for browser rendering
  - Initializes React root, routing, providers

**Frontend (Static SPA Serving):**
- `crates/server/src/routes/frontend.rs` — Serves built React app
  - Route `/` and `/{*path}` serve `index.html` (SPA fallback)
  - Embedded in binary via `rust-embed` macro

**CLI (NPX):**
- `npx-cli/src/cli.ts` → esbuild → `npx-cli/bin/cli.js`
  - Entry point: `#!/usr/bin/env node` shebang
  - Parses CLI args, downloads binary release, spawns server

**Configuration:**

**Vite (Frontend Build):**
- `packages/local-web/vite.config.ts` — Dev server, build config, aliases
- `packages/web-core/vite.config.ts` — Library build config
- Aliases: `@web/` → `packages/local-web/src/`, `@/` → `packages/web-core/src/`, `shared` → `shared/`

**TypeScript:**
- `packages/local-web/tsconfig.json`, `packages/web-core/tsconfig.json`
- `npx-cli/tsconfig.json` — CLI TypeScript config

**Rust:**
- `Cargo.toml` (root workspace) — Workspace members, dependencies
- `crates/server/Cargo.toml` — `default-run = "server"`, binary definition
- `rustfmt.toml` — Code formatting rules
- `rust-toolchain.toml` — Pinned Rust version

**Database:**
- `crates/db/migrations/` — Numbered `.sql` files (run in order by SQLx)
- Example: `crates/db/migrations/20240101000000_init.sql`

**Core Logic:**

**Workspace Management:**
- `crates/services/src/services/container.rs` — Container/execution orchestration
- `crates/worktree-manager/src/lib.rs` — Git worktree operations
- `crates/workspace-manager/src/lib.rs` — Workspace CRUD

**Events & Real-time:**
- `crates/services/src/services/events.rs` — Event broadcasting
- `crates/server/src/routes/events.rs` — SSE endpoint `/api/events`

**File System:**
- `crates/services/src/services/filesystem.rs` — File read/write/watch
- `crates/server/src/routes/filesystem.rs` — File API endpoints

**Authentication:**
- `crates/services/src/services/auth.rs` — Auth context, user identity
- `crates/trusted-key-auth/src/lib.rs` — SPAKE2 key exchange

**Testing:**
- Unit tests co-located with code: `#[cfg(test)]` modules in `.rs` files
- Integration tests: `tests/` directories in crate roots (none currently)
- Frontend tests: TBD (Vitest setup available in `packages/local-web/`)

## Naming Conventions

**Files:**
- Rust source: `snake_case.rs` (e.g., `event_service.rs`, `deployment_impl.rs`)
- TypeScript source: `kebab-case.ts` or `PascalCase.tsx` for components (e.g., `hooks.ts`, `Button.tsx`)
- Routes in `crates/server/src/routes/`: `plural_noun.rs` (e.g., `workspaces.rs`, `execution_processes.rs`)

**Directories:**
- Rust modules: `snake_case/` (e.g., `crates/services/src/services/`)
- TS feature folders: `kebab-case/` (e.g., `packages/web-core/src/features/`)
- Routes: `routes/` (contains route handlers)
- Utilities: `shared/`, `utils/`

**Functions/Variables:**
- Rust: `snake_case` (e.g., `spawn_execution()`, `initialize_deployment()`)
- TypeScript: `camelCase` (e.g., `useWorkspaces()`, `fetchRepoInfo()`)
- React Components: `PascalCase` (e.g., `WorkspaceCard`, `ExecutionViewer`)
- React Hooks: `camelCase` with `use` prefix (e.g., `useWorkspace()`, `useEvents()`)

**Types:**
- Rust: `PascalCase` (e.g., `LocalDeployment`, `ContainerService`)
- TypeScript: `PascalCase` (e.g., `Workspace`, `ExecutionResult`)
- TS Interfaces: `PascalCase` (e.g., `IWorkspaceOptions`)

**Database:**
- Table names: `snake_case` (e.g., `workspaces`, `execution_processes`)
- Column names: `snake_case` (e.g., `created_at`, `workspace_id`)
- Migration files: `YYYYMMDDhhmmss_description.sql` (e.g., `20240115120000_add_workspace_table.sql`)

## Where to Add New Code

**New Feature (Full Stack):**
- Backend API endpoint: `crates/server/src/routes/[feature_name].rs`
- Service logic: `crates/services/src/services/[feature_name].rs`
- Database model: `crates/db/src/models/[feature_name].rs` (if needed)
- Frontend page: `packages/local-web/src/routes/[FeatureName]/`
- Frontend hooks: `packages/web-core/src/shared/hooks/use-[feature-name].ts`
- Tests: Co-located with source (`#[cfg(test)]` in Rust, `.test.tsx` next to `.tsx` in TS)

**New Component:**
- Design system: `packages/ui/src/components/[ComponentName]/index.tsx`
- Feature-specific: `packages/web-core/src/features/[feature]/components/[ComponentName].tsx`
- Local-specific: `packages/local-web/src/routes/[Route]/[ComponentName].tsx`

**New Service/Crate:**
- Backend service: `crates/[service-name]/src/lib.rs`
- Add to `Cargo.toml` workspace members
- Export public API via `pub mod` in `lib.rs`
- Depend on via `[dependencies]` in `Cargo.toml`

**New Executor Action:**
- Action handler: `crates/executors/src/actions/[action_name].rs`
- Register in: `crates/executors/src/executors/mod.rs` (match statement)
- Schema: Auto-generated from Rust type, placed in `shared/schemas/`

**Utilities:**
- Shared Rust: `crates/utils/src/` (or new crate if significant)
- Shared TypeScript: `packages/web-core/src/shared/`
- Local-specific helpers: `packages/local-web/src/shared/`

## Special Directories

**`.dev-ports.json`:**
- Purpose: Stores allocated ports for dev server
- Generated: Yes (by `scripts/setup-dev-environment.js`)
- Committed: No (in `.gitignore`)
- Format: `{ "frontend": 3000, "backend": 3001, "preview_proxy": 3002 }`

**`dev_assets/`:**
- Purpose: Local development database and logs
- Generated: Yes (on first run, copied from `dev_assets_seed/`)
- Committed: No (in `.gitignore`)
- Contains: `db.v2.sqlite`, `logs/`, config files

**`assets/`:**
- Purpose: Production-bundled assets
- Generated: No (committed)
- Embedded: Via `rust-embed` macro in `crates/server/src/routes/frontend.rs`
- Served: At `/` when running production binary

**`shared/schemas/`:**
- Purpose: JSON schemas for executor configurations
- Generated: Yes (by `pnpm run generate-types`)
- Committed: Yes (schemas are committed)
- Source: Derives from Rust types via `schemars` crate

**`shared/types.ts` & `shared/remote-types.ts`:**
- Purpose: TypeScript type definitions auto-generated from Rust
- Generated: Yes (by `pnpm run generate-types` and `pnpm run remote:generate-types`)
- Committed: Yes (generated files are committed)
- Source: Rust `#[derive(TS)]` macro in Rust crates
- Regenerate: When Rust API types change

**`.planning/codebase/`:**
- Purpose: Codebase analysis documents (this directory)
- Generated: Yes (by `/gsd-map-codebase` orchestrator)
- Committed: Yes
- Contains: ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, TESTING.md, STACK.md, INTEGRATIONS.md, CONCERNS.md

---

*Structure analysis: 2026-05-16*
