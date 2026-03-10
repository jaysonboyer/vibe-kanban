# Bug: MCP `update_issue` cannot pass `null` for `parent_issue_id` (un-nest sub-issues)

## Summary

The `update_issue` MCP tool cannot un-nest a sub-issue from its parent because the MCP protocol has no way to transmit a JSON `null` value for a parameter typed as `format: uuid, nullable: true`. The tool description says "Pass null to un-nest from parent" but this is impossible for MCP clients to do.

## Reproduction

1. Create an issue as a sub-issue of a parent (set `parent_issue_id`)
2. Call `update_issue` with `parent_issue_id` set to `null` to un-nest it
3. The MCP client serializes the string `"null"` which fails UUID parsing:

```
MCP error -32602: failed to deserialize parameters: UUID parsing failed:
invalid character: expected an optional prefix of `urn:uuid:` followed by
[0-9a-fA-F-], found `n` at 1
```

## Root Cause

In `crates/mcp/src/task_server/tools/remote_issues.rs` line 210:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpUpdateIssueRequest {
    // ...
    #[schemars(
        description = "Parent issue ID to set this as a subissue. Pass null to un-nest from parent."
    )]
    parent_issue_id: Option<Option<Uuid>>,  // line 210
}
```

The `Option<Option<Uuid>>` type is the correct Rust idiom for distinguishing between:
- **Field absent** (`None`) → don't change the value
- **Field explicitly `null`** (`Some(None)`) → set to NULL in the database
- **Field has a value** (`Some(Some(uuid))`) → set to that UUID

However, the `schemars::JsonSchema` derive generates a JSON Schema like:

```json
{
  "parent_issue_id": {
    "type": ["string", "null"],
    "format": "uuid"
  }
}
```

The MCP protocol (via `rmcp`) deserializes tool call arguments from JSON. When an MCP client (Claude, etc.) wants to send `null`, it passes the string `"null"` as the parameter value. The `serde` + `uuid` deserializer then tries to parse `"null"` as a UUID and fails.

### The fundamental issue

MCP tool parameters are defined by JSON Schema, but the MCP client-server protocol doesn't have a clean mechanism for clients to express "this optional field should be JSON `null`" vs "this optional field is absent." The `rmcp` framework deserializes all parameters from a JSON object, so in theory `{"parent_issue_id": null}` should work — but MCP clients (including Claude) serialize the parameter value as the string `"null"` rather than the JSON value `null`.

## Workaround Used

The REST API at `localhost:3000` (the VK CLI local proxy) accepts `PATCH /api/remote/issues/{id}` with a proper JSON body `{"parent_issue_id": null}`, so direct `curl` calls work:

```bash
curl -s -X PATCH "http://localhost:3000/api/remote/issues/{issue_id}" \
  -H "Content-Type: application/json" \
  -d '{"parent_issue_id": null}'
```

## Proposed Fix Options

### Option A: Custom deserializer for the MCP request (recommended)

Add a custom serde deserializer that handles the string `"null"` as `Some(None)` for the `parent_issue_id` field:

```rust
fn deserialize_nullable_uuid<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Accept: absent → None, null → Some(None), valid UUID string → Some(Some(uuid))
    // Also accept: the literal string "null" → Some(None)
    // ...
}
```

Then apply it:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpUpdateIssueRequest {
    // ...
    #[serde(default, deserialize_with = "deserialize_nullable_uuid")]
    parent_issue_id: Option<Option<Uuid>>,
}
```

### Option B: Use a sentinel UUID value

Accept a well-known "nil UUID" (`00000000-0000-0000-0000-000000000000`) to mean "set to null":

```rust
// In update_issue handler, before building the payload:
let parent_issue_id = parent_issue_id.map(|opt| {
    opt.and_then(|uuid| {
        if uuid.is_nil() { None } else { Some(uuid) }
    })
});
```

This is simpler but less clean — the nil UUID hack isn't self-documenting and would need to be documented in the tool description.

### Option C: Add a separate `clear_parent` boolean parameter

```rust
#[schemars(description = "Set to true to remove this issue from its parent (un-nest)")]
clear_parent: Option<bool>,
```

Then in the handler:

```rust
let parent_issue_id = if clear_parent.unwrap_or(false) {
    Some(None) // explicitly null
} else {
    parent_issue_id
};
```

This is the most MCP-friendly approach since it avoids the nullable UUID problem entirely, but adds a somewhat redundant parameter.

## Affected Files

| File | Role |
|------|------|
| `crates/mcp/src/task_server/tools/remote_issues.rs:193-211` | MCP tool request struct definition |
| `crates/api-types/src/issue.rs:63+` | `UpdateIssueRequest` API type (uses `some_if_present` custom deserializer, already handles `Option<Option<_>>` correctly for the REST API) |

## Impact

Any MCP client trying to un-nest a sub-issue from its parent via `update_issue` will hit this error. The same pattern would affect any future `Option<Option<Uuid>>` fields added to MCP tool request structs.
