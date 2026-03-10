# Vibe Kanban REST API

All routes are prefixed with `/api`. Responses follow the wrapper:
```json
{ "success": bool, "data": <T>, "error": "string | null" }
```

WebSocket endpoints accept `ws://` connections. Auth is validated via origin middleware.

---

## Health

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Returns `"OK"` |

---

## Config & System

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/info` | Full system config, login status, profiles, environment |
| PUT | `/api/config` | Update app config |
| GET | `/api/sounds/{sound}` | Fetch audio file (wav) |
| GET | `/api/mcp-config` | Get MCP server config |
| POST | `/api/mcp-config` | Update MCP server config |
| GET | `/api/profiles` | Get executor profiles |
| PUT | `/api/profiles` | Update executor profiles (raw JSON body) |
| GET | `/api/editors/check-availability` | Check if editor is installed |
| GET | `/api/agents/check-availability` | Check agent availability |
| GET | `/api/agents/preset-options` | Get agent preset options |
| WS | `/api/agents/discovered-options/ws` | Stream discovered agent options |

### `GET /api/info` → `UserSystemInfo`
```
config: Config
analytics_user_id: String
login_status: LoggedOut | LoggedIn
profiles: ExecutorConfigs
environment: { os_type, os_version, os_architecture, bitness }
capabilities: HashMap<String, Vec<BaseAgentCapability>>
shared_api_base?: String
preview_proxy_port?: u16
```

### `PUT /api/config` body: `Config`
```
disclaimer_acknowledged: bool
onboarding_acknowledged: bool
analytics_enabled: bool
git_branch_prefix: String
executor_profile: String
editor: EditorConfig
```

### `GET /api/mcp-config` query
```
executor: BaseCodingAgent  (required)
```

### `POST /api/mcp-config` query + body
```
query:  executor: BaseCodingAgent  (required)
body:   servers: HashMap<String, Value>
```

### `GET /api/editors/check-availability` query
```
editor_type: EditorType  (required)
```

### `GET /api/agents/check-availability` query
```
executor: BaseCodingAgent  (required)
```

### `GET /api/agents/preset-options` query
```
executor: BaseCodingAgent  (required)
variant?: String
```

### `WS /api/agents/discovered-options/ws` query
```
executor: BaseCodingAgent  (required)
workspace_id?: Uuid
repo_id?: Uuid
```

---

## Authentication & OAuth

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/handoff/init` | Start OAuth handoff |
| GET | `/api/auth/handoff/complete` | Complete OAuth handoff |
| POST | `/api/auth/logout` | Logout (204) |
| GET | `/api/auth/status` | Auth status + user profile |
| GET | `/api/auth/token` | Get (or refresh) access token |
| GET | `/api/auth/user` | Get current user ID |

### `POST /api/auth/handoff/init` body
```
provider: String
return_to: String
```
→ `{ handoff_id: Uuid, authorize_url: String }`

### `GET /api/auth/handoff/complete` query
```
handoff_id: Uuid  (required)
app_code?: String
error?: String
```
→ HTML redirect response

### `GET /api/auth/status` → `StatusResponse`
```
logged_in: bool
profile?: UserProfile
degraded?: String
```

### `GET /api/auth/token` → `TokenResponse`
```
access_token: String
expires_at?: DateTime<Utc>
```

---

## Organizations

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/organizations` | List orgs |
| POST | `/api/organizations` | Create org |
| GET | `/api/organizations/{id}` | Get org |
| PATCH | `/api/organizations/{id}` | Update org name |
| DELETE | `/api/organizations/{id}` | Delete org (204) |
| POST | `/api/organizations/{org_id}/invitations` | Invite member |
| GET | `/api/organizations/{org_id}/invitations` | List invitations |
| POST | `/api/organizations/{org_id}/invitations/revoke` | Revoke invitation |
| GET | `/api/invitations/{token}` | Get invitation by token |
| POST | `/api/invitations/{token}/accept` | Accept invitation |
| GET | `/api/organizations/{org_id}/members` | List members |
| DELETE | `/api/organizations/{org_id}/members/{user_id}` | Remove member (204) |
| PATCH | `/api/organizations/{org_id}/members/{user_id}/role` | Update member role |

### `POST /api/organizations` body
```
name: String
slug: String
```

### `PATCH /api/organizations/{id}` body
```
name: String
```

### `POST /api/organizations/{org_id}/invitations` body
```
email: String
role: Owner | Manager | Member
```

### `POST /api/organizations/{org_id}/invitations/revoke` body
```
invitation_id: Uuid
```

### `PATCH /api/organizations/{org_id}/members/{user_id}/role` body
```
role: Owner | Manager | Member
```

---

## Remote API — Projects

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/workspaces/by-local-id/{id}` | Get remote workspace by local ID |
| GET | `/api/remote/projects` | List projects in org |
| GET | `/api/remote/projects/{project_id}` | Get project |
| GET | `/api/remote/project-statuses` | List statuses for project |

### `GET /api/remote/projects` query
```
organization_id: Uuid  (required)
```

### `GET /api/remote/project-statuses` query
```
project_id: Uuid  (required)
```

---

## Remote API — Issues

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/issues` | List issues |
| POST | `/api/remote/issues` | Create issue |
| GET | `/api/remote/issues/{issue_id}` | Get issue |
| PATCH | `/api/remote/issues/{issue_id}` | Update issue |
| DELETE | `/api/remote/issues/{issue_id}` | Delete issue |

### `GET /api/remote/issues` query
```
project_id: Uuid  (required)
```

### `POST /api/remote/issues` body
```
id?: Uuid
project_id: Uuid
status_id: Uuid
title: String
description?: String
priority?: Urgent | High | Medium | Low
start_date?: DateTime<Utc>
target_date?: DateTime<Utc>
completed_at?: DateTime<Utc>
sort_order: f64
parent_issue_id?: Uuid
extension_metadata: Value
```

### `PATCH /api/remote/issues/{issue_id}` body (all optional)
```
status_id?: Uuid
title?: String
description?: String | null
priority?: IssuePriority | null
start_date?: DateTime<Utc> | null
target_date?: DateTime<Utc> | null
completed_at?: DateTime<Utc> | null
sort_order?: f64
parent_issue_id?: Uuid | null
extension_metadata?: Value
```

### Issue type
```
id: Uuid
project_id: Uuid
simple_id: String          (e.g. "PROJ-42")
title: String
description?: String
priority?: Urgent | High | Medium | Low
status_id: Uuid
start_date?: DateTime<Utc>
target_date?: DateTime<Utc>
completed_at?: DateTime<Utc>
sort_order: f64
parent_issue_id?: Uuid
created_at: DateTime<Utc>
updated_at: DateTime<Utc>
```

---

## Remote API — Issue Assignees

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/issue-assignees` | List assignees |
| GET | `/api/remote/issue-assignees/{id}` | Get assignee |
| POST | `/api/remote/issue-assignees` | Add assignee |
| DELETE | `/api/remote/issue-assignees/{id}` | Remove assignee |

### `GET /api/remote/issue-assignees` query
```
issue_id: Uuid  (required)
```

### `POST /api/remote/issue-assignees` body
```
id?: Uuid
issue_id: Uuid
user_id: Uuid
```

---

## Remote API — Issue Relationships

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/issue-relationships` | List relationships |
| POST | `/api/remote/issue-relationships` | Create relationship |
| DELETE | `/api/remote/issue-relationships/{id}` | Delete relationship |

### `GET /api/remote/issue-relationships` query
```
issue_id: Uuid  (required)
```

### `POST /api/remote/issue-relationships` body
```
id?: Uuid
issue_id: Uuid
related_issue_id: Uuid
relationship_type: Blocking | Related | HasDuplicate
```

---

## Remote API — Tags

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/tags` | List project tags |
| GET | `/api/remote/tags/{tag_id}` | Get tag |
| GET | `/api/remote/issue-tags` | List tags on issue |
| GET | `/api/remote/issue-tags/{id}` | Get issue tag |
| POST | `/api/remote/issue-tags` | Add tag to issue |
| DELETE | `/api/remote/issue-tags/{id}` | Remove tag from issue |

### `GET /api/remote/tags` query
```
project_id: Uuid  (required)
```

### `GET /api/remote/issue-tags` query
```
issue_id: Uuid  (required)
```

### `POST /api/remote/issue-tags` body
```
id?: Uuid
issue_id: Uuid
tag_id: Uuid
```

---

## Remote API — Pull Requests

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/remote/pull-requests` | List PRs for issue |

### `GET /api/remote/pull-requests` query
```
issue_id: Uuid  (required)
```

---

## Local Tags

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/tags` | List local tags |
| POST | `/api/tags` | Create tag |
| PUT | `/api/tags/{tag_id}` | Update tag |
| DELETE | `/api/tags/{tag_id}` | Delete tag |

### `GET /api/tags` query
```
search?: String
```

### `POST /api/tags` body
```
tag_name: String
tag_color?: String
```

### `PUT /api/tags/{tag_id}` body
```
tag_name?: String
tag_color?: String
```

---

## Repositories

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/repo/register` | Register existing repo |
| POST | `/api/repo/init` | Init new repo |
| GET | `/api/repo/search` | Search repos |
| POST | `/api/repo/batch` | Get multiple repos by IDs |
| GET | `/api/repo/{id}` | Get repo |
| GET | `/api/repo/{id}/remote-info` | Get remote info |
| GET | `/api/repo/{id}/branches` | List branches |
| GET | `/api/repo/{id}/local-branches` | List local branches |
| POST | `/api/repo/{id}/open-editor` | Open in editor |
| POST | `/api/repo/{id}/fetch` | Fetch from remote |
| POST | `/api/repo/{id}/open-github` | Open on GitHub |

---

## Filesystem

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/filesystem/directory` | List directory contents |
| GET | `/api/filesystem/git-repos` | Discover git repos |

---

## Containers

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/containers/info` | Map container ref → workspace ID |
| GET | `/api/containers/attempt-context` | Get workspace context |

### `GET /api/containers/info` query
```
container_ref: String  (required)
```

---

## Task Attempts (Workspaces)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/task-attempts` | List all workspaces |
| POST | `/api/task-attempts/create-and-start` | Create + start workspace |
| POST | `/api/task-attempts/from-pr` | Create workspace from PR |
| WS | `/api/task-attempts/stream/ws` | Stream workspace events |
| POST | `/api/task-attempts/summary` | Batch fetch summaries |
| GET | `/api/task-attempts/{id}` | Get workspace |
| PUT | `/api/task-attempts/{id}` | Update workspace metadata |
| DELETE | `/api/task-attempts/{id}` | Delete workspace (async, 202) |
| POST | `/api/task-attempts/{id}/run-agent-setup` | Run agent setup |
| POST | `/api/task-attempts/{id}/gh-cli-setup` | GitHub CLI setup |
| POST | `/api/task-attempts/{id}/run-setup-script` | Run setup script |
| POST | `/api/task-attempts/{id}/run-cleanup-script` | Run cleanup script |
| POST | `/api/task-attempts/{id}/run-archive-script` | Run archive script |
| POST | `/api/task-attempts/{id}/start-dev-server` | Start dev server |
| POST | `/api/task-attempts/{id}/stop` | Stop execution |
| GET | `/api/task-attempts/{id}/branch-status` | Git status for all repos |
| POST | `/api/task-attempts/{id}/change-target-branch` | Change target branch |
| POST | `/api/task-attempts/{id}/rename-branch` | Rename workspace branch |
| POST | `/api/task-attempts/{id}/merge` | Merge into target |
| POST | `/api/task-attempts/{id}/push` | Push branch |
| POST | `/api/task-attempts/{id}/push/force` | Force push branch |
| POST | `/api/task-attempts/{id}/rebase` | Rebase onto target |
| POST | `/api/task-attempts/{id}/rebase/continue` | Continue rebase |
| POST | `/api/task-attempts/{id}/conflicts/abort` | Abort merge/rebase |
| POST | `/api/task-attempts/{id}/open-editor` | Open in editor |
| GET | `/api/task-attempts/{id}/repos` | List repos in workspace |
| GET | `/api/task-attempts/{id}/first-message` | Get initial user prompt |
| WS | `/api/task-attempts/{id}/diff/ws` | Stream diff changes |
| PUT | `/api/task-attempts/{id}/mark-seen` | Mark AI turns as seen |
| POST | `/api/task-attempts/{id}/link` | Link to remote issue |
| POST | `/api/task-attempts/{id}/unlink` | Unlink from remote issue |
| POST | `/api/task-attempts/{id}/pr` | Create PR |
| POST | `/api/task-attempts/{id}/pr/attach` | Attach existing PR |
| GET | `/api/task-attempts/{id}/pr/comments` | Get PR comments |

### `POST /api/task-attempts/create-and-start` body
```
name?: String
repos: Vec<WorkspaceRepoInput>
linked_issue?: LinkedIssueInfo
executor_config: ExecutorConfig
prompt: String
image_ids?: Vec<Uuid>
```

### `PUT /api/task-attempts/{id}` body
```
archived?: bool
pinned?: bool
name?: String
```

### `DELETE /api/task-attempts/{id}` query
```
delete_remote?: bool   (default: false)
delete_branches?: bool (default: false)
```

### `WS /api/task-attempts/stream/ws` query
```
archived?: bool
limit?: i64
```

### `GET /api/task-attempts/{id}/branch-status` → `Vec<RepoBranchStatus>`
```
repo_id: Uuid
repo_name: String
commits_ahead?: usize
commits_behind?: usize
has_uncommitted_changes?: bool
conflicted_files: Vec<String>
merges: Vec<Merge>
is_rebase_in_progress: bool
is_target_remote: bool
```

### `POST /api/task-attempts/{id}/change-target-branch` body
```
repo_id: Uuid
new_target_branch: String
```

### `POST /api/task-attempts/{id}/rename-branch` body
```
new_branch_name: String
```

### `POST /api/task-attempts/{id}/merge` body
```
repo_id: Uuid
```

### `POST /api/task-attempts/{id}/push` body
```
repo_id: Uuid
```

### `POST /api/task-attempts/{id}/rebase` body
```
repo_id: Uuid
old_base_branch?: String
new_base_branch?: String
```

### `POST /api/task-attempts/{id}/rebase/continue` body
```
repo_id: Uuid
```

### `POST /api/task-attempts/{id}/conflicts/abort` body
```
repo_id: Uuid
```

### `POST /api/task-attempts/{id}/open-editor` body
```
editor_type?: String
file_path?: String
```

### `POST /api/task-attempts/{id}/link` body
```
project_id: Uuid
issue_id: Uuid
```

### `WS /api/task-attempts/{id}/diff/ws` query
```
stats_only?: bool  (default: false)
```

---

## Execution Processes

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/execution-processes/{id}` | Get process |
| POST | `/api/execution-processes/{id}/stop` | Stop process |
| GET | `/api/execution-processes/{id}/repo-states` | Get repo states |
| WS | `/api/execution-processes/{id}/raw-logs/ws` | Stream raw logs |
| WS | `/api/execution-processes/{id}/normalized-logs/ws` | Stream normalized logs |
| WS | `/api/execution-processes/stream/session/ws` | Stream processes for session |

### `GET /api/execution-processes/{id}` → `ExecutionProcess`
```
id: Uuid
workspace_id: Uuid
status: Running | Completed | Killed | Failed
started_at: DateTime<Utc>
completed_at?: DateTime<Utc>
```

### `WS /api/execution-processes/stream/session/ws` query
```
session_id: Uuid        (required)
show_soft_deleted?: bool (default: false)
```

---

## Sessions

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sessions` | List sessions |
| POST | `/api/sessions` | Create session |
| GET | `/api/sessions/{id}` | Get session |
| POST | `/api/sessions/{id}/follow-up` | Continue session |
| POST | `/api/sessions/{id}/reset` | Reset session |
| POST | `/api/sessions/{id}/review` | Start review |
| GET | `/api/sessions/queue/{id}` | Get queue info |
| POST | `/api/sessions/queue/{id}/next` | Get next queue item |

---

## Approvals

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/approvals/{id}/respond` | Respond to approval |

### `POST /api/approvals/{id}/respond` body
```
status: Approved | Rejected
```

---

## Search

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/search` | Search files across repos |

### `GET /api/search` query
```
q: String                   (required)
mode?: Fuzzy | ...           (default: Fuzzy)
repo_ids: String            (required, comma-separated Uuids)
```

---

## Scratch (Temporary Storage)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/scratch` | List all scratch items |
| GET | `/api/scratch/{scratch_type}/{id}` | Get item |
| POST | `/api/scratch/{scratch_type}/{id}` | Create item |
| PUT | `/api/scratch/{scratch_type}/{id}` | Upsert item |
| DELETE | `/api/scratch/{scratch_type}/{id}` | Delete item |
| WS | `/api/scratch/{scratch_type}/{id}/stream/ws` | Stream changes |

### `POST /api/scratch/{scratch_type}/{id}` body
```
payload: ScratchPayload
```

---

## Events

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/events` | Get events |

---

## Terminal

| Method | Path | Description |
|--------|------|-------------|
| WS | `/api/terminal/ws` | Interactive terminal |

---

## Migration

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/migration/status` | Get DB migration status |

---

## Images

| Method | Path | Description |
|--------|------|-------------|
| (various) | `/api/images/...` | Image management |

---

## Common Response Wrapper

```json
{
  "success": true,
  "data": { ... },
  "error": null
}
```

Error responses:
```json
{
  "success": false,
  "data": null,
  "error": "error message"
}
```

## HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | OK |
| 201 | Created |
| 202 | Accepted (async operation started) |
| 204 | No Content |
| 400 | Bad Request |
| 401 | Unauthorized |
| 404 | Not Found |
| 409 | Conflict |
| 500 | Internal Server Error |
