# Architecture

This document describes the internal module structure, data flow, and key abstractions in ivara.

## Module Overview

ivara is a single-crate Rust CLI. All source lives under `src/`.

| Module | File | Responsibility |
|--------|------|----------------|
| `cli` | `src/cli.rs` | CLI definition via clap derive. Defines `Cli` struct and `Commands` enum (9 variants). |
| `commands` | `src/commands/mod.rs` | Command implementations, split into submodules. |
| `commands::capture` | `src/commands/capture.rs` | Stdin JSON ingestion pipeline. |
| `commands::sessions` | `src/commands/sessions.rs` | List sessions with duration and event count. |
| `commands::query` | `src/commands/query.rs` | Timeline, show, and filtered query output. |
| `commands::analysis` | `src/commands/analysis.rs` | Stats and summary generation. |
| `commands::maintenance` | `src/commands/maintenance.rs` | Prune old data and export sessions. |
| `commands::usage` | `src/commands/usage.rs` | `backfill-usage` — parse transcripts to fill the `session_usage` table. |
| `db` | `src/db.rs` | SQLite connection, schema DDL, all database operations. |
| `events` | `src/events.rs` | Type definitions: `EventType` (25 variants), `Event`, `Session`, `HookInput`. |
| `query` | `src/query.rs` | Query engine: `QueryFilter` struct, dynamic SQL builder, time parsing. |
| `usage` | `src/usage.rs` | Transcript JSONL parser; `SessionUsage` token-usage aggregate. |
| `storage` | `src/storage.rs` | File-backed payload storage for large events (>4KB). |

## Entry Point

`src/main.rs` does three things:

1. Parses CLI arguments via `cli::Cli::parse()` (clap).
2. Opens the SQLite connection via `db::connect()` (creates data dir and runs schema DDL if needed).
3. Dispatches to the matching command handler via `run(command, &conn)`.

All command handlers receive a `&rusqlite::Connection` and return `anyhow::Result<()>`. Errors are printed to stderr prefixed with "Error:" and exit with code 1.

## Data Flow: Capture Pipeline

The capture command is the hot path -- it runs as a Claude Code hook and must complete quickly.

```
stdin JSON
    |
    v
HookInput::from_json()        -- deserialize full JSON blob
    |
    v
hook.event_type()              -- map hook_event_name -> EventType enum
    |
    v
hook.payload_size()            -- estimate serialized byte size
    |
    +-- > 4096 bytes ------------> storage::write_payload()
    |     (large payload)              writes to payloads/<session_id>/<uuid>.json
    |                                  returns absolute path
    |                                  metadata_json = hook.metadata() (small fields only)
    |
    +-- <= 4096 bytes -----------> metadata_json = full stdin JSON (inline)
    |     (small payload)          payload_path = None
    |
    v
db::insert_event()             -- INSERT into events table
    |
    v
db::upsert_session()           -- INSERT or UPDATE sessions table
    |
    +-- SessionEnd only -------> usage::parse_transcript()
                                   db::upsert_session_usage()
```

The 4096-byte threshold (`storage::PAYLOAD_THRESHOLD`) determines whether the full JSON is stored inline in `metadata_json` or written to a separate file. Large payloads (tool responses, assistant messages) go to disk; small payloads stay inline for fast queries.

When the event is a `SessionEnd`, capture also parses the session transcript (`hook.transcript_path`) for token usage and writes a `session_usage` row. This is best-effort — a missing or malformed transcript is swallowed so it never fails the capture. Sessions whose `SessionEnd` never fired are filled in later by `ivara backfill-usage`.

## Data Flow: Query Pipeline

All read commands follow the same pattern:

```
CLI args
    |
    v
db::resolve_session()          -- prefix-match session IDs (LIKE 'prefix%')
    |
    v
db function                    -- session_events(), get_event(), list_sessions()
  or
query::query_events()          -- dynamic SQL builder with QueryFilter
    |
    v
Vec<Event> or Vec<Session>
    |
    +-- --json flag -----------> serde_json::to_string_pretty() -> stdout
    |
    +-- table format ----------> formatted println! -> stdout
```

The `show` command additionally reads file-backed payloads via `storage::read_payload()` and inlines them into the output.

## Module Dependencies

```
main.rs
  +-> cli           (parse args)
  +-> db             (connect, schema)
  +-> commands/
        capture  --> events, storage, db, usage
        sessions --> db
        query    --> db, storage, query (engine)
        analysis --> db, query, sessions (format_duration_secs)
        maintenance --> db, storage
        usage    --> db, usage
```

Key points:
- `db` and `storage` are the only modules with filesystem side effects.
- `events` is a pure data module (types only, no I/O).
- `query` (the engine) builds dynamic SQL; `commands::query` handles CLI output formatting.
- `commands::analysis` reuses `format_duration_secs` from `commands::sessions`.

## Concurrency Model

ivara uses SQLite with WAL mode and a 5-second busy timeout (`db::connect()`). Multiple concurrent captures (e.g., parallel hook invocations) are safe. There is no in-process concurrency -- each invocation is a single-threaded process.

## Error Handling

All functions return `anyhow::Result`. The `main()` function catches errors, prints them to stderr prefixed with "Error:", and exits with code 1. Empty error messages are suppressed.

## Source References

- Entry point and dispatch: `src/main.rs`
- CLI definition: `src/cli.rs`
- Capture pipeline: `src/commands/capture.rs`
- Database layer: `src/db.rs`
- Query engine: `src/query.rs`
- Payload storage: `src/storage.rs`
- Type definitions: `src/events.rs`
- Transcript token-usage parser: `src/usage.rs`
- Backfill command: `src/commands/usage.rs`
