use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::Connection;

use super::sessions::format_duration_secs;
use crate::events::ActiveSession;

/// List currently-live sessions — those with a `SessionStart` event and no `SessionEnd` event.
pub fn run(conn: &Connection, json: bool, limit: i64) -> Result<()> {
    let sessions = crate::db::list_active_sessions(conn, limit)?;
    let now = Utc::now();

    let active: Vec<ActiveSession> = sessions
        .into_iter()
        .map(|s| {
            let duration = duration_string(&s.first_seen, &s.last_seen);
            let idle = idle_string(&s.last_seen, now);
            let tool = crate::db::in_flight_tool(conn, &s.session_id)
                .ok()
                .flatten();
            ActiveSession {
                session_id: s.session_id,
                last_seen: s.last_seen,
                duration,
                event_count: s.event_count,
                cwd: s.cwd,
                idle,
                tool,
                model: s.model,
            }
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&active)?);
        return Ok(());
    }

    if active.is_empty() {
        println!("No active sessions.");
        return Ok(());
    }

    println!(
        "{:<40} {:<22} {:<12} {:<8} {:<40} {:<10} {:<14} MODEL",
        "SESSION", "LAST SEEN", "DURATION", "EVENTS", "CWD", "IDLE", "TOOL",
    );
    println!("{}", "-".repeat(160));

    for a in &active {
        let cwd = a.cwd.as_deref().unwrap_or("-");
        let tool = a.tool.as_deref().unwrap_or("");
        let model = a.model.as_deref().unwrap_or("");
        // Truncate SESSION to 38 chars, matching `ivara sessions`.
        let sid = if a.session_id.len() > 38 {
            &a.session_id[..38]
        } else {
            &a.session_id
        };
        let last = if a.last_seen.len() > 19 {
            &a.last_seen[..19]
        } else {
            &a.last_seen
        };
        println!(
            "{:<40} {:<22} {:<12} {:<8} {:<40} {:<10} {:<14} {}",
            sid, last, a.duration, a.event_count, cwd, a.idle, tool, model,
        );
    }

    Ok(())
}

/// Format the session duration (first_seen → last_seen). Returns "-" when either side
/// does not parse as RFC 3339.
fn duration_string(first_seen: &str, last_seen: &str) -> String {
    match (
        DateTime::parse_from_rfc3339(first_seen),
        DateTime::parse_from_rfc3339(last_seen),
    ) {
        (Ok(first), Ok(last)) => format_duration_secs((last - first).num_seconds()),
        _ => "-".to_string(),
    }
}

/// Format idle (time since `last_seen`). Clamps negative deltas to 0 so slight clock skew
/// doesn't render as a negative number.
fn idle_string(last_seen: &str, now: DateTime<Utc>) -> String {
    match DateTime::parse_from_rfc3339(last_seen) {
        Ok(last) => {
            let secs = (now - last.with_timezone(&Utc)).num_seconds().max(0);
            format_duration_secs(secs)
        }
        Err(_) => "-".to_string(),
    }
}
