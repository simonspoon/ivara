# Configuration

ivara uses convention-over-configuration. There are no config files -- behavior is controlled through two environment variables and compile-time constants.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `IVARA_HOME` | `~/.ivara` | Root directory for all ivara data (database and payload files). |
| `IVARA_HOOKS_HOME` | `~` | Home directory used by `install-hooks` / `uninstall-hooks` / `hooks status` to locate `<home>/.claude/settings.json` at `user` scope. Does not affect the telemetry data directory. |

Set `IVARA_HOME` to relocate the data directory:

```bash
export IVARA_HOME=/custom/path
ivara sessions   # reads from /custom/path/ivara.db
```

This is also used by the test infrastructure to isolate test runs (see [contributing.md](../dev/contributing.md)).

`IVARA_HOOKS_HOME` only affects the hook-management commands at `user` scope: instead of `~/.claude`, they resolve settings and the wrapper script under `$IVARA_HOOKS_HOME/.claude`. It is mainly used to isolate hook-install tests; `project`-scope commands ignore it and always target `<cwd>/.claude`.

## Data Directory Layout

```
$IVARA_HOME/              # defaults to ~/.ivara/
  ivara.db                # SQLite database (events, sessions, session_usage tables)
  payloads/               # file-backed payloads for large events
    <session_id>/         # one subdirectory per session
      <event_uuid>.json   # full JSON payload from capture
```

The directory is created automatically on first use by `db::connect()`. No manual setup is required.

### Database File

`ivara.db` is a SQLite database containing three tables (`events`, `sessions`, and `session_usage`). See [data-model.md](../dev/data-model.md) for the full schema.

### Payload Files

Events larger than the payload threshold are stored as individual JSON files under `payloads/<session_id>/`. Each file is named `<event_uuid>.json` and contains the complete JSON that was piped to `ivara capture` via stdin.

The `prune` command cleans up payload files when their corresponding events are deleted, and removes empty session directories afterward.

## SQLite Configuration

These settings are applied on every connection in `db::connect()` (`src/db.rs`):

| Setting | Value | Purpose |
|---------|-------|---------|
| `journal_mode` | `WAL` | Write-Ahead Logging for concurrent read/write access. Enables multiple ivara processes to capture simultaneously without blocking. |
| `busy_timeout` | `5000` ms (5 seconds) | Maximum time to wait when the database is locked by another process before returning a BUSY error. |
| `foreign_keys` | `ON` | Enables foreign key constraint enforcement. |

WAL mode is particularly important because Claude Code may fire multiple hooks concurrently (e.g., when subagents are active). The 5-second busy timeout provides enough headroom for brief contention without causing hook timeouts.

## Payload Threshold

| Constant | Value | Location |
|----------|-------|----------|
| `PAYLOAD_THRESHOLD` | `4096` bytes | `src/storage.rs` |

Events are classified by their estimated serialized size (`HookInput::payload_size()`):

| Size | Storage Strategy |
|------|------------------|
| <= 4096 bytes | **Inline**: full JSON stored in the `metadata_json` column of the `events` table. No file written. |
| > 4096 bytes | **File-backed**: full JSON written to `payloads/<session_id>/<uuid>.json`. The `metadata_json` column stores only small metadata fields (excludes `tool_input`, `tool_response`, `last_assistant_message`). The `payload_path` column stores the absolute file path. |

This threshold is a compile-time constant. Most session lifecycle events (SessionStart, SessionEnd, CwdChanged) are well under 4096 bytes. Tool events with large inputs or responses (e.g., file reads, long command outputs) typically exceed it.

## Path Resolution

The data directory path is resolved in this order (`db::data_dir()` in `src/db.rs`):

1. If `IVARA_HOME` environment variable is set, use its value.
2. Otherwise, use `$HOME/.ivara` (via the `dirs` crate's `home_dir()`).

The database path is always `data_dir().join("ivara.db")`. The payloads directory is always `data_dir().join("payloads")`.

## Source References

- Data directory and connection setup: `src/db.rs` (`data_dir()`, `db_path()`, `connect()`)
- Payload threshold and file operations: `src/storage.rs` (`PAYLOAD_THRESHOLD`, `payloads_dir()`, `session_payload_dir()`)
