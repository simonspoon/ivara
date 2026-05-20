use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("IVARA_HOME") {
        return PathBuf::from(dir);
    }
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".ivara")
}

pub fn db_path() -> PathBuf {
    data_dir().join("ivara.db")
}

pub fn connect() -> Result<Connection> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let conn = Connection::open(db_path())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    initialize(&conn)?;
    Ok(conn)
}

fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_uuid TEXT NOT NULL UNIQUE,
            session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            tool_name TEXT,
            tool_use_id TEXT,
            cwd TEXT,
            payload_path TEXT,
            metadata_json TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id);
        CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
        CREATE INDEX IF NOT EXISTS idx_events_tool_name ON events(tool_name);
        CREATE INDEX IF NOT EXISTS idx_events_tool_use_id ON events(tool_use_id);
        CREATE INDEX IF NOT EXISTS idx_events_session_type ON events(session_id, event_type);

        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            event_count INTEGER NOT NULL DEFAULT 0,
            cwd TEXT,
            model TEXT
        );

        CREATE TABLE IF NOT EXISTS session_usage (
            session_id TEXT PRIMARY KEY,
            model TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            web_search_requests INTEGER NOT NULL DEFAULT 0,
            web_fetch_requests INTEGER NOT NULL DEFAULT 0,
            api_calls INTEGER NOT NULL DEFAULT 0,
            parsed_at TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

/// Insert a new event row. Returns the row ID.
#[allow(clippy::too_many_arguments)]
pub fn insert_event(
    conn: &Connection,
    event_uuid: &str,
    session_id: &str,
    event_type: &str,
    timestamp: &str,
    tool_name: Option<&str>,
    tool_use_id: Option<&str>,
    cwd: Option<&str>,
    payload_path: Option<&str>,
    metadata_json: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO events (event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            event_uuid,
            session_id,
            event_type,
            timestamp,
            tool_name,
            tool_use_id,
            cwd,
            payload_path,
            metadata_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Upsert the sessions table when a new event arrives.
pub fn upsert_session(
    conn: &Connection,
    session_id: &str,
    timestamp: &str,
    cwd: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (session_id, first_seen, last_seen, event_count, cwd, model)
         VALUES (?1, ?2, ?2, 1, ?3, ?4)
         ON CONFLICT(session_id) DO UPDATE SET
           last_seen = MAX(sessions.last_seen, excluded.last_seen),
           event_count = sessions.event_count + 1,
           cwd = COALESCE(excluded.cwd, sessions.cwd),
           model = COALESCE(excluded.model, sessions.model)",
        rusqlite::params![session_id, timestamp, cwd, model],
    )?;
    Ok(())
}

/// List sessions, ordered by most recent first.
pub fn list_sessions(conn: &Connection, limit: i64) -> Result<Vec<crate::events::Session>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, first_seen, last_seen, event_count, cwd, model
         FROM sessions ORDER BY last_seen DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(crate::events::Session {
            session_id: row.get(0)?,
            first_seen: row.get(1)?,
            last_seen: row.get(2)?,
            event_count: row.get(3)?,
            cwd: row.get(4)?,
            model: row.get(5)?,
        })
    })?;
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row?);
    }
    Ok(sessions)
}

/// List active sessions — rows with at least one `SessionStart` event and no `SessionEnd` event.
///
/// Ordered by most-recent activity first.
pub fn list_active_sessions(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<crate::events::Session>> {
    let mut stmt = conn.prepare(
        "SELECT s.session_id, s.first_seen, s.last_seen, s.event_count, s.cwd, s.model
         FROM sessions s
         WHERE EXISTS (
             SELECT 1 FROM events
             WHERE session_id = s.session_id AND event_type = 'SessionStart'
         )
         AND NOT EXISTS (
             SELECT 1 FROM events
             WHERE session_id = s.session_id AND event_type = 'SessionEnd'
         )
         ORDER BY s.last_seen DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(crate::events::Session {
            session_id: row.get(0)?,
            first_seen: row.get(1)?,
            last_seen: row.get(2)?,
            event_count: row.get(3)?,
            cwd: row.get(4)?,
            model: row.get(5)?,
        })
    })?;
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row?);
    }
    Ok(sessions)
}

/// Find the tool name of the most recent `PreToolUse` event in a session that has no matching
/// `PostToolUse` or `PostToolUseFailure` (by `tool_use_id`). Returns None when nothing is in flight.
pub fn in_flight_tool(conn: &Connection, session_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT tool_name FROM events
         WHERE session_id = ?1
           AND event_type = 'PreToolUse'
           AND tool_use_id IS NOT NULL
           AND tool_use_id NOT IN (
               SELECT tool_use_id FROM events
               WHERE session_id = ?1
                 AND event_type IN ('PostToolUse', 'PostToolUseFailure')
                 AND tool_use_id IS NOT NULL
           )
         ORDER BY timestamp DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map([session_id], |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok(None),
    }
}

/// Get events for a session, optionally filtered by event type.
pub fn session_events(
    conn: &Connection,
    session_id: &str,
    event_type: Option<&str>,
) -> Result<Vec<crate::events::Event>> {
    let query = match event_type {
        Some(_) => {
            "SELECT id, event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json
             FROM events WHERE session_id LIKE ?1 AND event_type = ?2 ORDER BY timestamp ASC"
        }
        None => {
            "SELECT id, event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json
             FROM events WHERE session_id LIKE ?1 ORDER BY timestamp ASC"
        }
    };

    let pattern = format!("{}%", session_id);
    let mut stmt = conn.prepare(query)?;

    let rows = if let Some(et) = event_type {
        stmt.query_map(rusqlite::params![pattern, et], map_event_row)?
    } else {
        stmt.query_map(rusqlite::params![pattern], map_event_row)?
    };

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

/// Get events for a session with `id > after_id`, ordered by `id ASC`.
///
/// Used by `ivara stream` to poll for new events after initial replay. Prefix resolution
/// must happen before calling this — pass the full `session_id` (exact match).
pub fn session_events_after_id(
    conn: &Connection,
    session_id: &str,
    after_id: i64,
) -> Result<Vec<crate::events::Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json
         FROM events WHERE session_id = ?1 AND id > ?2 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, after_id], map_event_row)?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

/// Get a single event by ID.
pub fn get_event(conn: &Connection, event_id: i64) -> Result<Option<crate::events::Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json
         FROM events WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([event_id], map_event_row)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Resolve a session ID prefix to the full session ID.
pub fn resolve_session(conn: &Connection, prefix: &str) -> Result<Option<String>> {
    let pattern = format!("{}%", prefix);
    let mut stmt =
        conn.prepare("SELECT session_id FROM sessions WHERE session_id LIKE ?1 LIMIT 1")?;
    let mut rows = stmt.query_map([pattern], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Delete events and sessions before a given timestamp. Returns count of deleted events.
pub fn prune_before(conn: &Connection, before: &str) -> Result<usize> {
    let count = conn.execute("DELETE FROM events WHERE timestamp < ?1", [before])?;
    conn.execute(
        "DELETE FROM sessions WHERE session_id NOT IN (SELECT DISTINCT session_id FROM events)",
        [],
    )?;
    conn.execute(
        "DELETE FROM session_usage WHERE session_id NOT IN (SELECT DISTINCT session_id FROM events)",
        [],
    )?;
    Ok(count)
}

/// Insert or replace the token-usage row for a session.
///
/// A full overwrite is correct: re-parsing a transcript always yields the
/// complete usage picture, so the latest parse supersedes any prior row.
pub fn upsert_session_usage(
    conn: &Connection,
    session_id: &str,
    usage: &crate::usage::SessionUsage,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO session_usage
         (session_id, model, input_tokens, output_tokens, cache_creation_tokens,
          cache_read_tokens, web_search_requests, web_fetch_requests, api_calls, parsed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            session_id,
            usage.model,
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_creation_tokens,
            usage.cache_read_tokens,
            usage.web_search_requests,
            usage.web_fetch_requests,
            usage.api_calls,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Get stored token usage for a single session. Returns None when the session
/// has no usage row yet (not captured at SessionEnd, not backfilled).
pub fn get_session_usage(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<crate::usage::SessionUsage>> {
    let mut stmt = conn.prepare(
        "SELECT model, input_tokens, output_tokens, cache_creation_tokens,
                cache_read_tokens, web_search_requests, web_fetch_requests, api_calls
         FROM session_usage WHERE session_id = ?1",
    )?;
    let mut rows = stmt.query_map([session_id], map_usage_row)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Sum token usage across every session. Returns None when no usage has been
/// recorded at all, so callers can distinguish "zero" from "unknown".
pub fn total_usage(conn: &Connection) -> Result<Option<crate::usage::SessionUsage>> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM session_usage", [], |row| row.get(0))?;
    if count == 0 {
        return Ok(None);
    }
    let usage = conn.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0), COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(web_search_requests), 0), COALESCE(SUM(web_fetch_requests), 0),
                COALESCE(SUM(api_calls), 0)
         FROM session_usage",
        [],
        |row| {
            Ok(crate::usage::SessionUsage {
                model: None,
                input_tokens: row.get(0)?,
                output_tokens: row.get(1)?,
                cache_creation_tokens: row.get(2)?,
                cache_read_tokens: row.get(3)?,
                web_search_requests: row.get(4)?,
                web_fetch_requests: row.get(5)?,
                api_calls: row.get(6)?,
            })
        },
    )?;
    Ok(Some(usage))
}

/// Find a transcript path recorded for a session. Claude Code includes
/// `transcript_path` in the payload of essentially every hook event, so this
/// works even when the `SessionEnd` hook was never wired.
pub fn session_transcript_path(conn: &Connection, session_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(metadata_json, '$.transcript_path')
         FROM events
         WHERE session_id = ?1
           AND metadata_json IS NOT NULL
           AND json_extract(metadata_json, '$.transcript_path') IS NOT NULL
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map([session_id], |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok(None),
    }
}

/// All known session IDs, most-recent activity first.
pub fn all_session_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT session_id FROM sessions ORDER BY last_seen DESC")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

fn map_usage_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::usage::SessionUsage> {
    Ok(crate::usage::SessionUsage {
        model: row.get(0)?,
        input_tokens: row.get(1)?,
        output_tokens: row.get(2)?,
        cache_creation_tokens: row.get(3)?,
        cache_read_tokens: row.get(4)?,
        web_search_requests: row.get(5)?,
        web_fetch_requests: row.get(6)?,
        api_calls: row.get(7)?,
    })
}

/// Get payload paths for events before a timestamp (for file cleanup).
pub fn payload_paths_before(conn: &Connection, before: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT payload_path FROM events WHERE timestamp < ?1 AND payload_path IS NOT NULL",
    )?;
    let rows = stmt.query_map([before], |row| row.get::<_, String>(0))?;
    let mut paths = Vec::new();
    for row in rows {
        paths.push(row?);
    }
    Ok(paths)
}

fn map_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::events::Event> {
    Ok(crate::events::Event {
        id: row.get(0)?,
        event_uuid: row.get(1)?,
        session_id: row.get(2)?,
        event_type: row.get(3)?,
        timestamp: row.get(4)?,
        tool_name: row.get(5)?,
        tool_use_id: row.get(6)?,
        cwd: row.get(7)?,
        payload_path: row.get(8)?,
        metadata_json: row.get(9)?,
    })
}
