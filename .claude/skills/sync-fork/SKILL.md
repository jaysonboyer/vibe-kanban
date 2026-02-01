---
name: sync-fork
description: Sync fork (jaysonboyer/vibe-kanban) with upstream parent (BloopAI/vibe-kanban). Use when the user wants to update their fork with latest changes from upstream, sync with parent repository, pull upstream changes, or rebase/merge from BloopAI/vibe-kanban.
---

# Sync Fork with Upstream

Synchronize the fork (jaysonboyer/vibe-kanban) with the upstream parent repository (BloopAI/vibe-kanban).

## Workflow

### 1. Verify Current State

Check repository status and remote configuration:

```bash
git branch --show-current
git status
git remote -v
```

**Requirements:**
- Clean working directory (no uncommitted changes)
- Both `origin` (git@github.com:jaysonboyer/vibe-kanban.git) and `upstream` (https://github.com/BloopAI/vibe-kanban.git) remotes configured

**If remotes missing:**
```bash
git remote add upstream https://github.com/BloopAI/vibe-kanban.git
```

### 2. Ask User for Strategy

Use AskUserQuestion tool to ask which sync strategy:

**Merge (Recommended)**
- Preserves complete history with merge commits
- Safer, easier to undo
- Command: `git merge upstream/main`

**Rebase**
- Creates linear history
- Replays commits on top of upstream
- Command: `git rebase upstream/main`

### 3. Handle Uncommitted Changes

If uncommitted changes exist:
- Offer to stash: `git stash push -m "Auto-stash before fork sync"`
- Or ask user to commit first
- Remember to reapply stash after sync

### 4. Fetch and Preview Changes

```bash
git fetch upstream
git log --oneline main..upstream/main
```

Report to user:
- Number of commits ahead/behind
- Summary of changes from commit messages

### 5. Switch to Main Branch

```bash
git checkout main
```

### 6. Perform Sync

**Merge strategy:**
```bash
git merge upstream/main
```

**Rebase strategy:**
```bash
git rebase upstream/main
```

### 7. Handle Conflicts

If conflicts occur:

1. List conflicted files: `git status`
2. Inform user which files have conflicts
3. Provide guidance:
   ```
   To resolve:
   1. Edit conflicted files (look for <<<<<<, ======, >>>>>>)
   2. Stage resolved files: git add <file>
   3. Continue: git merge --continue (or git rebase --continue)

   To abort: git merge --abort (or git rebase --abort)
   ```
4. Wait for user to resolve before continuing

### 8. Push to Fork

```bash
git push origin main
```

**Never use --force on main** unless explicitly requested.

### 9. Restore Stashed Changes

If stashed earlier:
```bash
git stash pop
```

### 10. Report Summary

Provide clear summary:
- ✅ Commits synced (count)
- 📝 Major changes (from commit messages)
- 🔄 Current status (up to date/ahead/behind)
- 💡 Next steps (e.g., rebase feature branches)

## Error Handling

### Diverged Branches

If main has local commits not in upstream, warn user:
- Merge: Creates merge commit (safe)
- Rebase: Replays commits (may conflict)

### Network Issues

If fetch fails:
- Check internet connection
- Verify GitHub access
- Try HTTPS for upstream if SSH fails

## Post-Sync

Recommend updating feature branches:
```bash
git checkout feature-branch
git rebase main
```

## Safety Checks

Before destructive operations:
1. ✅ Clean working directory OR stashed
2. ✅ Remotes correctly configured
3. ✅ Explain what will happen
4. ✅ Never force-push to main without confirmation
