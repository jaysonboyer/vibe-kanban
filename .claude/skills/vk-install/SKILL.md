---
name: vk-install
description: Build and install vibe-kanban globally from the local codebase. Use when the user wants to install, reinstall, or update their global vibe-kanban binary from source. Supports both production (release, optimized) and dev (debug, faster compile) builds. Only works in the vibe-kanban repository.
---

# Install Vibe Kanban Globally from Source

Build the full vibe-kanban application (frontend + Rust binaries) and install it globally on the system using the local codebase.

## How It Works

The `npx-cli/bin/download.js` module activates **LOCAL_DEV_MODE** when `npx-cli/dist/` exists. This means after running `pnpm run build:npx`, the global `vibe-kanban` command automatically uses the locally built binaries instead of downloading from R2.

## Workflow

### 1. Verify Working Directory

Confirm we are in the vibe-kanban repository:

```bash
pwd
ls Cargo.toml local-build.sh npx-cli/
```

If not in the right directory, inform the user and stop.

### 2. Check Prerequisites

```bash
node --version
pnpm --version
cargo --version
```

All three must be available. If any are missing, inform the user which tool needs to be installed.

### 3. Check for Uncommitted Changes

```bash
git status --short
```

Warn the user if there are staged or unstaged changes — they will be included in this build. Do not block, just inform.

### 4. Check Current Global Install

```bash
which vibe-kanban 2>/dev/null || echo "NOT INSTALLED"
vibe-kanban --version 2>/dev/null || true
npm list -g vibe-kanban 2>/dev/null || true
```

Report what version (if any) is currently installed globally.

### 5. Ask for Build Mode and Confirmation

Use AskUserQuestion to confirm before starting the build. Present the user with:
- Current branch and latest commit (`git log --oneline -1`)
- Currently installed version (from step 4)

Ask them to choose a build mode:

| Mode | Compile flags | Binary location | Estimated time |
|------|--------------|-----------------|----------------|
| **Production** | `--release` | `target/release/` | ~5-10 min first time, faster with cache |
| **Dev** | (debug) | `target/debug/` | ~1-2 min first time, faster with cache |

**When to choose each:**
- **Production** — Normal install; optimized binary, same as official releases
- **Dev** — Rapid iteration on Rust code; faster compile, larger/slower binary

Options:
- **Production build** — Optimized release binary
- **Dev build** — Debug binary, faster to compile
- **Cancel** — Abort

### 6. Install npm Dependencies

```bash
pnpm i
```

### 7. Run the Build

**If production build:**

```bash
pnpm run build:npx
```

This runs `local-build.sh` which builds the frontend, compiles three Rust binaries with `--release`, and packages them into `npx-cli/dist/<platform>/`.

**If dev build:**

Run each step manually so debug binaries land in the right place:

```bash
# Detect platform (matches local-build.sh logic)
PLATFORM=$(node -e "
  const os = require('os');
  const p = process.platform === 'darwin' ? 'macos' : process.platform;
  const a = os.arch() === 'arm64' ? 'arm64' : 'x64';
  console.log(p + '-' + a);
")

# Clean and set up dist
rm -rf npx-cli/dist
mkdir -p "npx-cli/dist/$PLATFORM"

# Build frontend (same as production)
(cd frontend && npm run build)

# Build Rust binaries in debug mode (no --release)
export VK_SHARED_API_BASE="https://api.vibekanban.com"
export VITE_VK_SHARED_API_BASE="https://api.vibekanban.com"
cargo build --bin server --manifest-path Cargo.toml
cargo build --bin mcp_task_server --manifest-path Cargo.toml
cargo build --bin review --manifest-path Cargo.toml

# Package into dist (same zip structure local-build.sh uses)
cp target/debug/server vibe-kanban
zip -q vibe-kanban.zip vibe-kanban && rm vibe-kanban
mv vibe-kanban.zip "npx-cli/dist/$PLATFORM/"

cp target/debug/mcp_task_server vibe-kanban-mcp
zip -q vibe-kanban-mcp.zip vibe-kanban-mcp && rm vibe-kanban-mcp
mv vibe-kanban-mcp.zip "npx-cli/dist/$PLATFORM/"

cp target/debug/review vibe-kanban-review
zip -q vibe-kanban-review.zip vibe-kanban-review && rm vibe-kanban-review
mv vibe-kanban-review.zip "npx-cli/dist/$PLATFORM/"
```

Show live output so the user can follow progress.

### 8. Install Globally via Symlink

Create or update the symlink at `~/.local/bin/vibe-kanban` pointing directly to the workspace `cli.js`. This approach is stable across nvm version switches and doesn't depend on npm global install paths.

**IMPORTANT: Check if the symlink already exists before creating it.** If `~/.local/bin/vibe-kanban` already points to the workspace `cli.js`, skip this step entirely — the build already updated the binaries it uses.

```bash
# Check first
ls -la ~/.local/bin/vibe-kanban 2>/dev/null || echo "NOT FOUND"

# Only create if missing or pointing elsewhere
mkdir -p ~/.local/bin
rm -f ~/.local/bin/vibe-kanban
ln -s /Users/jayboyer/workspace/vibe-kanban/npx-cli/bin/cli.js ~/.local/bin/vibe-kanban
```

The symlink works because `cli.js` has a `#!/usr/bin/env node` shebang and activates LOCAL_DEV_MODE automatically when `npx-cli/dist/` exists.

**Note:** If there's an old npm global install, remove it first:
```bash
npm uninstall -g vibe-kanban 2>/dev/null || true
```

### 9. Verify Installation

```bash
ls -la ~/.local/bin/vibe-kanban
which vibe-kanban
```

Confirm the symlink points to the workspace `cli.js`.

### 10. Report Summary

Provide a clear summary:
- ✅ Build successful (`production` or `dev` — whichever was chosen)
- 🔗 Symlink: `~/.local/bin/vibe-kanban` → `/Users/jayboyer/workspace/vibe-kanban/npx-cli/bin/cli.js`
- 🔖 Version: `<version from package.json>`
- 💡 To run: `vibe-kanban`
- 💡 MCP configs (`~/.claude.json`, `~/Library/Application Support/Claude/claude_desktop_config.json`) should use `node npx-cli/bin/cli.js --mcp` instead of `npx -y vibe-kanban@latest --mcp`
- ⚠️ If dev build: remind the user the binary is unoptimized; run `vk-install` with production mode before any real use

## Error Handling

### Build Failures

**Frontend build fails:**
- Run `pnpm run check` to surface TypeScript errors
- Check Node/pnpm versions match project requirements

**Rust build fails:**
- Run `cargo check` to see compiler errors
- Ensure Rust toolchain is up to date: `rustup update`
- SQLx offline mode issues: run `pnpm run prepare-db` first

### Symlink Creation Fails (Permissions)

If `ln -s` fails with permission denied:
```bash
mkdir -p ~/.local/bin
chmod u+w ~/.local/bin
ln -s /Users/jayboyer/workspace/vibe-kanban/npx-cli/bin/cli.js ~/.local/bin/vibe-kanban
```

### Stale Binaries After Rebuild

The symlink always points to the latest build. After `pnpm run build:npx` completes, the symlink automatically uses the new binaries — no reinstall needed.

## Uninstalling

To remove the symlink:
```bash
rm ~/.local/bin/vibe-kanban
```

## Notes

- The `npx-cli/dist/` directory must exist for LOCAL_DEV_MODE to activate. If you delete it, the global binary will try to download from R2 instead.
- `VIBE_KANBAN_LOCAL=1` env var also forces LOCAL_DEV_MODE without needing the `dist/` directory.
- The build sets `VK_SHARED_API_BASE=https://api.vibekanban.com` for remote features.
- Platform is auto-detected by `local-build.sh` (outputs to `npx-cli/dist/macos-arm64/` on Apple Silicon).
