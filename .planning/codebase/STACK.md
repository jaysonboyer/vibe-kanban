# Technology Stack

**Analysis Date:** 2026-05-16

## Languages

**Primary:**
- Rust (edition 2024, 1.0+) - Backend server, MCP server, CLI binaries, all crates in `crates/`
- TypeScript 5.7.0 - React frontend (local-web, remote-web, web-core), MCP client, npx-cli
- JavaScript (Node.js) - Build scripts, dev utilities

**Secondary:**
- SQL (SQLx) - Database migrations and queries (SQLite for local, Postgres for remote)

## Runtime

**Environment:**
- Rust: Tokio 1.0 async runtime (full feature set)
- Node.js: >=20 (specified in `package.json` engines)
- Postgres: 16-alpine (remote deployment via docker-compose)

**Package Manager:**
- pnpm: 10.13.1 (workspace manager, enforced in `package.json`)
- Cargo: Rust package manager (workspace members in Cargo.toml)
- npm: for npx-cli (separate npm workspace)

## Frameworks

**Core Backend:**
- Axum 0.8.4 - HTTP server framework (features: macros, multipart, ws)
- Tokio 1.0 - Async runtime with full feature set
- SQLx 0.8.6 - Database access with compile-time query verification
- Tower-HTTP 0.5 - Middleware suite (CORS, compression, tracing, FS serving, validation)

**Frontend:**
- React 18 + React Router 7 - UI framework and client-side routing
- Vite 7.3.1 - Build tool and dev server
- Tailwind CSS - Styling framework
- Radix UI - Headless component library (@radix-ui/*)
- Lexical 0.36.2 - Rich text editor

**Build & Development:**
- esbuild 0.27.2 - Bundler for npx-cli
- Cargo (Rust compiler) - All backend crates
- Prettier (2 spaces, single quotes) - Code formatter
- ESLint - Code linting
- TypeScript 5.7.0 - Type checking

**Testing:**
- Vitest - Frontend test runner (referenced in conventions)
- cargo test - Rust unit tests (run with `cargo test --workspace`)

**Type Generation:**
- ts-rs (custom fork from xazukx/ts-rs, branch use-ts-enum) - Derives TypeScript from Rust structs
- schemars 1.0.4 - JSON Schema generation

## Key Dependencies

**Critical:**
- sqlx 0.8.6 - SQLite (local) and Postgres (remote) with compile-time query safety
- reqwest 0.13 - HTTP client with TLS (rustls with aws-lc-rs crypto)
- sentry 0.46.2 - Error tracking and release monitoring
- tokio-tungstenite 0.26 - WebSocket support for relay communication

**Infrastructure:**
- rmcp 1.2.0 - Model Context Protocol (MCP) server implementation
- agent-client-protocol 0.8 - Claude MCP agent protocol support
- codex-protocol (OpenAI, git tag rust-v0.124.0) - OpenAI Codex integration
- ts-rs (workspace dep) - Rust↔TypeScript type generation
- serde_json 1.0 - JSON serialization with order preservation
- chrono 0.4 - Date/time handling with serde support

**Frontend:**
- @lexical/react 0.36.2 - Rich text editing components
- @dnd-kit/core, @dnd-kit/sortable - Drag-and-drop functionality
- @tanstack/electric-db-collection - ElectricSQL sync binding
- @tanstack/react-query 5.85.5 - Server state management
- @sentry/react 9.34.0 - Error tracking

**Relay & Communication:**
- relay-client, relay-ws, relay-protocol, relay-webrtc - Custom relay protocol for P2P/tunneling
- ws-bridge - WebSocket bridge for relay connections
- tokio-util 0.7 - Async utilities
- futures 0.3 - Async primitives

**Crypto & TLS:**
- rustls 0.23 - TLS client (aws-lc-rs backend, no OpenSSL)
- aws-lc-rs 1.16.0 - AWS cryptography provider
- ed25519-dalek 2.2.0 - EdDSA signatures
- spake2 0.5.0-pre.0 - SPAKE2 key exchange (for relay auth)

## Configuration

**Environment Variables (Core):**
- `BACKEND_PORT` - Backend server port (default 3000), read from ENV or PREVIEW_PROXY_PORT
- `FRONTEND_PORT` - Frontend dev server port (auto-assigned via setup-dev-environment.js, default 3000)
- `PREVIEW_PROXY_PORT` - Subdomain proxy for preview links (auto-assigned, default 3002)
- `HOST` - Bind address for server (default 127.0.0.1, use 0.0.0.0 for Docker)
- `RUST_LOG` - Tracing log level (default "info", supports debug-level routing per crate)
- `VK_ALLOWED_ORIGINS` - CORS allowed origins (e.g., "http://localhost:3000" in dev)
- `VITE_VK_SHARED_API_BASE` - Frontend API base URL override (optional remote server connection)

**Sentry (Error Tracking):**
- `SENTRY_DSN` or `SENTRY_DSN_BACKEND` - Backend error tracking
- `SENTRY_DSN_REMOTE` - Remote server (cloud) error tracking (crates/remote/)

**Dev Tool Config:**
- Ports auto-allocated by `scripts/setup-dev-environment.js`, stored in `.dev-ports.json` (gitignored)
- Dev assets copied from `dev_assets_seed/` to `dev_assets/` on first run
- Database file location: `~/.local/share/vibe-kanban/db.v2.sqlite` (Linux/macOS) or `%APPDATA%\vibe-kanban\db.v2.sqlite` (Windows)

**Build:**
- Cargo.toml at repo root defines workspace members and shared dependencies
- TypeScript projects have separate tsconfig.json files (npx-cli, local-web, web-core, remote-web)
- Prettier config at root for formatting consistency
- ESLint config for web projects
- Rust enforced with rustfmt and clippy

## Platform Requirements

**Development:**
- Node.js >=20
- pnpm >=8
- Rust (latest stable)
- SQLite development libraries (for offline query checks)
- For Windows: native-tls or rustls (no vendored OpenSSL needed; we use aws-lc-rs)

**Production (Local/Desktop):**
- Linux x64, Linux ARM64, macOS x64 (Intel), macOS ARM64 (Apple Silicon), Windows x64, Windows ARM64
- SQLite database file in user's asset directory
- No external services required (batteries-included)

**Production (Remote/Cloud):**
- Postgres 16+ with logical replication (`wal_level=logical`)
- ElectricSQL service (internal, port 3000)
- Docker environment (multi-stage: Node → Rust → Debian slim runtime)
- Azure Blob Storage or AWS S3 (optional, for attachments)
- SMTP or webhook service for notifications (optional)

## Deployment

**NPX Distribution:**
- Published to npm as `vibe-kanban` (version 0.1.44)
- Entrypoint: `npx-cli/bin/cli.js` (Node 20+, bundled with esbuild)
- CLI downloads pre-built binaries from R2 CDN (platform-specific zips)
- Supports: `npx vibe-kanban` (browser mode, default), `npx vibe-kanban --desktop` (Tauri app), `npx vibe-kanban mcp` (MCP server)
- Caching: `~/.cache/vibe-kanban/` on Linux/macOS, `%APPDATA%\vibe-kanban\` on Windows
- Global symlink install: `ln -s $(pwd)/npx-cli/bin/cli.js ~/.local/bin/vibe-kanban`

**Local Docker:**
- `compose.yml` at repo root spins up vibe-kanban container
- Maps `PORT` env var to port 3000 in container
- Mounts `/repos` volume for workspace data

**Remote Docker:**
- `crates/remote/docker-compose.yml` includes: remote-server (Axum), remote-db (Postgres), electric (ElectricSQL), azurite (Azure Blob mock)
- Multi-stage Dockerfile: Node stage builds frontend → Rust stage builds binary → runtime runs binary
- Server listens on `:8081` (PORT=3000 in container maps to :8081 on host)
- ElectricSQL on internal port 3000

## Asset Handling

- Frontend builds bundled into Rust binary via `rust-embed` (served from `/` and `/api/` paths)
- Local dev: Vite dev server on `FRONTEND_PORT`, proxies API calls to backend
- Sound assets (`/api/sounds/{sound}`) served from `assets/` directory
- Static SPA fallback for remote deployment: `/srv/static` (built during Docker image creation)

---

*Stack analysis: 2026-05-16*
