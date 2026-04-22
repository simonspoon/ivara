# Commands Reference

ivara provides 11 commands for capturing, querying, and managing Claude Code session data.

All commands that produce tabular output support a `--json` flag for machine-readable JSON output.

## capture

Capture a hook event from stdin JSON. This is the command registered in Claude Code hooks.

```bash
echo '{"session_id":"s1","hook_event_name":"SessionStart","cwd":"/p"}' | ivara capture
```

| Argument | Type | Description |
|----------|------|-------------|
| (stdin) | JSON | Required. The hook event JSON from Claude Code. |

No flags. Reads the full JSON from stdin, validates the event type, and stores it. Small payloads (<= 4096 bytes) are stored inline in the database; larger payloads are written to disk under `payloads/<session_id>/`.

Exits with code 0 on success, code 1 on error (invalid JSON, unknown event type).

## sessions

List sessions, ordered by most recent activity.

```bash
ivara sessions
ivara sessions --limit 5
ivara sessions --json
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON array. |
| `--limit` | integer | `20` | Maximum number of sessions to display. |

Table output columns: SESSION (ID, truncated to 38 chars), LAST SEEN (timestamp, truncated to 19 chars), DURATION (computed from first to last event), EVENTS (count), CWD.

## active

List currently-live sessions. A session is active when it has recorded a `SessionStart` event and has not yet recorded a `SessionEnd` event.

```bash
ivara active
ivara active --limit 5
ivara active --json
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON array. |
| `--limit` | integer | `20` | Maximum number of sessions to display. |

Table output columns: SESSION (ID, truncated to 38 chars), LAST SEEN (timestamp, truncated to 19 chars), DURATION (first event to last event), EVENTS (count), CWD, IDLE (human-readable elapsed since the most recent event, e.g. `12s`, `5m`, `1h 3m`), TOOL (tool name from the most recent `PreToolUse` that has no matching `PostToolUse` or `PostToolUseFailure`; empty when nothing is in flight), MODEL (opus / sonnet / haiku, from the `SessionStart` event; empty when unknown).

Rows are ordered by most-recent activity first. When no sessions are active, the command prints `No active sessions.` and exits 0.

JSON output shape (keys are snake_case):

```json
[
  {
    "session_id": "abc-123",
    "last_seen": "2026-04-22T19:22:13+00:00",
    "duration": "5m 12s",
    "event_count": 42,
    "cwd": "/proj",
    "idle": "12s",
    "tool": "Bash",
    "model": "opus"
  }
]
```

## timeline

Show chronological events for a session.

```bash
ivara timeline abc
ivara timeline abc --json
ivara timeline abc --event-type PreToolUse
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `session` | string | yes | Session ID (prefix match). |

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON array. |
| `--event-type` | string | none | Filter to a single event type. |

The session argument supports prefix matching -- provide just the first few characters of the session ID. Table output columns: ID, TIME, EVENT TYPE, DETAIL (tool name or extracted metadata field).

## show

Show full event detail including payload.

```bash
ivara show 42
ivara show 42 --json
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `event_id` | integer | yes | The event row ID. |

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON (includes inlined payload). |

Displays: Event ID, UUID, Session, Type, Timestamp, Tool (if present), Tool Use ID (if present), CWD (if present), and the full payload. For file-backed payloads, reads the file from disk and displays it. In JSON mode, the payload is inlined as a parsed `payload` field.

## query

Query and filter events across sessions.

```bash
ivara query --event-type PreToolUse
ivara query --tool Bash --since 1h
ivara query --session abc --has-error
ivara query --since 2d --until 1d --limit 100 --json
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--event-type` | string | none | Filter by event type name. |
| `--tool` | string | none | Filter by tool name. |
| `--session` | string | none | Filter by session ID (prefix match). |
| `--since` | string | none | Events after this time. Accepts ISO 8601 or relative: `30m`, `1h`, `2d`, `1w`. |
| `--until` | string | none | Events before this time. Same format as `--since`. |
| `--has-error` | bool | `false` | Only events with errors (StopFailure, PostToolUseFailure, or metadata containing "error"). |
| `--limit` | integer | `50` | Maximum results. |
| `--json` | bool | `false` | Output as JSON array. |

Relative time format: a number followed by a unit suffix.

| Suffix | Unit |
|--------|------|
| `m` | minutes |
| `h` | hours |
| `d` | days |
| `w` | weeks |

Results are ordered by timestamp descending (newest first). Table output columns: ID, TIME, EVENT TYPE, TOOL, SESSION (truncated to 12 chars).

## stats

Show statistics, either global or for a specific session.

```bash
ivara stats
ivara stats abc
ivara stats --json
ivara stats abc --json
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `session` | string | no | Session ID (prefix match). Omit for global stats. |

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON. |

Displays: scope (session ID or "global"), total events, error events, error rate (percentage), event type breakdown, tool usage breakdown, and duration (session mode only).

JSON output shape:

```json
{
  "scope": "global",
  "total_events": 150,
  "error_events": 3,
  "error_rate": 2.0,
  "event_types": [{"name": "PostToolUse", "count": 45}, ...],
  "tool_usage": [{"name": "Bash", "count": 30}, ...],
  "duration": "1h 23m"
}
```

## summary

Generate a concise session narrative.

```bash
ivara summary abc
ivara summary abc --json
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `session` | string | yes | Session ID (prefix match). |

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | `false` | Output as JSON. |

Displays: session ID, model (from SessionStart), CWD, time period with duration, event and error counts, and top 5 most-used tools.

JSON output shape:

```json
{
  "session_id": "abc-123",
  "cwd": "/project",
  "model": "opus",
  "start": "2026-03-30T10:00:00+00:00",
  "end": "2026-03-30T11:23:00+00:00",
  "duration": "1h 23m",
  "total_events": 150,
  "errors": 3,
  "top_tools": [{"tool": "Bash", "count": 30}, ...]
}
```

## prune

Delete old data from the database and payload files.

```bash
ivara prune --days 30
ivara prune --before 2026-01-01
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--before` | string | none | Delete events before this date (YYYY-MM-DD format). |
| `--days` | integer | none | Delete events older than N days. |

One of `--before` or `--days` must be provided. If both are given, `--before` takes precedence.

Prune does three things:
1. Deletes matching event rows from the database.
2. Deletes associated payload files from disk.
3. Removes orphaned sessions (sessions with no remaining events) and cleans up empty payload directories.

Reports the count of deleted events and payload files (including any file deletion errors).

## export

Export a full session as a JSON array with inlined payloads.

```bash
ivara export abc
ivara export abc > session.json
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `session` | string | yes | Session ID (prefix match). |

Outputs a JSON array to stdout where each element is an event object with an additional `payload` field containing the parsed event data. For file-backed payloads, the file content is read and inlined. For inline payloads, `metadata_json` is parsed and included.

## stream

Stream a session's events as newline-delimited JSON (JSONL). Replays every existing event for the session in timestamp order, then tails new events as they are captured. Designed for piping live Claude Code activity into other tools.

```bash
ivara stream abc
ivara stream abc | jq 'select(.event_type == "PreToolUse")'
```

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `session` | string | yes | Session ID (prefix match). |

No flags. Each stdout line is one JSON object with the same shape as a single element of `ivara export` output: all `Event` columns plus an inlined `payload` field (from the on-disk payload file, or the parsed inline `metadata_json`). Stdout is flushed after every line so consumers see events promptly.

Exit conditions:

| Trigger | Exit code |
|---------|-----------|
| A `SessionEnd` event for the session is emitted | `0` |
| SIGINT (Ctrl-C) | `0` |
| Consumer closes the pipe (BrokenPipe) | `0` |
| Unknown session prefix | `1` (error to stderr) |

New events are polled from the database at a fixed 250ms interval. The command does not support multi-session streaming, filtering flags, or replay-only mode — use `ivara export` for one-shot replay or `ivara query` / `ivara timeline` for filtering.

## Source References

- CLI definition (all commands and flags): `src/cli.rs`
- Capture: `src/commands/capture.rs`
- Sessions: `src/commands/sessions.rs`
- Active: `src/commands/active.rs`
- Timeline, show, query: `src/commands/query.rs`
- Stats, summary: `src/commands/analysis.rs`
- Prune, export: `src/commands/maintenance.rs`
- Stream: `src/commands/stream.rs`
- Query engine (filter building, time parsing): `src/query.rs`
