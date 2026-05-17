# Testing Patterns

**Analysis Date:** 2026-05-16

## Test Framework

**Rust:**
- **Runner:** `cargo test --workspace`
- **Framework:** Standard Rust testing with `#[cfg(test)]` modules
- **Assertions:** Standard `assert!`, `assert_eq!`, `assert_ne!`
- **Config:** Tests run in-process with test threads (default parallelization)

**TypeScript/Frontend:**
- **Runner:** Vitest (installed but limited usage)
- **Status:** Minimal test coverage; one test file found
- **Usage:** Import patterns show `import { describe, it, expect } from 'vitest'`

## Test File Organization

**Rust:**
- **Location:** Colocated with source code using `#[cfg(test)]` modules
- **Pattern:** Tests live in the same file as implementation, within a conditional test module

**TypeScript/React:**
- **Location:** Colocated in same directory as source (e.g., `diffDataAdapter.test.ts` next to `diffDataAdapter.ts`)
- **Naming:** `*.test.ts` or `*.test.tsx` suffix
- **Status:** Minimal usage; only one test file found in the entire codebase (`packages/web-core/src/shared/lib/diffDataAdapter.test.ts`)

## Test Structure

**Rust Pattern (from codebase):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        // Arrange
        let fixture = setup();

        // Act
        let result = function_under_test();

        // Assert
        assert_eq!(result, expected);
    }
}
```

Examples found in:
- `crates/server/src/startup.rs` — Server initialization tests
- `crates/server/src/routes/attachments.rs` — File upload validation tests
- `crates/server/src/middleware/origin.rs` — Origin header validation tests
- `crates/git-host/src/detection.rs` — Git host detection tests
- `crates/relay-webrtc/src/fragment.rs` — WebRTC fragment handling tests
- `crates/executors/src/logs/utils/entry_index.rs` — Log entry indexing tests

**TypeScript Pattern (from `diffDataAdapter.test.ts`):**
```typescript
import { describe, it, expect } from 'vitest';
import { transformDiffToFileDiffMetadata } from './diffDataAdapter';
import type { Diff } from 'shared/types';

// Test fixtures
function createDiffFixture(overrides: Partial<Diff> = {}): Diff {
  return {
    // default values
    ...overrides,
  } as Diff;
}

// Test suite
describe('transformDiffToFileDiffMetadata', () => {
  it('produces FileDiffMetadata with expected fields', () => {
    const diff = createDiffFixture();
    const result = transformDiffToFileDiffMetadata(diff);

    expect(result).toHaveProperty('name');
    expect(result.type).toBe('change');
  });

  it('handles contentOmitted with placeholder metadata', () => {
    const diff = createDiffFixture({ contentOmitted: true });
    const result = transformDiffToFileDiffMetadata(diff);

    expect(result.hunks).toEqual([]);
  });
});
```

## Mocking

**Rust:**
- **Framework:** `#[test]` uses real dependencies where practical
- **Mocking:** Limited; tests call actual functions with fixtures
- **Test fixtures:** Inline setup functions (e.g., `createDiffFixture()` in TypeScript tests, similar patterns expected in Rust)

**TypeScript/React:**
- **Status:** Minimal mocking setup observed
- **Tools available:** Vitest supports `vi.mock()` and `vi.fn()` but not extensively used
- **Pattern:** When needed, import fixtures and pass as arguments

## Fixtures and Factories

**Pattern (from `diffDataAdapter.test.ts`):**
```typescript
function createDiffFixture(overrides: Partial<Diff> = {}): Diff {
  return {
    oldPath: 'src/hello.ts',
    newPath: 'src/hello.ts',
    oldContent: 'const a = 1;\nconst b = 2;\n',
    newContent: 'const a = 1;\nconst b = 3;\nconst c = 4;\n',
    change: 'modified',
    contentOmitted: false,
    ...overrides,
  } as Diff;
}
```

**Location:** Fixtures defined inline in test files or in a `fixtures.ts` file in the same directory

## Coverage

**Requirements:** No enforced coverage targets

**View Coverage:**
```bash
cargo tarpaulin --workspace  # For Rust (if installed)
# TypeScript coverage via vitest with --coverage flag (not configured)
```

## Test Types

**Unit Tests (Rust):**
- **Scope:** Test individual functions/modules in isolation
- **Approach:** Use `#[test]` attribute; call functions with fixtures; assert results
- **Examples:**
  - Origin validation: `crates/server/src/middleware/origin.rs` — Tests URL parsing and host normalization
  - Git detection: `crates/git-host/src/detection.rs` — Tests git host identification logic
  - Path utilities: `crates/utils/src/path.rs` — Tests file path operations
  - Log indexing: `crates/executors/src/logs/utils/entry_index.rs` — Tests entry indexing for logs

**Unit Tests (TypeScript):**
- **Scope:** Test utility functions and data transformations
- **Approach:** Import function, create fixture, call, assert
- **Example:** `diffDataAdapter.test.ts` — Tests diff parsing and metadata generation

**Integration Tests:**
- **Status:** Not formally organized; backend tests with real database/services live in `#[cfg(test)]` modules
- **Example:** `crates/server/src/startup.rs` contains tests that start server, verify configuration

**E2E Tests:**
- **Status:** Not found in codebase
- **Manual testing:** `pnpm test:npm` runs `test-npm-package.sh` (validates npx CLI build)

## Run Commands

**Rust Tests:**
```bash
cargo test --workspace                    # Run all tests
cargo test --bin server                   # Run tests for specific crate
cargo test -- --nocapture                 # Show println! output
cargo test --lib                          # Run library tests only
cargo test -- --test-threads=1            # Run sequentially
```

**Type Checking (Required by CI):**
```bash
pnpm run generate-types:check              # Verify shared/types.ts matches Rust types
pnpm run remote:generate-types:check       # Verify shared/remote-types.ts matches remote types
pnpm run check                             # TypeScript type check all packages
pnpm run backend:check                     # cargo check all workspaces
```

**Linting (Required by CI):**
```bash
pnpm run lint                              # ESLint + cargo clippy
pnpm run backend:lint                      # cargo clippy only
pnpm run local-web:lint                    # ESLint only for local-web
```

**Formatting (Required before commit):**
```bash
pnpm run format                            # Format all Rust + TypeScript code
```

## Testing Gaps & Coverage Concerns

**High-Risk Areas with Minimal Testing:**

### 1. **Server Startup & Initialization**
- **Files:** `crates/server/src/startup.rs`, `crates/server/src/main.rs`
- **Current coverage:** Basic startup tests in `startup.rs` check port binding
- **Gap:** Database initialization, configuration loading, relay registration startup path
- **Impact:** Startup failures won't be caught until runtime
- **Recommendation:** Add tests for:
  - Database migration failures
  - Missing environment variables
  - Port binding conflicts
  - Asset directory creation
  - Relay registration failures

### 2. **Binary Entry Points**
- **Files:** `crates/server/src/bin/generate_types.rs`, `crates/server/src/main.rs`
- **Current coverage:** No dedicated tests
- **Gap:** Binary initialization and argument parsing not covered
- **Impact:** CLI flag changes or new startup logic could break silently
- **Recommendation:** Add integration tests for:
  - Binary invocation with various flags
  - Environment variable precedence
  - Error exit codes

### 3. **NPX CLI Build & Bundle**
- **Files:** `npx-cli/src/`, `npx-cli/package.json`
- **Current coverage:** Manual validation via `pnpm test:npm` script (shell-based, not unit tested)
- **Gap:** Bundle integrity, CLI argument parsing not covered
- **Impact:** CLI publishing could fail or ship broken binaries
- **Recommendation:** Add tests for:
  - Binary startup with test arguments
  - File extraction (zip contents)
  - Version flag parsing

### 4. **Error Handling in Routes**
- **Files:** `crates/server/src/routes/` (workspaces, attachments, events, containers, etc.)
- **Current coverage:** Individual route tests sparse
- **Gap:** Error paths (invalid input, missing resources) rarely tested
- **Impact:** Unhandled errors could leak internal state or return poor error messages
- **Recommendation:** Add tests for:
  - Invalid UUID formats
  - Missing workspace/file references
  - Multipart parsing failures (attachment upload)
  - Concurrent request handling

### 5. **Frontend Components (React)**
- **Files:** `packages/local-web/src/`, `packages/web-core/src/`
- **Current coverage:** One test file found (`diffDataAdapter.test.ts`)
- **Gap:** No tests for components, hooks, providers, or routing
- **Impact:** UI regressions, broken features not caught until manual testing
- **Recommendation:** Add Vitest tests for:
  - Custom hooks (query/mutation behavior)
  - Component rendering with different props
  - Layer boundary enforcement (presentation components don't use API)
  - Dialog pattern compliance

### 6. **Type Generation Pipeline**
- **Files:** `crates/server/src/bin/generate_types.rs`, `crates/remote/src/bin/remote-generate-types.rs`
- **Current coverage:** CI check (`generate-types:check`) verifies no drift, but not the generation logic itself
- **Gap:** Type serialization, enum handling, recursive types not explicitly tested
- **Impact:** Generated types might have wrong type signatures or missing fields
- **Recommendation:** Add tests for:
  - Enum variant serialization
  - Optional field handling
  - Recursive/nested type generation
  - Type name collision handling

## CI Pipeline

**Commands run in CI (from `package.json`):**
1. `pnpm run lint` — ESLint + cargo clippy
2. `pnpm run check` — TypeScript types + cargo check
3. `pnpm run generate-types:check` — Verify types.ts matches Rust sources
4. `pnpm run prepare-db:check` — Verify SQLx metadata is up-to-date

**No explicit `cargo test` in CI** — Tests must pass locally before committing

**Frontend checks:**
- `pnpm run local-web:check` — TypeScript in local-web
- `pnpm run remote-web:check` — TypeScript in remote-web
- `pnpm run web-core:check` — TypeScript in web-core
- `pnpm run ui:check` — TypeScript in ui package

## Testing the NPX CLI

**Manual test script:**
```bash
pnpm run build:npx                         # Build the standalone binary + CLI bundle
pnpm test:npm                              # Run test-npm-package.sh
```

**What `test:npm` validates:**
- NPX binary bundles and runs (shell script in repo root)
- Basic CLI invocation succeeds

**Current gaps:**
- No automated CLI argument testing
- No test coverage for error handling
- No tests for different Node versions

## Web App Testing Philosophy

**Current approach:** Minimal testing; reliance on ESLint, type checking, and manual QA

**Why:**
- Vitest configured but not in use for most of the app
- Focus on preventing type errors via TypeScript + ESLint rules
- Complex UI components verified manually during development

**Recommendation for startup code changes:**
When modifying `crates/server/src/startup.rs` or related initialization code:
1. Add `#[test]` modules covering success and failure paths
2. Test configuration precedence (env vars, defaults)
3. Test resource cleanup (file handles, database connections)
4. Run `cargo test --workspace` before committing

---

*Testing analysis: 2026-05-16*
