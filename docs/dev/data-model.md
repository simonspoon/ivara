# Data Model

This document describes the database schema, Rust type definitions, and payload storage strategy used by ivara.

## Database Schema

ivara uses SQLite with three tables. Schema DDL is in `src/db.rs` (`initialize()` function) and runs on every connection via `CREATE TABLE IF NOT EXISTS` — there is no migration framework, so new tables are picked up automatically on the next connect.

### events

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `INTEGER` | `PRIMARY KEY AUTOINCREMENT` | Auto-incrementing row ID. |
| `event_uuid` | `TEXT` | `NOT NULL UNIQUE` | UUIDv4 generated at capture time. |
| `session_id` | `TEXT` | `NOT NULL` | Claude Code session identifier. |
| `event_type` | `TEXT` | `NOT NULL` | One of 25 `EventType` variant names. |
| `timestamp` | `TEXT` | `NOT NULL` | RFC 3339 timestamp (capture time, not hook time). |
| `tool_name` | `TEXT` | nullable | Tool name for tool-related events (e.g., "Bash", "Read"). |
| `tool_use_id` | `TEXT` | nullable | Tool use identifier for correlating Pre/PostToolUse pairs. |
| `cwd` | `TEXT` | nullable | Working directory at event time. |
| `payload_path` | `TEXT` | nullable | Absolute path to file-backed payload (only if >4096 bytes). |
| `metadata_json` | `TEXT` | nullable | Inline JSON -- full payload if small, or extracted metadata if large. |

### Indexes on events

| Index | Column(s) |
|-------|-----------|
| `idx_events_session` | `session_id` |
| `idx_events_type` | `event_type` |
| `idx_events_timestamp` | `timestamp` |
| `idx_events_tool_name` | `tool_name` |
| `idx_events_tool_use_id` | `tool_use_id` |
| `idx_events_session_type` | `session_id, event_type` |

The composite `idx_events_session_type` is required for `list_active_sessions` and `in_flight_tool` in `src/db.rs`, which both filter events by `(session_id, event_type)`. Without it, the planner picks `idx_events_type` and re-scans every SessionStart/SessionEnd row per session -- on a 235k-event DB this turned `ivara active` into a ~129s outer query. With the composite index both subqueries become O(log n) lookups (outer query 129s -> 0.03s; `ivara active --json --limit 200` runs in ~41ms). When adding new query paths that filter on multiple event columns, check the EXPLAIN QUERY PLAN before relying on single-column indexes.

### sessions

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `session_id` | `TEXT` | `PRIMARY KEY` | Claude Code session identifier. |
| `first_seen` | `TEXT` | `NOT NULL` | RFC 3339 timestamp of earliest event. |
| `last_seen` | `TEXT` | `NOT NULL` | RFC 3339 timestamp of latest event. |
| `event_count` | `INTEGER` | `NOT NULL DEFAULT 0` | Running count of events in this session. |
| `cwd` | `TEXT` | nullable | Last known working directory. |
| `model` | `TEXT` | nullable | Model name from SessionStart event (e.g., "opus"). |

The sessions table is maintained via upsert (`INSERT ... ON CONFLICT DO UPDATE`). On each new event, `last_seen` is updated to the max of existing and new timestamp, `event_count` is incremented, and `cwd`/`model` are filled via `COALESCE` (first non-null wins).

### session_usage

Aggregated token usage per session, derived by parsing the session's transcript JSONL file. Hook events carry no token counts, so this table is the only place token usage is recorded.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `session_id` | `TEXT` | `PRIMARY KEY` | Claude Code session identifier. |
| `model` | `TEXT` | nullable | Model driving the most API calls in the transcript. |
| `input_tokens` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `input_tokens` across all assistant messages. |
| `output_tokens` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `output_tokens`. |
| `cache_creation_tokens` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `cache_creation_input_tokens`. |
| `cache_read_tokens` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `cache_read_input_tokens`. |
| `web_search_requests` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `server_tool_use.web_search_requests`. |
| `web_fetch_requests` | `INTEGER` | `NOT NULL DEFAULT 0` | Sum of `server_tool_use.web_fetch_requests`. |
| `api_calls` | `INTEGER` | `NOT NULL DEFAULT 0` | Count of assistant messages (API requests) counted. |
| `parsed_at` | `TEXT` | `NOT NULL` | RFC 3339 timestamp of the parse that produced this row. |

Rows are written via `INSERT OR REPLACE` (`db::upsert_session_usage`) — a full overwrite, because re-parsing a transcript always yields the complete picture. Population happens two ways:

1. **Automatically** — `commands::capture` parses the transcript when a `SessionEnd` event arrives. This is best-effort: a missing or malformed transcript never fails the capture.
2. **On demand** — `ivara backfill-usage` (`commands::usage`) parses transcripts for historical sessions or ones whose `SessionEnd` never fired.

Each value is a sum over per-API-call `usage` blocks. `input_tokens` therefore intentionally counts the same context repeatedly — every API call is billed for its full input — so it is a billing figure, not the context size. `prune` deletes `session_usage` rows whose session no longer has any events.

## Rust Types

Core types are defined in `src/events.rs`; the token-usage type (`SessionUsage`) is in `src/usage.rs`.

### EventType (enum, 25 variants)

```rust
pub enum EventType {
    SessionStart,         // Session lifecycle
    SessionEnd,
    CwdChanged,           // Environment
    ConfigChange,
    InstructionsLoaded,
    UserPromptSubmit,     // User interaction
    Stop,                 // Completion
    StopFailure,
    PreToolUse,           // Tool lifecycle
    PostToolUse,
    PostToolUseFailure,
    PermissionRequest,
    SubagentStart,        // Agent lifecycle
    SubagentStop,
    TaskCreated,          // Task management
    TaskCompleted,
    TeammateIdle,
    FileChanged,          // Filesystem
    WorktreeCreate,
    WorktreeRemove,
    PreCompact,           // Context compaction
    PostCompact,
    Elicitation,          // MCP elicitation
    ElicitationResult,
    Notification,         // Notifications
}
```

Conversion methods:
- `EventType::from_hook_name(&str) -> Option<EventType>` -- maps Claude Code hook name string to enum variant.
- `EventType::as_str(&self) -> &'static str` -- returns the variant name as a string.
- `Display` trait -- same as `as_str()`.

### Event (struct)

Represents a stored event row from the database.

```rust
pub struct Event {
    pub id: i64,                         // SQLite row ID
    pub event_uuid: String,              // UUIDv4
    pub session_id: String,
    pub event_type: String,              // EventType variant name
    pub timestamp: String,               // RFC 3339
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub cwd: Option<String>,
    pub payload_path: Option<String>,    // absolute path if file-backed
    pub metadata_json: Option<String>,   // inline JSON
}
```

### Session (struct)

Represents a session summary row.

```rust
pub struct Session {
    pub session_id: String,
    pub first_seen: String,     // RFC 3339
    pub last_seen: String,      // RFC 3339
    pub event_count: i64,
    pub cwd: Option<String>,
    pub model: Option<String>,
}
```

### HookInput (struct)

The raw JSON blob that Claude Code sends via stdin. All fields except `session_id` and `hook_event_name` are optional (using `#[serde(default)]`).

| Field Group | Fields |
|-------------|--------|
| **Required** | `session_id`, `hook_event_name` |
| **Common** | `transcript_path`, `cwd`, `permission_mode` |
| **Tool events** | `tool_name`, `tool_input` (Value), `tool_use_id`, `tool_response` (Value) |
| **Session events** | `source`, `model`, `reason` |
| **Stop events** | `stop_hook_active` (bool), `last_assistant_message` |
| **StopFailure** | `error`, `error_details`, `is_interrupt` (bool) |
| **User prompt** | `prompt` |
| **Subagent events** | `agent_id`, `agent_type`, `agent_transcript_path` |
| **Task events** | `task_id`, `task_subject`, `task_description`, `teammate_name`, `team_name` |
| **File events** | `file_path`, `change_type` |
| **Compaction events** | `compaction_trigger` |
| **Elicitation events** | `mcp_server_name`, `fields` (Vec\<Value\>) |
| **ElicitationResult** | `result_action`, `result_content` (Value) |
| **Notification** | `message`, `title`, `notification_type` |

Key methods on `HookInput`:
- `from_json(&str) -> Result<Self>` -- deserialize from stdin JSON.
- `event_type() -> Result<EventType>` -- extract and validate the event type.
- `metadata() -> serde_json::Value` -- build a metadata object with only small fields (excludes `tool_input`, `tool_response`, `last_assistant_message`).
- `payload_size() -> usize` -- estimate serialized size for threshold comparison.

### SessionUsage (struct)

Aggregated token usage for one session. Defined in `src/usage.rs`. Mirrors the `session_usage` table columns (minus `session_id` and `parsed_at`).

```rust
pub struct SessionUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub web_search_requests: i64,
    pub web_fetch_requests: i64,
    pub api_calls: i64,
    pub model: Option<String>,
}
```

- `parse_transcript(&Path) -> Result<SessionUsage>` -- read a transcript JSONL file and sum usage across every assistant message. Non-assistant entries, synthetic entries (model `<synthetic>`), duplicate entry uuids, and unparseable lines are skipped.
- `total_tokens(&self) -> i64` -- `input + output + cache_creation + cache_read`.

## Payload Storage Strategy

Events are stored differently based on size, controlled by `PAYLOAD_THRESHOLD` (4096 bytes) in `src/storage.rs`.

| Condition | `metadata_json` column | `payload_path` column | On-disk file |
|-----------|------------------------|-----------------------|--------------|
| payload <= 4096 bytes | Full stdin JSON | `NULL` | None |
| payload > 4096 bytes | `HookInput::metadata()` (small fields only) | Absolute path to JSON file | `payloads/<session_id>/<event_uuid>.json` |

File layout on disk:

```
~/.ivara/
  ivara.db                              # SQLite database
  payloads/
    <session_id>/
      <event_uuid>.json                 # Full stdin JSON for large events
```

The `metadata()` method extracts only small scalar fields from `HookInput`, excluding the large fields (`tool_input`, `tool_response`, `last_assistant_message`) that typically push payloads over the threshold. This keeps `metadata_json` queryable for all events regardless of size.

## Source References

- Schema DDL and DB operations: `src/db.rs`
- Type definitions and HookInput: `src/events.rs`
- Payload storage functions: `src/storage.rs`
- Capture pipeline (where storage decisions happen): `src/commands/capture.rs`
- Transcript token-usage parser and `SessionUsage`: `src/usage.rs`
- Backfill command: `src/commands/usage.rs`
