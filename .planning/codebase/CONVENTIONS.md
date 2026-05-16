# Coding Conventions

**Analysis Date:** 2026-05-16

## Naming Patterns

**Rust:**
- **Modules:** snake_case (e.g., `relay_pairing`, `error_logging`)
- **Types/Structs:** PascalCase (e.g., `ServerHandle`, `AttachmentResponse`, `ApiError`)
- **Functions/Methods:** snake_case (e.g., `validate_origin`, `process_file_upload`, `from_file`)
- **Constants:** SCREAMING_SNAKE_CASE
- **Traits:** PascalCase
- **Macro names:** snake_case

**TypeScript/React:**
- **Files:** Depend on type:
  - React components (`.tsx`): PascalCase (e.g., `App.tsx`, `App.tsx`, `AppRuntimeProvider.tsx`)
  - Hooks (starting with `use`): camelCase (e.g., `useSessionStorageState.ts`, `useTauriNotificationNavigation.ts`)
  - Utils/helpers: camelCase (e.g., `clipboard.ts`, `diffDataAdapter.ts`, `diffStatsParser.ts`)
  - Lib files: camelCase (e.g., `api.ts`, `colors.ts`, `date.ts`)
  - Config files: camelCase (e.g., `vite.config.ts`)
- **Components:** PascalCase (e.g., `App`, `AppSystemNotifications`, `ClickedElementsProvider`)
- **Functions:** camelCase (e.g., `parentClipboardWrite`, `writeClipboardViaBridge`)
- **Variables:** camelCase (e.g., `isRelayRequest`, `contentOmitted`, `additionLines`)
- **Directories:** Depend on context:
  - Feature/layer directories: lowercase (e.g., `app/`, `routes/`, `shared/`, `entities/`, `features/`, `widgets/`, `pages/`, `integrations/`)
  - Component directories: PascalCase (e.g., `ui-new/`, `primitives/`, `views/`, `containers/`)
  - Utility directories: lowercase (e.g., `lib/`, `utils/`, `hooks/`, `constants/`)

## Code Style

**Formatting:**
- **Tool:** Prettier (configured in `packages/web-core/.prettierrc.json`)
- **Settings:**
  - Print width: 80 columns
  - Tab width: 2 spaces (no tabs)
  - Trailing comma: ES5 style
  - Quotes: Single quotes
  - Semicolons: Enabled

**Rust Formatting:**
- **Tool:** `rustfmt` with `rustfmt.toml` configuration
- **Settings (from `rustfmt.toml`):**
  - Edition: 2024
  - Reorder imports: enabled
  - Group imports: by "StdExternalCrate" (standard library, external crates, then local crates)
  - Imports granularity: Crate level

**Linting:**
- **Frontend (TypeScript/React):**
  - ESLint (configured in `packages/local-web/.eslintrc.cjs`, `packages/ui/.eslintrc.cjs`)
  - Key extensions:
    - `@typescript-eslint/recommended` — type safety rules
    - `react-hooks/recommended` — hooks best practices
    - `i18next/recommended` — internationalization (optional via `LINT_I18N=true`)
    - `prettier` — formatting compatibility
  - Key rules enforced:
    - No unused imports or variables (`unused-imports` plugin)
    - No explicit `any` types (warning level)
    - Switch exhaustiveness checking (requires handling all enum cases)
    - No re-exports in `ui-new/` (`no-restricted-syntax`)
    - File naming conventions via `check-file` plugin
    - Layer boundary enforcement (`no-restricted-imports` overrides by directory)

- **Backend (Rust):**
  - `cargo clippy` — linting with pedantic warnings as errors
  - Runs on all workspace crates

**Command format/lint:**
- `pnpm run format` — Runs `cargo fmt` for all Rust workspaces + Prettier for TypeScript
- `pnpm run lint` — Runs `cargo clippy` for all backend + ESLint for web packages

## Import Organization

**Rust (per `rustfmt.toml`):**
1. **Standard library imports** (e.g., `use std::...`)
2. **External crate imports** (e.g., `use axum::...`, `use serde::...`)
3. **Internal crate imports** (e.g., `use db::...`, `use crate::...`)

Within each group, imports are sorted alphabetically. Imports from the same crate are grouped together.

**TypeScript/React:**
1. **External node modules** (e.g., `import React from 'react'`, `import { describe, it } from 'vitest'`)
2. **Internal workspace packages** (e.g., `import { ... } from '@vibe/web-core'`, `import { ... } from '@vibe/ui'`)
3. **Alias-based internal imports** (e.g., `import { ... } from '@/shared'`, `import { ... } from '@web/app'`)
4. **Relative imports** (`.`, `..`, `./component`)

**Path Aliases:**
- `@/` — Points to the local app's `src/` root (used in monorepo packages)
- `@web/` — Points to web-specific app structures in `packages/local-web`
- `@vibe/` — Points to shared workspace packages (`@vibe/web-core`, `@vibe/ui`)
- `@/shared/` — Points to shared utilities and layers in the current app

## Error Handling

**Rust:**
- **Primary approach:** `anyhow::Result<T>` with `?` operator for propagation
- **Custom errors:** Use `thiserror` crate with `#[derive(Error)]` and `#[error(...)]` attributes
- **API errors:** Defined in `crates/server/src/error.rs` as `ApiError` enum with `ts_rs::TS` derive for TypeScript generation
- **Error transparency:** Use `#[error(transparent)]` for error wrapper types (e.g., `#[error(transparent)] Repo(#[from] RepoError)`)
- **Error context:** Add contextual information via `.context("message")` or `.wrap_err("message")`

**TypeScript/React:**
- **Async error handling:** Use `try/catch` blocks, `.catch()` handlers, or error boundaries
- **Validation:** Use Zod for runtime schema validation before API calls
- **API errors:** Caught and logged, typically displayed to user via toast notifications or error dialogs

## Logging

**Framework:** `tracing` crate for Rust backend logging

**Rust Patterns:**
- Use `tracing::info!()`, `tracing::warn!()`, `tracing::error!()`, `tracing::debug!()`
- Structured logging with key-value pairs: `tracing::info!(user_id = ?user_id, "User logged in")`
- Log level configured via `RUST_LOG` env var (default `info` for app, debug in dev mode)
- Sentry integration via `utils::sentry` module for error tracking

**TypeScript/React:**
- Console-based (dev) or error reporting via Sentry (production)
- Structured logging not enforced in frontend

## Comments

**When to Comment:**
- Explain non-obvious algorithm choices or business logic
- Mark temporary workarounds with `// TODO:` or `// FIXME:`
- Document public function/type contracts with doc comments

**Rust Doc Comments:**
```rust
/// One-line description.
///
/// Detailed explanation if needed.
pub fn my_function() { }
```

**TypeScript JSDoc (optional but encouraged):**
```typescript
/**
 * One-line description.
 *
 * @param param1 - Description
 * @returns Description
 */
export function myFunction(param1: string): boolean { }
```

**Code Comments:**
- Use `//` for single-line comments
- Explain **why**, not **what** (code shows what it does)

## Function Design

**Rust:**
- **Size:** Keep functions focused on a single responsibility; aim for <100 lines
- **Parameters:** Use type system to ensure correctness; prefer specific types over `impl Trait` in function signatures
- **Return Values:** 
  - Use `Result<T, E>` for fallible operations
  - Use `Option<T>` for optional values
  - Derive `Debug` on all types for better error messages
- **Async:** Mark with `async fn` and return futures

**TypeScript/React:**
- **Component Size:** Keep components under 300 lines; extract custom hooks for complex logic
- **Props:** Define as TypeScript interface with JSDoc where needed
- **Return Values:** Components return JSX or null; hooks return state/callbacks
- **Hooks:** Name with `use` prefix; call only at top level

## Module Design

**Rust:**
- **Exports:** Use `pub` to mark public APIs; `pub(crate)` for internal-only visibility
- **Organization:** Group related functions/types in modules; nest modules logically
- **Crate boundaries:** Each `crates/*/` directory is a separate crate with its own `Cargo.toml`

**TypeScript/React:**
- **Exports:** Named exports preferred over default exports (exception: Route/App entry points)
- **Barrel Files:** Discouraged in `ui-new/` layers (ESLint rule bans re-exports); allowed in shared layers
- **Index files:** Export only what's intentionally public from a directory

## Type Generation (Rust ↔ TypeScript)

**Critical:** Do NOT manually edit `shared/types.ts` or `shared/remote-types.ts`

**Process:**
1. **Add/modify Rust type** in crate source (e.g., `crates/server/src/routes/attachments.rs`):
   ```rust
   #[derive(Debug, Serialize, Deserialize, TS)]
   pub struct AttachmentResponse {
       pub id: Uuid,
       pub file_path: String,
       // ...
   }
   ```

2. **Regenerate TypeScript types:**
   ```bash
   pnpm run generate-types          # For local server types → shared/types.ts
   pnpm run remote:generate-types   # For remote server types → shared/remote-types.ts
   ```

3. **Generator binaries:**
   - Local: `crates/server/src/bin/generate_types.rs` — reads Rust types and outputs `shared/types.ts`
   - Remote: `crates/remote/src/bin/remote-generate-types.rs` — reads remote types and outputs `shared/remote-types.ts`

4. **CI check:** `pnpm run generate-types:check` verifies types are up-to-date (fails if drift detected)

## Shared API Types

**Location:** `crates/api-types/` — Rust crate containing types shared between local and remote servers

**Pattern:** Types used in both local and remote APIs are defined here to avoid duplication. They are generated into TypeScript via ts-rs derive macro.

## Special Directives

**Local Web App (`packages/local-web/`):**
- **ESLint layer enforcement:** Different overrides apply to different directory layers (`app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`, `integrations/`)
- **Legacy directories banned:** Code in legacy dirs (`components/`, `hooks/`, `lib/`, etc.) must migrate to proper FSD layers
- **Dialog pattern:** Use `DialogName.show(props)` / `DialogName.hide()`; do not import `NiceModal` directly
- **Presentation rule:** Components in `ui-new/views/` and `ui-new/primitives/` must not use hooks, API calls, or state management
- **Icon sizing:** Use Tailwind design system sizes (`size-icon-xs`, `size-icon-sm`, etc.), not arbitrary pixel sizes
- **i18n enforcement:** When `LINT_I18N=true`, literal strings in JSX trigger warnings (except test files)

## Type Checking

**Commands:**
- `pnpm run check` — Runs TypeScript `tsc --noEmit` on all frontend packages + backend `cargo check`
- `pnpm run backend:check` — Rust-only: `cargo check --workspace` (includes `crates/remote`)

Type checking is required to pass CI.

---

*Convention analysis: 2026-05-16*
