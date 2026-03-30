use anyhow::Result;
use rusqlite::Connection;

/// Query filter parameters.
pub struct QueryFilter {
    pub event_type: Option<String>,
    pub tool: Option<String>,
    pub session: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub has_error: bool,
    pub limit: i64,
}

/// Execute a filtered query against the events table.
pub fn query_events(conn: &Connection, filter: &QueryFilter) -> Result<Vec<crate::events::Event>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref et) = filter.event_type {
        conditions.push("event_type = ?".to_string());
        params.push(Box::new(et.clone()));
    }

    if let Some(ref tool) = filter.tool {
        conditions.push("tool_name = ?".to_string());
        params.push(Box::new(tool.clone()));
    }

    if let Some(ref session) = filter.session {
        conditions.push("session_id LIKE ?".to_string());
        params.push(Box::new(format!("{}%", session)));
    }

    if let Some(ref since) = filter.since {
        let ts = parse_time_arg(since)?;
        conditions.push("timestamp >= ?".to_string());
        params.push(Box::new(ts));
    }

    if let Some(ref until) = filter.until {
        let ts = parse_time_arg(until)?;
        conditions.push("timestamp <= ?".to_string());
        params.push(Box::new(ts));
    }

    if filter.has_error {
        conditions.push(
            "(event_type IN ('StopFailure', 'PostToolUseFailure') OR metadata_json LIKE '%\"error\"%')"
                .to_string(),
        );
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, event_uuid, session_id, event_type, timestamp, tool_name, tool_use_id, cwd, payload_path, metadata_json
         FROM events {} ORDER BY timestamp DESC LIMIT ?",
        where_clause
    );

    params.push(Box::new(filter.limit));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
    })?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

/// Parse a time argument — either ISO 8601 or a relative duration like "1h", "2d", "30m".
pub fn parse_time_arg(s: &str) -> Result<String> {
    // Try relative duration first
    let len = s.len();
    if len >= 2 {
        let (num_part, unit) = s.split_at(len - 1);
        if let Ok(n) = num_part.parse::<i64>() {
            let duration = match unit {
                "m" => chrono::Duration::minutes(n),
                "h" => chrono::Duration::hours(n),
                "d" => chrono::Duration::days(n),
                "w" => chrono::Duration::weeks(n),
                _ => {
                    // Not a relative duration, treat as ISO timestamp
                    return Ok(s.to_string());
                }
            };
            let ts = chrono::Utc::now() - duration;
            return Ok(ts.to_rfc3339());
        }
    }
    // Treat as ISO 8601 or date string
    Ok(s.to_string())
}

/// Get event count by type for stats.
pub fn event_counts_by_type(
    conn: &Connection,
    session_id: Option<&str>,
) -> Result<Vec<(String, i64)>> {
    let (sql, param): (String, Option<String>) = match session_id {
        Some(sid) => (
            "SELECT event_type, COUNT(*) FROM events WHERE session_id LIKE ? GROUP BY event_type ORDER BY COUNT(*) DESC".to_string(),
            Some(format!("{}%", sid)),
        ),
        None => (
            "SELECT event_type, COUNT(*) FROM events GROUP BY event_type ORDER BY COUNT(*) DESC"
                .to_string(),
            None,
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let mapper =
        |row: &rusqlite::Row| -> rusqlite::Result<(String, i64)> { Ok((row.get(0)?, row.get(1)?)) };
    let mut counts = Vec::new();
    if let Some(ref p) = param {
        let rows = stmt.query_map([p], mapper)?;
        for row in rows {
            counts.push(row?);
        }
    } else {
        let rows = stmt.query_map([], mapper)?;
        for row in rows {
            counts.push(row?);
        }
    }
    Ok(counts)
}

/// Get tool usage frequency.
pub fn tool_frequency(conn: &Connection, session_id: Option<&str>) -> Result<Vec<(String, i64)>> {
    let (sql, param): (String, Option<String>) = match session_id {
        Some(sid) => (
            "SELECT tool_name, COUNT(*) FROM events WHERE tool_name IS NOT NULL AND session_id LIKE ? GROUP BY tool_name ORDER BY COUNT(*) DESC".to_string(),
            Some(format!("{}%", sid)),
        ),
        None => (
            "SELECT tool_name, COUNT(*) FROM events WHERE tool_name IS NOT NULL GROUP BY tool_name ORDER BY COUNT(*) DESC".to_string(),
            None,
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let mapper =
        |row: &rusqlite::Row| -> rusqlite::Result<(String, i64)> { Ok((row.get(0)?, row.get(1)?)) };
    let mut counts = Vec::new();
    if let Some(ref p) = param {
        let rows = stmt.query_map([p], mapper)?;
        for row in rows {
            counts.push(row?);
        }
    } else {
        let rows = stmt.query_map([], mapper)?;
        for row in rows {
            counts.push(row?);
        }
    }
    Ok(counts)
}

/// Get error rate (error events / total events).
pub fn error_rate(conn: &Connection, session_id: Option<&str>) -> Result<(i64, i64)> {
    let (total_sql, error_sql, param): (String, String, Option<String>) = match session_id {
        Some(sid) => (
            "SELECT COUNT(*) FROM events WHERE session_id LIKE ?".to_string(),
            "SELECT COUNT(*) FROM events WHERE session_id LIKE ? AND event_type IN ('StopFailure', 'PostToolUseFailure')".to_string(),
            Some(format!("{}%", sid)),
        ),
        None => (
            "SELECT COUNT(*) FROM events".to_string(),
            "SELECT COUNT(*) FROM events WHERE event_type IN ('StopFailure', 'PostToolUseFailure')"
                .to_string(),
            None,
        ),
    };

    let total: i64 = if let Some(ref p) = param {
        conn.query_row(&total_sql, [p], |row| row.get(0))?
    } else {
        conn.query_row(&total_sql, [], |row| row.get(0))?
    };

    let errors: i64 = if let Some(ref p) = param {
        conn.query_row(&error_sql, [p], |row| row.get(0))?
    } else {
        conn.query_row(&error_sql, [], |row| row.get(0))?
    };

    Ok((errors, total))
}
