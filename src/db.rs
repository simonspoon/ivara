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

        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            event_count INTEGER NOT NULL DEFAULT 0,
            cwd TEXT,
            model TEXT
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
    Ok(count)
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
